//! # ZK Verifier Module
//!
//! This module provides a Zero-Knowledge (ZK) proof verification system for the Soroban ecosystem.
//! It specifically implements support for Groth16 proofs over the BN254 (Alt-BN128) curve.
//!
//! The ZK subsystem is designed to provide privacy-preserving access control by allowing users
//! to prove they possess certain credentials or meet specific criteria without revealing
//! the underlying sensitive data.
//!
//! ## Key Components
//! - `ZkVerifierContract`: The main contract implementation handling access requests and auditing.
//! - `Bn254Verifier`: The core library for verifying Groth16 proofs.
//! - `AuditTrail`: A persistence layer for logging successful verifications.
//! - `ZkAccessHelper`: A utility for formatting binary proof data into interoperable requests.

mod audit;
pub mod events;
mod helpers;
mod verifier;
pub mod vk;

pub use crate::audit::{AuditRecord, AuditTrail};
pub use crate::helpers::ZkAccessHelper;
pub use crate::verifier::{Bn254Verifier, PoseidonHasher, Proof, VerificationKey};

use common::whitelist;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    Symbol, Vec,
};

const ADMIN: Symbol = symbol_short!("ADMIN");
const PENDING_ADMIN: Symbol = symbol_short!("PEND_ADM");
const VK: Symbol = symbol_short!("VK");
const RATE_CFG: Symbol = symbol_short!("RATECFG");
const RATE_TRACK: Symbol = symbol_short!("RLTRK");
const VK: Symbol = symbol_short!("VK");

/// Maximum number of public inputs accepted per proof verification.
const MAX_PUBLIC_INPUTS: u32 = 16;

/// Request structure for ZK access verification.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRequest {
    /// The address of the user requesting access.
    pub user: Address,
    /// Unique identifier for the resource being accessed.
    pub resource_id: BytesN<32>,
    /// The Groth16 proof (points A, B, and C).
    pub proof: Proof,
    /// Public inputs associated with the proof.
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
    /// A proof component is saturated (all 0xFF) — invalid curve encoding.
    OversizedProofComponent = 7,
    /// A G1 point has a malformed internal structure (e.g. one coordinate is zero).
    MalformedG1Point = 8,
    /// The G2 point has a malformed internal structure (e.g. a limb is zero).
    MalformedG2Point = 9,
    /// A public-input element is all zeros.
    ZeroedPublicInput = 10,
    /// Cross-contract proof deserialization produced structurally invalid data.
    MalformedProofData = 11,
}

/// Map low-level proof validation errors into contract-level errors.
impl From<ProofValidationError> for ContractError {
    fn from(e: ProofValidationError) -> Self {
        match e {
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
///
/// This performs lightweight structural checks on the `AccessRequest` envelope.
/// Deeper proof-component validation (zeroed, oversized, malformed coordinates)
/// is delegated to [`Bn254Verifier::validate_proof_components`] which runs
/// inside `verify_proof` and returns granular [`ProofValidationError`] variants
/// that are mapped to [`ContractError`] via the `From` impl.
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

    /// Propose a new admin address. Only the current admin can call this.
    /// The new admin must call `accept_admin` to complete the transfer.
    pub fn propose_admin(
        env: Env,
        current_admin: Address,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &current_admin)?;

        env.storage().instance().set(&PENDING_ADMIN, &new_admin);

        events::publish_admin_transfer_proposed(&env, current_admin, new_admin);

        Ok(())
    }

    /// Accept the pending admin transfer. Only the proposed new admin can call this.
    /// Completes the two-step admin transfer process.
    pub fn accept_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        new_admin.require_auth();

        let pending: Address = env
            .storage()
            .instance()
            .get(&PENDING_ADMIN)
            .ok_or(ContractError::InvalidConfig)?;

        if new_admin != pending {
            return Err(ContractError::Unauthorized);
        }

        let old_admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::Unauthorized)?;

        env.storage().instance().set(&ADMIN, &new_admin);
        env.storage().instance().remove(&PENDING_ADMIN);

        events::publish_admin_transfer_accepted(&env, old_admin, new_admin);

        Ok(())
    }

    /// Cancel a pending admin transfer. Only the current admin can call this.
    pub fn cancel_admin_transfer(
        env: Env,
        current_admin: Address,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &current_admin)?;

        let pending: Address = env
            .storage()
            .instance()
            .get(&PENDING_ADMIN)
            .ok_or(ContractError::InvalidConfig)?;

        env.storage().instance().remove(&PENDING_ADMIN);

        events::publish_admin_transfer_cancelled(&env, current_admin, pending);

        Ok(())
    }

    /// Get the pending admin address, if any.
    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&PENDING_ADMIN)
    }

    /// Set the Groth16 verification key (admin-only).
    /// This stores the VK for later use in proof verification.
    pub fn set_verification_key(
        env: Env,
        caller: Address,
        vk: VerificationKey,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller)?;
        env.storage().instance().set(&VK, &vk);
        Ok(())
    }

    /// Get the stored Groth16 verification key.
    /// Returns the VK if it has been set, or None if not yet configured.
    pub fn get_verification_key(env: Env) -> Option<VerificationKey> {
        env.storage().instance().get(&VK)
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

    /// Verifies a ZK proof for resource access.
    ///
    /// This is the primary entry point for users to gain access to protected resources.
    /// It performs the following steps:
    /// 1. Authorizes the user.
    /// 2. Validates the request shape.
    /// 3. Checks whitelist and rate limits.
    /// 4. Verifies the Groth16 proof via `Bn254Verifier`.
    /// 5. Logs the access in the `AuditTrail` if successful.
    ///
    /// Returns `true` if the proof is valid and all checks pass, otherwise returns an error or `false`.
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

    /// Retrieves an audit record for a specific user and resource.
    ///
    /// Returns the `AuditRecord` if it exists, otherwise `None`.
    pub fn get_audit_record(
        env: Env,
        user: Address,
        resource_id: BytesN<32>,
    ) -> Option<AuditRecord> {
        AuditTrail::get_record(&env, user, resource_id)
    }
}
