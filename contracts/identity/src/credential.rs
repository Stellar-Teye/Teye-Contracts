#![allow(deprecated)]
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, Symbol, Vec};
type VkG1Point = Bytes;
type VkG2Point = Bytes;

const ZK_VERIFIER: Symbol = symbol_short!("ZK_VER");

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CredentialError {
    Unauthorized = 100,
    VerifierNotSet = 101,
    ZkVerificationFailed = 102,
    InvalidNonce = 103,
    CredentialExpired = 104,
}

pub fn set_zk_verifier(env: &Env, verifier_id: &Address) {
    env.storage().instance().set(&ZK_VERIFIER, verifier_id);
}

pub fn get_zk_verifier(env: &Env) -> Option<Address> {
    env.storage().instance().get(&ZK_VERIFIER)
}

pub fn verify_zk_credential(
    env: &Env,
    user: &Address,
    resource_id: BytesN<32>,
    proof_a: Bytes,
    proof_b: Bytes,
    proof_c: Bytes,
    public_inputs: Vec<BytesN<32>>,
    expires_at: u64,
    nonce: u64,
) -> Result<bool, CredentialError> {
    if env.ledger().timestamp() > expires_at {
        return Err(CredentialError::CredentialExpired);
    }

    let verifier_id = get_zk_verifier(env).ok_or(CredentialError::VerifierNotSet)?;
    let client = zk_verifier::ZkVerifierContractClient::new(env, &verifier_id);

    // Reconstruct the proof points from raw bytes.
    // The proof bytes are expected to be in G1 (64 bytes: 32x, 32y) and G2 (128 bytes: 32x0, 32x1, 32y0, 32y1) format.
    let proof = zk_verifier::Proof {
        a: zk_verifier::vk::G1Point {
            x: BytesN::from_array(env, &proof_a.to_array()[0..32].try_into().map_err(|_| CredentialError::ZkVerificationFailed)?),
            y: BytesN::from_array(env, &proof_a.to_array()[32..64].try_into().map_err(|_| CredentialError::ZkVerificationFailed)?),
        },
        b: zk_verifier::vk::G2Point {
            x: (
                BytesN::from_array(env, &proof_b.to_array()[0..32].try_into().map_err(|_| CredentialError::ZkVerificationFailed)?),
                BytesN::from_array(env, &proof_b.to_array()[32..64].try_into().map_err(|_| CredentialError::ZkVerificationFailed)?),
            ),
            y: (
                BytesN::from_array(env, &proof_b.to_array()[64..96].try_into().map_err(|_| CredentialError::ZkVerificationFailed)?),
                BytesN::from_array(env, &proof_b.to_array()[96..128].try_into().map_err(|_| CredentialError::ZkVerificationFailed)?),
            ),
        },
        c: zk_verifier::vk::G1Point {
            x: BytesN::from_array(env, &proof_c.to_array()[0..32].try_into().map_err(|_| CredentialError::ZkVerificationFailed)?),
            y: BytesN::from_array(env, &proof_c.to_array()[32..64].try_into().map_err(|_| CredentialError::ZkVerificationFailed)?),
        },
    };

    let request = zk_verifier::AccessRequest {
        user: user.clone(),
        resource_id,
        proof,
        public_inputs,
        expires_at,
        nonce,
    };

    let is_valid = client.verify_access(&request);
    if is_valid {
        super::events::emit_zk_credential_verified(env, user.clone(), true);
    }
    Ok(is_valid)
}
