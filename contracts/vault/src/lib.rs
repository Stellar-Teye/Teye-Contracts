#![no_std]
#![allow(clippy::too_many_arguments)]

extern crate alloc;
use alloc::vec::Vec as StdVec;

pub mod encryption;
pub mod key_rotation;
pub mod sss;

#[cfg(test)]
mod test;

use common::{VaultPolicy, VaultRecord, VaultShare};
use identity::IdentityContractClient;
use key_rotation::RotationEvent;
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, String, Symbol, Vec};

const ADMIN: Symbol = symbol_short!("ADMIN");
const IDENTITY: Symbol = symbol_short!("IDENTITY");
const INIT: Symbol = symbol_short!("INIT");
const VAULT: Symbol = symbol_short!("VAULT");
const SHARE: Symbol = symbol_short!("SHARE");
const APR: Symbol = symbol_short!("EM_APR");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VaultError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidConfig = 4,
    VaultNotFound = 5,
    InsufficientShares = 6,
    InvalidShare = 7,
    EmergencyThresholdNotMet = 8,
    DeadmanNotReady = 9,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultSnapshot {
    pub record: VaultRecord,
    pub shard_holders: Vec<Address>,
}

#[contract]
pub struct VaultContract;

#[contractimpl]
impl VaultContract {
    pub fn initialize(env: Env, admin: Address, identity_contract: Address) -> Result<(), VaultError> {
        if env.storage().instance().has(&INIT) {
            return Err(VaultError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&IDENTITY, &identity_contract);
        env.storage().instance().set(&INIT, &true);
        Ok(())
    }

    pub fn configure_vault(
        env: Env,
        owner: Address,
        threshold: u32,
        shard_count: u32,
        emergency_threshold: u32,
        inactivity_timeout_secs: u64,
        data_ref_hash: String,
        secret_seed: BytesN<32>,
    ) -> Result<VaultSnapshot, VaultError> {
        Self::require_init(&env)?;
        owner.require_auth();

        if threshold == 0
            || shard_count == 0
            || threshold > shard_count
            || emergency_threshold < threshold
            || emergency_threshold > shard_count
        {
            return Err(VaultError::InvalidConfig);
        }

        let identity: Address = env.storage().instance().get(&IDENTITY).ok_or(VaultError::NotInitialized)?;
        let id_client = IdentityContractClient::new(&env, &identity);
        let guardians = id_client.get_guardians(&owner);
        if guardians.len() < shard_count {
            return Err(VaultError::InvalidConfig);
        }

        let policy = VaultPolicy {
            threshold,
            shard_count,
            emergency_threshold,
            inactivity_timeout_secs,
        };

        let now = env.ledger().timestamp();
        let record = VaultRecord {
            owner: owner.clone(),
            epoch: 1,
            policy: policy.clone(),
            data_ref_hash,
            created_at: now,
            last_activity_at: now,
            deadman_release_at: now.saturating_add(inactivity_timeout_secs),
        };
        env.storage().persistent().set(&(VAULT, owner.clone()), &record);

        let shares = sss::split(
            secret_seed.to_array(),
            threshold as u8,
            shard_count as u8,
            env.crypto().sha256(&secret_seed.into()).to_array(),
        );

        let mut holders = Vec::new(&env);
        for i in 0..shard_count {
            let guardian = guardians.get(i).ok_or(VaultError::InvalidConfig)?;
            let share = shares.get(i as usize).ok_or(VaultError::InvalidConfig)?;
            let s = VaultShare {
                guardian: guardian.clone(),
                x: share.x as u32,
                y: BytesN::from_array(&env, &share.y),
            };
            env.storage().persistent().set(&(SHARE, owner.clone(), record.epoch, guardian.clone()), &s);
            holders.push_back(guardian);
        }

        Ok(VaultSnapshot { record, shard_holders: holders })
    }

    pub fn reconstruct_key(
        env: Env,
        requester: Address,
        owner: Address,
        shares: Vec<VaultShare>,
    ) -> Result<BytesN<32>, VaultError> {
        Self::require_init(&env)?;
        requester.require_auth();

        let record: VaultRecord = env.storage().persistent().get(&(VAULT, owner.clone())).ok_or(VaultError::VaultNotFound)?;
        if shares.len() < record.policy.threshold {
            return Err(VaultError::InsufficientShares);
        }

        let mut verified = StdVec::new();
        for i in 0..record.policy.threshold {
            let s = shares.get(i).ok_or(VaultError::InsufficientShares)?;
            let stored: VaultShare = env
                .storage()
                .persistent()
                .get(&(SHARE, owner.clone(), record.epoch, s.guardian.clone()))
                .ok_or(VaultError::InvalidShare)?;
            if stored.x != s.x || stored.y != s.y {
                return Err(VaultError::InvalidShare);
            }
            verified.push(sss::Share { x: s.x as u8, y: s.y.to_array() });
        }

        let secret = sss::reconstruct(&verified, record.policy.threshold as u8).ok_or(VaultError::InsufficientShares)?;
        Ok(BytesN::from_array(&env, &secret))
    }

    pub fn submit_emergency_approval(env: Env, guardian: Address, owner: Address) -> Result<(), VaultError> {
        Self::require_init(&env)?;
        guardian.require_auth();

        let record: VaultRecord = env.storage().persistent().get(&(VAULT, owner.clone())).ok_or(VaultError::VaultNotFound)?;
        let stored: Option<VaultShare> = env.storage().persistent().get(&(SHARE, owner.clone(), record.epoch, guardian.clone()));
        if stored.is_none() {
            return Err(VaultError::Unauthorized);
        }
        env.storage().persistent().set(&(APR, owner, guardian), &true);
        Ok(())
    }

    pub fn emergency_reconstruct(env: Env, owner: Address) -> Result<BytesN<32>, VaultError> {
        Self::require_init(&env)?;

        let record: VaultRecord = env.storage().persistent().get(&(VAULT, owner.clone())).ok_or(VaultError::VaultNotFound)?;
        let identity: Address = env.storage().instance().get(&IDENTITY).ok_or(VaultError::NotInitialized)?;
        let id_client = IdentityContractClient::new(&env, &identity);
        let guardians = id_client.get_guardians(&owner);

        let mut approved = 0u32;
        let mut shares = StdVec::new();
        for i in 0..guardians.len() {
            if let Some(g) = guardians.get(i) {
                let ok: bool = env.storage().persistent().get(&(APR, owner.clone(), g.clone())).unwrap_or(false);
                if ok {
                    approved = approved.saturating_add(1);
                    if let Some(s) = env.storage().persistent().get::<_, VaultShare>(&(SHARE, owner.clone(), record.epoch, g.clone())) {
                        shares.push(sss::Share { x: s.x as u8, y: s.y.to_array() });
                    }
                }
            }
        }

        if approved < record.policy.emergency_threshold {
            return Err(VaultError::EmergencyThresholdNotMet);
        }

        let secret = sss::reconstruct(&shares, record.policy.threshold as u8).ok_or(VaultError::InsufficientShares)?;
        Ok(BytesN::from_array(&env, &secret))
    }

    pub fn rotate_key(env: Env, owner: Address, new_seed: BytesN<32>) -> Result<RotationEvent, VaultError> {
        Self::require_init(&env)?;
        owner.require_auth();

        let mut record: VaultRecord = env.storage().persistent().get(&(VAULT, owner.clone())).ok_or(VaultError::VaultNotFound)?;
        let identity: Address = env.storage().instance().get(&IDENTITY).ok_or(VaultError::NotInitialized)?;
        let id_client = IdentityContractClient::new(&env, &identity);
        let guardians = id_client.get_guardians(&owner);

        let old_epoch = record.epoch;
        let new_epoch = old_epoch.saturating_add(1);
        let shares = sss::split(
            new_seed.to_array(),
            record.policy.threshold as u8,
            record.policy.shard_count as u8,
            env.crypto().sha256(&new_seed.into()).to_array(),
        );

        for i in 0..record.policy.shard_count {
            let g = guardians.get(i).ok_or(VaultError::InvalidConfig)?;
            let share = shares.get(i as usize).ok_or(VaultError::InvalidConfig)?;
            env.storage().persistent().set(
                &(SHARE, owner.clone(), new_epoch, g.clone()),
                &VaultShare { guardian: g, x: share.x as u32, y: BytesN::from_array(&env, &share.y) },
            );
        }

        record.epoch = new_epoch;
        record.last_activity_at = env.ledger().timestamp();
        record.deadman_release_at = record.last_activity_at.saturating_add(record.policy.inactivity_timeout_secs);
        env.storage().persistent().set(&(VAULT, owner.clone()), &record);

        Ok(RotationEvent {
            owner,
            previous_epoch: old_epoch,
            new_epoch,
            rotated_at: record.last_activity_at,
        })
    }

    pub fn touch_activity(env: Env, owner: Address) -> Result<(), VaultError> {
        owner.require_auth();
        let mut record: VaultRecord = env.storage().persistent().get(&(VAULT, owner.clone())).ok_or(VaultError::VaultNotFound)?;
        record.last_activity_at = env.ledger().timestamp();
        record.deadman_release_at = record.last_activity_at.saturating_add(record.policy.inactivity_timeout_secs);
        env.storage().persistent().set(&(VAULT, owner), &record);
        Ok(())
    }

    pub fn trigger_deadman_release(env: Env, owner: Address) -> Result<bool, VaultError> {
        let record: VaultRecord = env.storage().persistent().get(&(VAULT, owner)).ok_or(VaultError::VaultNotFound)?;
        if env.ledger().timestamp() < record.deadman_release_at {
            return Err(VaultError::DeadmanNotReady);
        }
        Ok(true)
    }

    pub fn get_vault(env: Env, owner: Address) -> Result<VaultRecord, VaultError> {
        env.storage().persistent().get(&(VAULT, owner)).ok_or(VaultError::VaultNotFound)
    }

    fn require_init(env: &Env) -> Result<(), VaultError> {
        if !env.storage().instance().has(&INIT) {
            return Err(VaultError::NotInitialized);
        }
        Ok(())
    }
}
