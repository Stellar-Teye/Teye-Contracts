#![no_std]

pub mod audit;
pub mod credentials;
pub mod events;
pub mod helpers;
pub mod plonk;
pub mod revocation;
pub mod selective_disclosure;
pub mod verifier;
pub mod vk;

use soroban_sdk::{contracttype, Address, BytesN, Vec};

// Re-export commonly used types from verifier module
pub use verifier::{Bn254Verifier, G1Point, G2Point, PoseidonHasher, Proof, ZkVerifier};

/// Represents a ZK access request submitted for verification.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRequest {
    /// The user requesting access.
    pub user: Address,
    /// The resource being accessed.
    pub resource_id: BytesN<32>,
    /// The ZK proof.
    pub proof: Proof,
    /// Public inputs for the proof.
    pub public_inputs: Vec<BytesN<32>>,
    /// Expiration timestamp for this access request.
    pub expires_at: u64,
    /// Nonce for replay protection.
    pub nonce: u64,
}

/// Contract error types.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    InvalidProof = 1,
    ExpiredRequest = 2,
    InvalidNonce = 3,
    Unauthorized = 4,
}

/// Verification record for audit logging.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationRecord {
    pub submitter: Address,
    pub proof_id: u64,
    pub verified: bool,
    pub timestamp: u64,
}

// Re-export helper utilities
pub use helpers::{MerkleVerifier, ZkAccessHelper};
pub use vk::VerificationKey;
