#![allow(deprecated)] // events().publish migration tracked separately

use soroban_sdk::{contracttype, symbol_short, Address, BytesN, Env};

/// Fired when an admin transfer is proposed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferProposedEvent {
    pub current_admin: Address,
    pub proposed_admin: Address,
    pub timestamp: u64,
}

/// Fired when an admin transfer is accepted.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferAcceptedEvent {
    pub old_admin: Address,
    pub new_admin: Address,
    pub timestamp: u64,
}

/// Fired when a pending admin transfer is cancelled.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferCancelledEvent {
    pub admin: Address,
    pub cancelled_proposed: Address,
    pub timestamp: u64,
}

/// Event payload for rejected access attempts.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRejectedEvent {
    pub user: Address,
    pub resource_id: BytesN<32>,
    pub error: u32,
    pub timestamp: u64,
}

pub fn publish_admin_transfer_proposed(env: &Env, current_admin: Address, proposed_admin: Address) {
    env.events().publish(
        (symbol_short!("ADM_PROP"), current_admin.clone()),
        AdminTransferProposedEvent {
            current_admin,
            proposed_admin,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_admin_transfer_accepted(env: &Env, old_admin: Address, new_admin: Address) {
    env.events().publish(
        (symbol_short!("ADM_ACPT"), new_admin.clone()),
        AdminTransferAcceptedEvent {
            old_admin,
            new_admin,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_admin_transfer_cancelled(env: &Env, admin: Address, cancelled_proposed: Address) {
    env.events().publish(
        (symbol_short!("ADM_CNCL"), admin.clone()),
        AdminTransferCancelledEvent {
            admin,
            cancelled_proposed,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_access_rejected(
    env: &Env,
    user: Address,
    resource_id: BytesN<32>,
    error: crate::ContractError,
) {
    env.events().publish(
        (symbol_short!("REJECT"), user.clone(), resource_id.clone()),
        AccessRejectedEvent {
            user,
            resource_id,
            error: error as u32,
            timestamp: env.ledger().timestamp(),
        },
    );
}
