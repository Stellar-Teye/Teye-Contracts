#![no_std]
#![allow(dead_code, clippy::arithmetic_side_effects)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod audit;
pub mod events;
mod helpers;
pub mod plonk;
pub mod verifier;
pub mod vk;

pub use crate::audit::{AuditRecord, AuditTrail};
pub use crate::events::AccessRejectedEvent;
pub use crate::helpers::{MerkleVerifier, ZkAccessHelper};
pub use crate::verifier::{Bn254Verifier, PoseidonHasher, Proof, ProofValidationError, ZkVerifier};
pub use crate::vk::VerificationKey;

use common::{nonce, whitelist, CommonError};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    Vec,
};

const MAX_PUBLIC_INPUTS: u32 = 16;
const MAX_BATCH_PROOFS: u32 = 64;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum DataKey {
    Admin,
    Initialized,
    VerificationKey,
    Paused,
    RateConfig,
    RateState(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct RateLimitConfig {
    pub max_requests_per_window: u64,
    pub window_duration_seconds: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct RateLimitState {
    pub count: u64,
    pub window_start: u64,
}

/// Request envelope for proof verification.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRequest {
    pub user: Address,
    pub resource_id: BytesN<32>,
    pub proof: Proof,
    pub public_inputs: Vec<BytesN<32>>,
    pub nonce: u64,
    pub expires_at: u64,
}

/// Batch verification response.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchVerificationSummary {
    pub total: u32,
    pub verified: u32,
    pub recursive_valid: bool,
}

/// Per-item batch audit event payload.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchAccessAuditEvent {
    pub user: Address,
    pub resource_id: BytesN<32>,
    pub proof_index: u32,
    pub verified: bool,
    pub timestamp: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    Unauthorized = 1,
    RateLimited = 2,
    InvalidConfig = 3,
    EmptyPublicInputs = 4,
    TooManyPublicInputs = 5,
    DegenerateProof = 6,
    OversizedProofComponent = 7,
    MalformedG1Point = 8,
    MalformedG2Point = 9,
    ZeroedPublicInput = 10,
    MalformedProofData = 11,
    ExpiredProof = 12,
    InvalidNonce = 13,
    Paused = 14,
    BatchTooLarge = 15,
}

fn map_proof_validation_error(error: ProofValidationError) -> ContractError {
    match error {
        ProofValidationError::ZeroedComponent => ContractError::DegenerateProof,
        ProofValidationError::OversizedComponent => ContractError::OversizedProofComponent,
        ProofValidationError::MalformedG1PointA | ProofValidationError::MalformedG1PointC => {
            ContractError::MalformedG1Point
        }
        ProofValidationError::MalformedG2Point => ContractError::MalformedG2Point,
        ProofValidationError::EmptyPublicInputs => ContractError::EmptyPublicInputs,
        ProofValidationError::ZeroedPublicInput => ContractError::ZeroedPublicInput,
    }
}

fn map_nonce_error(error: CommonError) -> ContractError {
    match error {
        CommonError::InvalidNonce | CommonError::NonceOverflow => ContractError::InvalidNonce,
        _ => ContractError::InvalidNonce,
    }
}

fn default_verification_key(env: &Env) -> VerificationKey {
    let one = BytesN::from_array(env, &[1u8; 32]);
    let g1 = vk::G1Point {
        x: one.clone(),
        y: one.clone(),
    };
    let g2 = vk::G2Point {
        x: (one.clone(), one.clone()),
        y: (one.clone(), one.clone()),
    };
    let mut ic = Vec::new(env);
    ic.push_back(g1.clone());

    VerificationKey {
        alpha_g1: g1.clone(),
        beta_g2: g2.clone(),
        gamma_g2: g2.clone(),
        delta_g2: g2,
        ic,
    }
}

fn get_verification_key(env: &Env) -> VerificationKey {
    env.storage()
        .instance()
        .get(&DataKey::VerificationKey)
        .unwrap_or_else(|| default_verification_key(env))
}

fn require_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ContractError::Unauthorized)?;
    if *caller != admin {
        return Err(ContractError::Unauthorized);
    }
    Ok(())
}

fn validate_request(env: &Env, request: &AccessRequest) -> Result<(), ContractError> {
    if request.public_inputs.is_empty() {
        return Err(ContractError::EmptyPublicInputs);
    }
    if request.public_inputs.len() > MAX_PUBLIC_INPUTS {
        return Err(ContractError::TooManyPublicInputs);
    }
    if request.expires_at != 0 && env.ledger().timestamp() > request.expires_at {
        return Err(ContractError::ExpiredProof);
    }
    Bn254Verifier::validate_proof_components(&request.proof, &request.public_inputs)
        .map_err(map_proof_validation_error)
}

