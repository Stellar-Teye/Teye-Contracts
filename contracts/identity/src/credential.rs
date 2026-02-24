//! # ZK Credential Verification
//!
//! Privacy-preserving credential verification using zero-knowledge proofs.
//! Users can prove they hold a valid credential (e.g., "licensed optometrist")
//! without revealing the credential itself on-chain.
//!
//! Verification is delegated to the `zk_verifier` contract via a cross-contract
//! call to `verify_access`.

use soroban_sdk::{symbol_short, Address, BytesN, Env, Symbol, Vec};

// Re-use the proof type definitions from the zk_verifier crate.
use zk_verifier::vk::G1Point as VkG1Point;
use zk_verifier::vk::G2Point as VkG2Point;
use zk_verifier::{AccessRequest, ZkVerifierContractClient};

// ── Storage keys ─────────────────────────────────────────────────────────────

const ZK_VERIFIER: Symbol = symbol_short!("ZK_VER");

// ── Errors ───────────────────────────────────────────────────────────────────

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CredentialError {
    /// Caller is not the contract admin.
    Unauthorized = 100,
    /// No zk_verifier contract address has been configured.
    VerifierNotSet = 101,
    /// The ZK proof was structurally valid but verification returned false.
    ZkVerificationFailed = 102,
}

// ── Public helpers ───────────────────────────────────────────────────────────

/// Store the address of the deployed `zk_verifier` contract.
pub fn set_zk_verifier(env: &Env, verifier_id: &Address) {
    env.storage().instance().set(&ZK_VERIFIER, verifier_id);
}

/// Retrieve the stored `zk_verifier` contract address, if any.
pub fn get_zk_verifier(env: &Env) -> Option<Address> {
    env.storage().instance().get(&ZK_VERIFIER)
}

/// Verify a ZK credential proof by cross-calling the `zk_verifier` contract.
///
/// This function:
/// 1. Reads the stored verifier address.
/// 2. Builds an `AccessRequest` from the supplied Groth16 proof and public inputs.
/// 3. Invokes `ZkVerifierContract::verify_access` via cross-contract call.
/// 4. Emits a `ZK_CRED` event on success (containing only the user address and
///    resource hash — no credential data is leaked on-chain).
///
/// # Arguments
/// * `user` — The address claiming credential ownership.
/// * `resource_id` — An opaque 32-byte identifier for the credential type.
/// * `proof_a` — G1 point A of the Groth16 proof.
/// * `proof_b` — G2 point B of the Groth16 proof.
/// * `proof_c` — G1 point C of the Groth16 proof.
/// * `public_inputs` — Public input scalars for the proof circuit.
pub fn verify_zk_credential(
    env: &Env,
    user: &Address,
    resource_id: BytesN<32>,
    proof_a: VkG1Point,
    proof_b: VkG2Point,
    proof_c: VkG1Point,
    public_inputs: Vec<BytesN<32>>,
) -> Result<bool, CredentialError> {
    // 1. Load verifier contract address.
    let verifier_addr: Address = env
        .storage()
        .instance()
        .get(&ZK_VERIFIER)
        .ok_or(CredentialError::VerifierNotSet)?;

    // 2. Build the cross-contract proof types.
    let proof = zk_verifier::verifier::Proof {
        a: zk_verifier::verifier::G1Point {
            x: proof_a.x.clone(),
            y: proof_a.y.clone(),
        },
        b: zk_verifier::verifier::G2Point {
            x: (proof_b.x.0.clone(), proof_b.x.1.clone()),
            y: (proof_b.y.0.clone(), proof_b.y.1.clone()),
        },
        c: zk_verifier::verifier::G1Point {
            x: proof_c.x.clone(),
            y: proof_c.y.clone(),
        },
    };

    let request = AccessRequest {
        user: user.clone(),
        resource_id: resource_id.clone(),
        proof,
        public_inputs,
    };

    // 3. Cross-contract call to the zk_verifier.
    //    Use `try_verify_access` to catch any internal panics from BN254
    //    pairing operations (e.g., invalid curve points) — these are mapped
    //    to ZkVerificationFailed rather than aborting the transaction.
    let client = ZkVerifierContractClient::new(env, &verifier_addr);
    let is_valid = match client.try_verify_access(&request) {
        Ok(Ok(valid)) => valid,
        // The verifier contract returned a typed error or the call panicked.
        _ => return Err(CredentialError::ZkVerificationFailed),
    };

    // 4. Emit event on success (privacy-preserving: only user + resource hash).
    if is_valid {
        env.events()
            .publish((symbol_short!("ZK_CRED"), user.clone()), resource_id);
    }

    Ok(is_valid)
}
