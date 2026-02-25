//! Structured event emitting for the identity contract.
//!
//! These functions emit events in a format compatible with the `events` contract
//! streaming system. Each publishes under a hierarchical topic so that external
//! subscribers can filter using wildcard patterns (e.g. `identity.*`).

#![allow(deprecated)] // events().publish migration tracked separately

use soroban_sdk::{contracttype, symbol_short, Address, Env};

// ── Event payloads ───────────────────────────────────────────────────────────

/// Fired when an identity owner is activated or deactivated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerStatusChangedEvent {
    pub owner: Address,
    pub active: bool,
    pub timestamp: u64,
}

/// Fired when a guardian is added or removed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardianChangedEvent {
    pub owner: Address,
    pub guardian: Address,
    pub added: bool,
    pub timestamp: u64,
}

/// Fired when a recovery process is initiated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryInitiatedEvent {
    pub owner: Address,
    pub new_address: Address,
    pub initiated_by: Address,
    pub timestamp: u64,
}

/// Fired when a recovery process is executed successfully.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryExecutedEvent {
    pub old_address: Address,
    pub new_address: Address,
    pub timestamp: u64,
}

/// Fired when a recovery process is cancelled.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryCancelledEvent {
    pub owner: Address,
    pub timestamp: u64,
}

/// Fired when a ZK credential is verified.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZkCredentialVerifiedEvent {
    pub user: Address,
    pub verified: bool,
    pub timestamp: u64,
}

// ── Publishers ───────────────────────────────────────────────────────────────

/// Emit a streaming event when identity ownership status changes.
pub fn emit_owner_status_changed(env: &Env, owner: Address, active: bool) {
    env.events().publish(
        (symbol_short!("STREAM"), symbol_short!("ID_STAT")),
        OwnerStatusChangedEvent {
            owner,
            active,
            timestamp: env.ledger().timestamp(),
        },
    );
}

/// Emit a streaming event when a guardian is added or removed.
pub fn emit_guardian_changed(env: &Env, owner: Address, guardian: Address, added: bool) {
    env.events().publish(
        (symbol_short!("STREAM"), symbol_short!("ID_GUARD")),
        GuardianChangedEvent {
            owner,
            guardian,
            added,
            timestamp: env.ledger().timestamp(),
        },
    );
}

/// Emit a streaming event when recovery is initiated.
pub fn emit_recovery_initiated(
    env: &Env,
    owner: Address,
    new_address: Address,
    initiated_by: Address,
) {
    env.events().publish(
        (symbol_short!("STREAM"), symbol_short!("ID_RINIT")),
        RecoveryInitiatedEvent {
            owner,
            new_address,
            initiated_by,
            timestamp: env.ledger().timestamp(),
        },
    );
}

/// Emit a streaming event when recovery is executed.
pub fn emit_recovery_executed(env: &Env, old_address: Address, new_address: Address) {
    env.events().publish(
        (symbol_short!("STREAM"), symbol_short!("ID_REXEC")),
        RecoveryExecutedEvent {
            old_address,
            new_address,
            timestamp: env.ledger().timestamp(),
        },
    );
}

/// Emit a streaming event when recovery is cancelled.
pub fn emit_recovery_cancelled(env: &Env, owner: Address) {
    env.events().publish(
        (symbol_short!("STREAM"), symbol_short!("ID_RCNCL")),
        RecoveryCancelledEvent {
            owner,
            timestamp: env.ledger().timestamp(),
        },
    );
}

/// Emit a streaming event when a ZK credential is verified.
pub fn emit_zk_credential_verified(env: &Env, user: Address, verified: bool) {
    env.events().publish(
        (symbol_short!("STREAM"), symbol_short!("ID_ZKCRD")),
        ZkCredentialVerifiedEvent {
            user,
            verified,
            timestamp: env.ledger().timestamp(),
        },
    );
}