fn consume_rate_limit(env: &Env, user: &Address) -> bool {
    let maybe_cfg: Option<RateLimitConfig> = env.storage().instance().get(&DataKey::RateConfig);
    let cfg = match maybe_cfg {
        Some(cfg) => cfg,
        None => return true,
    };
    if cfg.max_requests_per_window == 0 || cfg.window_duration_seconds == 0 {
        return true;
    }

    let now = env.ledger().timestamp();
    let key = DataKey::RateState(user.clone());
    let mut state: RateLimitState =
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(RateLimitState {
                count: 0,
                window_start: now,
            });

    if now.saturating_sub(state.window_start) >= cfg.window_duration_seconds {
        state.window_start = now;
        state.count = 0;
    }

    if state.count >= cfg.max_requests_per_window {
        return false;
    }

    state.count = state.count.saturating_add(1);
    env.storage().persistent().set(&key, &state);
    true
}

fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

#[contract]
pub struct ZkVerifierContract;

#[contractimpl]
impl ZkVerifierContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        admin.require_auth();
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(ContractError::InvalidConfig);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Initialized, &true);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(
            &DataKey::RateConfig,
            &RateLimitConfig {
                max_requests_per_window: 0,
                window_duration_seconds: 0,
            },
        );
        whitelist::set_whitelist_enabled(&env, false);
        Ok(())
    }

    pub fn set_verification_key(
        env: Env,
        caller: Address,
        vk: VerificationKey,
    ) -> Result<(), ContractError> {
        require_admin(&env, &caller)?;
        env.storage().instance().set(&DataKey::VerificationKey, &vk);
        Ok(())
    }

    pub fn set_rate_limit_config(
        env: Env,
        caller: Address,
        max_requests_per_window: u64,
        window_duration_seconds: u64,
    ) -> Result<(), ContractError> {
        require_admin(&env, &caller)?;
        if max_requests_per_window == 0 || window_duration_seconds == 0 {
            return Err(ContractError::InvalidConfig);
        }
        env.storage().instance().set(
            &DataKey::RateConfig,
            &RateLimitConfig {
                max_requests_per_window,
                window_duration_seconds,
            },
        );
        Ok(())
    }

    pub fn pause(env: Env, caller: Address) -> bool {
        if require_admin(&env, &caller).is_err() {
            return false;
        }
        env.storage().instance().set(&DataKey::Paused, &true);
        true
    }

    pub fn unpause(env: Env, caller: Address) -> bool {
        if require_admin(&env, &caller).is_err() {
            return false;
        }
        env.storage().instance().set(&DataKey::Paused, &false);
        true
    }

    pub fn set_whitelist_enabled(
        env: Env,
        caller: Address,
        enabled: bool,
    ) -> Result<(), ContractError> {
        require_admin(&env, &caller)?;
        whitelist::set_whitelist_enabled(&env, enabled);
        Ok(())
    }

    pub fn add_to_whitelist(
        env: Env,
        caller: Address,
        address: Address,
    ) -> Result<(), ContractError> {
        require_admin(&env, &caller)?;
        whitelist::add_to_whitelist(&env, &address);
        Ok(())
    }

    pub fn remove_from_whitelist(
        env: Env,
        caller: Address,
        address: Address,
    ) -> Result<(), ContractError> {
        require_admin(&env, &caller)?;
        whitelist::remove_from_whitelist(&env, &address);
        Ok(())
    }

    pub fn get_nonce(env: Env, user: Address) -> u64 {
        nonce::current_nonce(&env, &user)
    }

    pub fn verify_access(env: Env, request: AccessRequest) -> Result<bool, ContractError> {
        Self::verify_single(&env, &request, false)
    }

    pub fn verify_access_plonk(env: Env, request: AccessRequest) -> Result<bool, ContractError> {
        Self::verify_single(&env, &request, true)
    }

    /// Verify a recursively composed batch of access proofs in one transaction.
    pub fn verify_batch_access(
        env: Env,
        proofs: Vec<AccessRequest>,
    ) -> Result<BatchVerificationSummary, ContractError> {
        if is_paused(&env) {
            return Err(ContractError::Paused);
        }
        if proofs.is_empty() {
            return Err(ContractError::EmptyPublicInputs);
        }
        if proofs.len() > MAX_BATCH_PROOFS {
            return Err(ContractError::BatchTooLarge);
        }

        let vk = get_verification_key(&env);
        let mut recursive_proofs = Vec::new(&env);
        let mut recursive_inputs: Vec<Vec<BytesN<32>>> = Vec::new(&env);

        for request in proofs.iter() {
            request.user.require_auth();
            recursive_proofs.push_back(request.proof.clone());
            recursive_inputs.push_back(request.public_inputs.clone());
        }

        let recursive_valid =
            Bn254Verifier::verify_recursive_proof(&env, &vk, &recursive_proofs, &recursive_inputs);

        let mut verified: u32 = 0;
        let mut i: u32 = 0;
        while i < proofs.len() {
            let request = match proofs.get(i) {
                Some(request) => request,
                None => break,
            };

            let mut proof_valid = false;
            let mut error = None;

            if !whitelist::check_whitelist_access(&env, &request.user) {
                error = Some(ContractError::Unauthorized);
            } else if let Err(e) =
                nonce::validate_and_increment_nonce(&env, &request.user, request.nonce)
            {
                error = Some(map_nonce_error(e));
            } else if !consume_rate_limit(&env, &request.user) {
                error = Some(ContractError::RateLimited);
            } else if let Err(e) = validate_request(&env, &request) {
                error = Some(e);
            } else if recursive_valid {
                proof_valid = true;
            } else {
                proof_valid =
                    Bn254Verifier::verify_proof(&env, &vk, &request.proof, &request.public_inputs);
                if !proof_valid {
                    error = Some(ContractError::MalformedProofData);
                }
            }

            if proof_valid {
                let proof_hash = PoseidonHasher::hash(&env, &request.public_inputs);
                AuditTrail::log_access(
                    &env,
                    request.user.clone(),
                    request.resource_id.clone(),
                    proof_hash,
                    request.expires_at,
                );
                verified = verified.saturating_add(1);
            } else if let Some(err) = error {
                events::publish_access_rejected(
                    &env,
                    request.user.clone(),
                    request.resource_id.clone(),
                    err,
                );
            }

            env.events().publish(
                (
                    symbol_short!("BATCHLOG"),
                    request.user.clone(),
                    request.resource_id.clone(),
                ),
                BatchAccessAuditEvent {
                    user: request.user.clone(),
                    resource_id: request.resource_id.clone(),
                    proof_index: i,
                    verified: proof_valid,
                    timestamp: env.ledger().timestamp(),
                },
            );

            i = i.saturating_add(1);
        }

        Ok(BatchVerificationSummary {
            total: proofs.len(),
            verified,
            recursive_valid,
        })
    }

    pub fn get_audit_record(
        env: Env,
        user: Address,
        resource_id: BytesN<32>,
    ) -> Option<AuditRecord> {
        AuditTrail::get_record(&env, user, resource_id)
    }

    pub fn verify_audit_chain(env: Env, user: Address, resource_id: BytesN<32>) -> bool {
        AuditTrail::verify_chain(&env, user, resource_id)
    }

    pub fn verify_data_inclusion(
        env: Env,
        root: BytesN<32>,
        leaf: BytesN<32>,
        proof_path: Vec<(BytesN<32>, bool)>,
    ) -> bool {
        MerkleVerifier::verify_merkle_proof(&env, &root, &leaf, &proof_path)
    }
}

