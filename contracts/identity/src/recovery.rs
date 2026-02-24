#![allow(clippy::arithmetic_side_effects)]

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

// ── Constants ────────────────────────────────────────────────────────────────

const MIN_GUARDIANS: u32 = 3;
const MAX_GUARDIANS: u32 = 5;
const COOLDOWN_PERIOD: u64 = 172_800; // 48 hours in seconds

const TTL_THRESHOLD: u32 = 5_184_000;
const TTL_EXTEND_TO: u32 = 10_368_000;

// ── Storage key symbols ──────────────────────────────────────────────────────

const GUARDIANS: Symbol = symbol_short!("GUARD");
const REC_THR: Symbol = symbol_short!("REC_THR");
const REC_REQ: Symbol = symbol_short!("REC_REQ");
const OWN_ACT: Symbol = symbol_short!("OWN_ACT");

// ── Errors ───────────────────────────────────────────────────────────────────

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RecoveryError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    MaxGuardiansReached = 4,
    DuplicateGuardian = 5,
    GuardianNotFound = 6,
    InvalidThreshold = 7,
    InsufficientGuardians = 8,
    NotAGuardian = 9,
    RecoveryAlreadyActive = 10,
    NoActiveRecovery = 11,
    AlreadyApproved = 12,
    InsufficientApprovals = 13,
    CooldownNotExpired = 14,
    OwnerDeactivated = 15,
}

// ── Types ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct RecoveryRequest {
    pub old_address: Address,
    pub new_address: Address,
    pub approvals: Vec<Address>,
    pub initiated_at: u64,
    pub execute_after: u64,
}

// ── Storage key helpers ──────────────────────────────────────────────────────

fn guardians_key(owner: &Address) -> (Symbol, Address) {
    (GUARDIANS, owner.clone())
}

fn threshold_key(owner: &Address) -> (Symbol, Address) {
    (REC_THR, owner.clone())
}

fn recovery_key(owner: &Address) -> (Symbol, Address) {
    (REC_REQ, owner.clone())
}

fn owner_active_key(owner: &Address) -> (Symbol, Address) {
    (OWN_ACT, owner.clone())
}

fn extend_ttl(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Owner status ─────────────────────────────────────────────────────────────

pub fn set_owner_active(env: &Env, owner: &Address) {
    let key = owner_active_key(owner);
    env.storage().persistent().set(&key, &true);
    extend_ttl(env, &key);
}

pub fn is_owner_active(env: &Env, owner: &Address) -> bool {
    let key = owner_active_key(owner);
    env.storage().persistent().get(&key).unwrap_or(false)
}

pub fn deactivate_owner(env: &Env, owner: &Address) {
    let key = owner_active_key(owner);
    env.storage().persistent().set(&key, &false);
    extend_ttl(env, &key);
}

// ── Guardian management ──────────────────────────────────────────────────────

pub fn get_guardians(env: &Env, owner: &Address) -> Vec<Address> {
    let key = guardians_key(owner);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env))
}

pub fn get_threshold(env: &Env, owner: &Address) -> u32 {
    let key = threshold_key(owner);
    env.storage().persistent().get(&key).unwrap_or(0)
}

pub fn add_guardian(
    env: &Env,
    owner: &Address,
    guardian: Address,
) -> Result<(), RecoveryError> {
    let key = guardians_key(owner);
    let mut guardians: Vec<Address> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));

    if guardians.len() >= MAX_GUARDIANS {
        return Err(RecoveryError::MaxGuardiansReached);
    }

    if guardians.contains(&guardian) {
        return Err(RecoveryError::DuplicateGuardian);
    }

    guardians.push_back(guardian);
    env.storage().persistent().set(&key, &guardians);
    extend_ttl(env, &key);

    Ok(())
}

pub fn remove_guardian(
    env: &Env,
    owner: &Address,
    guardian: &Address,
) -> Result<(), RecoveryError> {
    let key = guardians_key(owner);
    let guardians: Vec<Address> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));

    if !guardians.contains(guardian) {
        return Err(RecoveryError::GuardianNotFound);
    }

    let mut new_guardians = Vec::new(env);
    for i in 0..guardians.len() {
        if let Some(g) = guardians.get(i) {
            if g != *guardian {
                new_guardians.push_back(g);
            }
        }
    }

    env.storage().persistent().set(&key, &new_guardians);
    extend_ttl(env, &key);

    Ok(())
}

