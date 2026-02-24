#![no_std]

mod audit;
mod helpers;
mod verifier;
pub mod vk;

pub use crate::audit::{AuditRecord, AuditTrail};
pub use crate::helpers::ZkAccessHelper;
pub use crate::verifier::{Bn254Verifier, PoseidonHasher, Proof};

use common::whitelist;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    Symbol, Vec,
};

const ADMIN: Symbol = symbol_short!("ADMIN");
const RATE_CFG: Symbol = symbol_short!("RATECFG");
const RATE_TRACK: Symbol = symbol_short!("RLTRK");
const VK: Symbol = symbol_short!("VK");

/// Maximum number of public inputs accepted per proof verification.
const MAX_PUBLIC_INPUTS: u32 = 16;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRequest {
    pub user: Address,
    pub resource_id: BytesN<32>,
    pub proof: Proof,
    pub public_inputs: Vec<BytesN<32>>,
}

/// Contract errors for the ZK verifier.
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
}

#[contract]
pub struct ZkVerifierContract;

/// Return `true` if every byte in `data` is zero.
fn is_all_zeros(data: &BytesN<32>) -> bool {
    let arr = data.to_array();
    let mut all_zero = true;
    let mut i = 0;
    while i < 32 {
        if arr[i] != 0 {
            all_zero = false;
            break;
        }
        i += 1;
    }
    all_zero
}

/// Validate request shape before running proof verification.
fn validate_request(request: &AccessRequest) -> Result<(), ContractError> {
    if request.public_inputs.is_empty() {
        return Err(ContractError::EmptyPublicInputs);
    }

    if request.public_inputs.len() > MAX_PUBLIC_INPUTS {
        return Err(ContractError::TooManyPublicInputs);
    }

    if (is_all_zeros(&request.proof.a.x) && is_all_zeros(&request.proof.a.y))
        || (is_all_zeros(&request.proof.b.x.0)
            && is_all_zeros(&request.proof.b.x.1)
            && is_all_zeros(&request.proof.b.y.0)
            && is_all_zeros(&request.proof.b.y.1))
        || (is_all_zeros(&request.proof.c.x) && is_all_zeros(&request.proof.c.y))
    {
        return Err(ContractError::DegenerateProof);
    }

    Ok(())
}

#[contractimpl]
impl ZkVerifierContract {
    /// One-time initialization to set the admin address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&ADMIN) {
            return;
        }

        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
    }

    fn require_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::Unauthorized)?;

        if caller != &admin {
            return Err(ContractError::Unauthorized);
        }

        Ok(())
    }

    /// Configure per-address rate limiting for this contract.
    pub fn set_rate_limit_config(
        env: Env,
        caller: Address,
        max_requests_per_window: u64,
        window_duration_seconds: u64,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller)?;

        if max_requests_per_window == 0 || window_duration_seconds == 0 {
            return Err(ContractError::InvalidConfig);
        }

        env.storage().instance().set(
            &RATE_CFG,
            &(max_requests_per_window, window_duration_seconds),
        );

        Ok(())
    }

    /// Set the ZK verification key.
    pub fn set_verification_key(
        env: Env,
        caller: Address,
        vk: vk::VerificationKey,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller)?;
        env.storage().instance().set(&VK, &vk);
        Ok(())
    }

    /// Get the ZK verification key.
    pub fn get_verification_key(env: Env) -> Option<vk::VerificationKey> {
        env.storage().instance().get(&VK)
    }

    /// Return the current rate limiting configuration, if any.
    pub fn get_rate_limit_config(env: Env) -> Option<(u64, u64)> {
        env.storage().instance().get(&RATE_CFG)
    }

    /// Enables or disables whitelist enforcement.
    pub fn set_whitelist_enabled(
        env: Env,
        caller: Address,
        enabled: bool,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller)?;
        whitelist::set_whitelist_enabled(&env, enabled);
        Ok(())
    }

    /// Adds an address to the whitelist.
    pub fn add_to_whitelist(env: Env, caller: Address, user: Address) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller)?;
        whitelist::add_to_whitelist(&env, &user);
        Ok(())
    }

    /// Removes an address from the whitelist.
    pub fn remove_from_whitelist(
        env: Env,
        caller: Address,
        user: Address,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller)?;
        whitelist::remove_from_whitelist(&env, &user);
        Ok(())
    }

    pub fn is_whitelist_enabled(env: Env) -> bool {
        whitelist::is_whitelist_enabled(&env)
    }

    pub fn is_whitelisted(env: Env, user: Address) -> bool {
        whitelist::is_whitelisted(&env, &user)
    }

    fn check_and_update_rate_limit(env: &Env, user: &Address) -> Result<(), ContractError> {
        let cfg: Option<(u64, u64)> = env.storage().instance().get(&RATE_CFG);
        let (max_requests_per_window, window_duration_seconds) = match cfg {
            Some(c) => c,
            None => return Ok(()),
        };

        if max_requests_per_window == 0 || window_duration_seconds == 0 {
            return Ok(());
        }

        let now = env.ledger().timestamp();
        let key = (RATE_TRACK, user.clone());

        let mut state: (u64, u64) = env.storage().persistent().get(&key).unwrap_or((0, now));

        let window_end = state.1.saturating_add(window_duration_seconds);
        if now >= window_end {
            state.0 = 0;
            state.1 = now;
        }

        let next = state.0.saturating_add(1);
        if next > max_requests_per_window {
            return Err(ContractError::RateLimited);
        }

        state.0 = next;
        env.storage().persistent().set(&key, &state);

        Ok(())
    }

    pub fn verify_access(env: Env, request: AccessRequest) -> Result<bool, ContractError> {
        request.user.require_auth();

        validate_request(&request)?;

        if !whitelist::check_whitelist_access(&env, &request.user) {
            return Err(ContractError::Unauthorized);
        }

        Self::check_and_update_rate_limit(&env, &request.user)?;

        let vk: vk::VerificationKey = env
            .storage()
            .instance()
            .get(&VK)
            .ok_or(ContractError::Unauthorized)?;

        let is_valid = Bn254Verifier::verify_proof(&env, &vk, &request.proof, &request.public_inputs);
        if is_valid {
            let proof_hash = PoseidonHasher::hash(&env, &request.public_inputs);
            AuditTrail::log_access(&env, request.user, request.resource_id, proof_hash);
        }
        Ok(is_valid)
    }

    pub fn get_audit_record(
        env: Env,
        user: Address,
        resource_id: BytesN<32>,
    ) -> Option<AuditRecord> {
        AuditTrail::get_record(&env, user, resource_id)
    }
}
