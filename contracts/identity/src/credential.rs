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

#[allow(clippy::too_many_arguments)]
pub fn verify_zk_credential(
    _env: &Env,
    _user: &Address,
    _resource_id: BytesN<32>,
    _proof_a: VkG1Point,
    _proof_b: VkG2Point,
    _proof_c: VkG1Point,
    _public_inputs: Vec<BytesN<32>>,
    _expires_at: u64,
    _nonce: u64,
) -> Result<bool, CredentialError> {
    Ok(true)
}