pub fn set_threshold(
    env: &Env,
    owner: &Address,
    threshold: u32,
) -> Result<(), RecoveryError> {
    let guardians = get_guardians(env, owner);
    if threshold == 0 || threshold > guardians.len() {
        return Err(RecoveryError::InvalidThreshold);
    }

    let key = threshold_key(owner);
    env.storage().persistent().set(&key, &threshold);
    extend_ttl(env, &key);

    Ok(())
}

// ── Recovery lifecycle ───────────────────────────────────────────────────────

pub fn initiate_recovery(
    env: &Env,
    guardian: &Address,
    owner: &Address,
    new_address: Address,
) -> Result<(), RecoveryError> {
    let guardians = get_guardians(env, owner);

    if !guardians.contains(guardian) {
        return Err(RecoveryError::NotAGuardian);
    }

    if guardians.len() < MIN_GUARDIANS {
        return Err(RecoveryError::InsufficientGuardians);
    }

    let key = recovery_key(owner);
    if env.storage().persistent().has(&key) {
        return Err(RecoveryError::RecoveryAlreadyActive);
    }

    let now = env.ledger().timestamp();
    let mut approvals = Vec::new(env);
    approvals.push_back(guardian.clone());

    let request = RecoveryRequest {
        old_address: owner.clone(),
        new_address,
        approvals,
        initiated_at: now,
        execute_after: now + COOLDOWN_PERIOD,
    };

    env.storage().persistent().set(&key, &request);
    extend_ttl(env, &key);

    Ok(())
}

pub fn approve_recovery(
    env: &Env,
    guardian: &Address,
    owner: &Address,
) -> Result<(), RecoveryError> {
    let guardians = get_guardians(env, owner);

    if !guardians.contains(guardian) {
        return Err(RecoveryError::NotAGuardian);
    }

    let key = recovery_key(owner);
    let mut request: RecoveryRequest = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(RecoveryError::NoActiveRecovery)?;

    if request.approvals.contains(guardian) {
        return Err(RecoveryError::AlreadyApproved);
    }

    request.approvals.push_back(guardian.clone());
    env.storage().persistent().set(&key, &request);
    extend_ttl(env, &key);

    Ok(())
}

pub fn execute_recovery(
    env: &Env,
    owner: &Address,
) -> Result<Address, RecoveryError> {
    let key = recovery_key(owner);
    let request: RecoveryRequest = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(RecoveryError::NoActiveRecovery)?;

    let threshold = get_threshold(env, owner);
    if threshold == 0 {
        return Err(RecoveryError::InvalidThreshold);
    }

    if request.approvals.len() < threshold {
        return Err(RecoveryError::InsufficientApprovals);
    }

    let now = env.ledger().timestamp();
    if now < request.execute_after {
        return Err(RecoveryError::CooldownNotExpired);
    }

    let new_address = request.new_address.clone();

    // Deactivate old address
    deactivate_owner(env, owner);

    // Activate new address
    set_owner_active(env, &new_address);

    // Transfer guardian configuration to new address
    let guardians = get_guardians(env, owner);
    let new_guard_key = guardians_key(&new_address);
    env.storage().persistent().set(&new_guard_key, &guardians);
    extend_ttl(env, &new_guard_key);

    // Transfer threshold to new address
    let new_thr_key = threshold_key(&new_address);
    env.storage().persistent().set(&new_thr_key, &threshold);
    extend_ttl(env, &new_thr_key);

    // Clean up recovery request
    env.storage().persistent().remove(&key);

    Ok(new_address)
}

pub fn cancel_recovery(env: &Env, owner: &Address) -> Result<(), RecoveryError> {
    let key = recovery_key(owner);
    if !env.storage().persistent().has(&key) {
        return Err(RecoveryError::NoActiveRecovery);
    }

    env.storage().persistent().remove(&key);

    Ok(())
}

pub fn get_recovery_request(env: &Env, owner: &Address) -> Option<RecoveryRequest> {
    let key = recovery_key(owner);
    env.storage().persistent().get(&key)
}
