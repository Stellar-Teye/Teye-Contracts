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
}

pub fn set_zk_verifier(env: &Env, verifier_id: &Address) {
    env.storage().instance().set(&ZK_VERIFIER, verifier_id);
}

pub fn get_zk_verifier(env: &Env) -> Option<Address> {
    env.storage().instance().get(&ZK_VERIFIER)
}

use zk_verifier::{
    verifier::{G1Point, G2Point, Proof},
    AccessRequest, ZkVerifierContractClient,
};

#[allow(clippy::too_many_arguments)]
pub fn verify_zk_credential(
    env: &Env,
    user: &Address,
    resource_id: BytesN<32>,
    proof_a: VkG1Point,
    proof_b: VkG2Point,
    proof_c: VkG1Point,
    public_inputs: Vec<BytesN<32>>,
    expires_at: u64,
    nonce: u64,
) -> Result<bool, CredentialError> {
    let verifier_id = get_zk_verifier(env).ok_or(CredentialError::VerifierNotSet)?;

    if proof_a.len() != 64 || proof_b.len() != 128 || proof_c.len() != 64 {
        return Err(CredentialError::ZkVerificationFailed);
    }

    let mut a_buf = [0u8; 64];
    proof_a.copy_into_slice(&mut a_buf);
    let mut b_buf = [0u8; 128];
    proof_b.copy_into_slice(&mut b_buf);
    let mut c_buf = [0u8; 64];
    proof_c.copy_into_slice(&mut c_buf);

    let a_x = BytesN::from_array(env, &a_buf[0..32].try_into().unwrap());
    let a_y = BytesN::from_array(env, &a_buf[32..64].try_into().unwrap());

    let b_x0 = BytesN::from_array(env, &b_buf[0..32].try_into().unwrap());
    let b_x1 = BytesN::from_array(env, &b_buf[32..64].try_into().unwrap());
    let b_y0 = BytesN::from_array(env, &b_buf[64..96].try_into().unwrap());
    let b_y1 = BytesN::from_array(env, &b_buf[96..128].try_into().unwrap());

    let c_x = BytesN::from_array(env, &c_buf[0..32].try_into().unwrap());
    let c_y = BytesN::from_array(env, &c_buf[32..64].try_into().unwrap());

    let proof = Proof {
        a: G1Point { x: a_x, y: a_y },
        b: G2Point {
            x: (b_x0, b_x1),
            y: (b_y0, b_y1),
        },
        c: G1Point { x: c_x, y: c_y },
    };

    let request = AccessRequest {
        user: user.clone(),
        resource_id,
        proof,
        public_inputs,
        expires_at,
        nonce,
    };

    let client = ZkVerifierContractClient::new(env, &verifier_id);
    let result = client.try_verify_access(&request);

    match result {
        Ok(Ok(true)) => Ok(true),
        _ => Err(CredentialError::ZkVerificationFailed), // Any failure maps to VerificationFailed
    }
}
