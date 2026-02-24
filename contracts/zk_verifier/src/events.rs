use soroban_sdk::{symbol_short, Address, Env};

/// Fired when an admin transfer is proposed.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferProposedEvent {
    pub current_admin: Address,
    pub proposed_admin: Address,
    pub timestamp: u64,
}

/// Fired when an admin transfer is accepted.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferAcceptedEvent {
    pub old_admin: Address,
    pub new_admin: Address,
    pub timestamp: u64,
}

/// Fired when a pending admin transfer is cancelled.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferCancelledEvent {
    pub admin: Address,
    pub cancelled_proposed: Address,
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
