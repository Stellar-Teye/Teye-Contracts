#![allow(dead_code)]

pub mod verifier;
pub mod vk;

pub use verifier::{Bn254Verifier, Proof, ZkVerifier};

use soroban_sdk::{contracttype, Address, BytesN, Vec};

/// Request payload for ZK access verification.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRequest {
    pub user: Address,
    pub resource_id: BytesN<32>,
    pub proof: verifier::Proof,
    pub public_inputs: Vec<BytesN<32>>,
    pub expires_at: u64,
    pub nonce: u64,
}

mod helpers;
pub use helpers::ZkAccessHelper;
