#![allow(clippy::arithmetic_side_effects)]

use soroban_sdk::{contracttype, Address, BytesN, String, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultShare {
    pub guardian: Address,
    pub x: u32,
    pub y: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultPolicy {
    pub threshold: u32,
    pub shard_count: u32,
    pub emergency_threshold: u32,
    pub inactivity_timeout_secs: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultRecord {
    pub owner: Address,
    pub epoch: u32,
    pub policy: VaultPolicy,
    pub data_ref_hash: String,
    pub created_at: u64,
    pub last_activity_at: u64,
    pub deadman_release_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyApproval {
    pub owner: Address,
    pub guardian: Address,
    pub approved_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyReconstructionRequest {
    pub owner: Address,
    pub requester: Address,
    pub submitted_shares: Vec<VaultShare>,
    pub created_at: u64,
}