impl ZkVerifierContract {
    fn verify_single(
        env: &Env,
        request: &AccessRequest,
        use_plonk: bool,
    ) -> Result<bool, ContractError> {
        if is_paused(env) {
            return Err(ContractError::Paused);
        }

        request.user.require_auth();

        if !whitelist::check_whitelist_access(env, &request.user) {
            events::publish_access_rejected(
                env,
                request.user.clone(),
                request.resource_id.clone(),
                ContractError::Unauthorized,
            );
            return Err(ContractError::Unauthorized);
        }

        validate_request(env, request).inspect_err(|err| {
            events::publish_access_rejected(
                env,
                request.user.clone(),
                request.resource_id.clone(),
                *err,
            )
        })?;

        nonce::validate_and_increment_nonce(env, &request.user, request.nonce)
            .map_err(map_nonce_error)
            .inspect_err(|err| {
                events::publish_access_rejected(
                    env,
                    request.user.clone(),
                    request.resource_id.clone(),
                    *err,
                )
            })?;

        if !consume_rate_limit(env, &request.user) {
            events::publish_access_rejected(
                env,
                request.user.clone(),
                request.resource_id.clone(),
                ContractError::RateLimited,
            );
            return Err(ContractError::RateLimited);
        }

        let vk = get_verification_key(env);
        let is_valid = if use_plonk {
            plonk::PlonkVerifier::verify_proof(env, &vk, &request.proof, &request.public_inputs)
        } else {
            Bn254Verifier::verify_proof(env, &vk, &request.proof, &request.public_inputs)
        };

        if is_valid {
            let proof_hash = PoseidonHasher::hash(env, &request.public_inputs);
            AuditTrail::log_access(
                env,
                request.user.clone(),
                request.resource_id.clone(),
                proof_hash,
                request.expires_at,
            );
        } else {
            events::publish_access_rejected(
                env,
                request.user.clone(),
                request.resource_id.clone(),
                ContractError::MalformedProofData,
            );
        }

        Ok(is_valid)
    }
}
