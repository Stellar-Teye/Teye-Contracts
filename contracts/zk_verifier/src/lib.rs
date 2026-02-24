#![no_std]

mod audit;
mod helpers;
mod verifier;

pub use crate::audit::{AuditRecord, AuditTrail};
pub use crate::helpers::ZkAccessHelper;
pub use crate::verifier::{Bn254Verifier, PoseidonHasher, Proof};

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec,
};

const ADMIN: Symbol = symbol_short!("ADMIN");
const RATE_CFG: Symbol = symbol_short!("RATECFG");
const RATE_TRACK: Symbol = symbol_short!("RLTRK");

/// Maximum number of public inputs accepted per proof verification.
const MAX_PUBLIC_INPUTS: u32 = 16;

/// Contract errors for the ZK verifier.
#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    /// The public inputs vector is empty.
    EmptyPublicInputs = 1,
    /// Too many public inputs supplied.
    TooManyPublicInputs = 2,
    /// A proof component is all zeros (degenerate / trivially invalid).
    DegenerateProof = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRequest {
    pub user: Address,
    pub resource_id: BytesN<32>,
    pub proof: Proof,
    pub public_inputs: Vec<BytesN<32>>,
}

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    Unauthorized = 1,
    RateLimited = 2,
    InvalidConfig = 3,
}

#[contract]
pub struct ZkVerifierContract;

/// Return `true` if every byte in `data` is zero.
fn is_all_zeros<const N: usize>(data: &BytesN<N>) -> bool {
    let arr = data.to_array();
    let mut all_zero = true;
    let mut i = 0;
    while i < N {
        if arr[i] != 0 {
            all_zero = false;
            break;
        }
        i += 1;
    }
    all_zero
}

/// Validate the structural integrity of an [`AccessRequest`] before
/// performing the (expensive) cryptographic verification.
fn validate_request(request: &AccessRequest) -> Result<(), ContractError> {
    // Must have at least one public input.
    if request.public_inputs.is_empty() {
        return Err(ContractError::EmptyPublicInputs);
    }

    // Cap the number of public inputs to prevent excessive computation.
    if request.public_inputs.len() > MAX_PUBLIC_INPUTS {
        return Err(ContractError::TooManyPublicInputs);
    }

    // Reject degenerate proof components (all zero bytes).
    if is_all_zeros(&request.proof.a)
        || is_all_zeros(&request.proof.b)
        || is_all_zeros(&request.proof.c)
    {
        return Err(ContractError::DegenerateProof);
    }

    Ok(())
}

#[contractimpl]
impl ZkVerifierContract {
    /// One-time initialization to set the admin address.
    ///
    /// Subsequent calls are ignored once the admin is set.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&ADMIN) {
            return;
        }

        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
    }

    /// Configure per-address rate limiting for this contract.
    pub fn set_rate_limit_config(
        env: Env,
        caller: Address,
        max_requests_per_window: u64,
        window_duration_seconds: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        if max_requests_per_window == 0 || window_duration_seconds == 0 {
            return Err(ContractError::InvalidConfig);
        }

        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::Unauthorized)?;

        if caller != admin {
            return Err(ContractError::Unauthorized);
        }

        env.storage().instance().set(
            &RATE_CFG,
            &(max_requests_per_window, window_duration_seconds),
        );

        Ok(())
    }

    /// Return the current rate limiting configuration, if any.
    pub fn get_rate_limit_config(env: Env) -> Option<(u64, u64)> {
        env.storage().instance().get(&RATE_CFG)
    }

    fn check_and_update_rate_limit(env: &Env, user: &Address) -> Result<(), ContractError> {
        let cfg: Option<(u64, u64)> = env.storage().instance().get(&RATE_CFG);
        let (max_requests_per_window, window_duration_seconds) = match cfg {
            Some(c) => c,
            None => return Ok(()), // No config set -> unlimited
        };

        if max_requests_per_window == 0 || window_duration_seconds == 0 {
            // Explicitly disabled
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

        Self::check_and_update_rate_limit(&env, &request.user)?;

        let is_valid = Bn254Verifier::verify_proof(&env, &request.proof, &request.public_inputs);
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
