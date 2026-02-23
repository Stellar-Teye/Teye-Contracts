#![no_std]

mod audit;
mod helpers;
mod verifier;

pub use crate::audit::{AuditRecord, AuditTrail};
pub use crate::helpers::ZkAccessHelper;
pub use crate::verifier::{Bn254Verifier, PoseidonHasher, Proof};

use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env, Vec};

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
    pub fn verify_access(env: Env, request: AccessRequest) -> Result<bool, ContractError> {
        request.user.require_auth();

        validate_request(&request)?;

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
