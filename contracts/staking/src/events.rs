#![allow(deprecated)] // events().publish migration tracked separately

use soroban_sdk::{symbol_short, Address, Env, String};

// ── Event payloads ──────────────────────────────────────────────────────────

/// Fired once when the contract is bootstrapped.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializedEvent {
    pub admin: Address,
    pub stake_token: Address,
    pub reward_token: Address,
    pub reward_rate: i128,
    pub lock_period: u64,
    pub timestamp: u64,
}

/// Fired when a user deposits stake.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakedEvent {
    pub staker: Address,
    pub amount: i128,
    pub new_total_staked: i128,
    pub timestamp: u64,
}

/// Fired when a user queues an unstake request.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnstakeRequestedEvent {
    pub request_id: u64,
    pub staker: Address,
    pub amount: i128,
    pub unlock_at: u64,
    pub timestamp: u64,
}

/// Fired when a user withdraws after the timelock expires.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawnEvent {
    pub request_id: u64,
    pub staker: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Fired when a user claims accumulated rewards.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardClaimedEvent {
    pub staker: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Fired when the admin changes the reward rate.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardRateSetEvent {
    pub new_rate: i128,
    pub timestamp: u64,
}

/// Fired when a reward-rate change is proposed with a delay.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardRateProposedEvent {
    pub new_rate: i128,
    pub effective_at: u64,
    pub timestamp: u64,
}

/// Fired when the rate-change delay is updated.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateChangeDelaySetEvent {
    pub delay: u64,
    pub timestamp: u64,
}

/// Fired when the admin changes the lock period.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockPeriodSetEvent {
    pub new_period: u64,
    pub timestamp: u64,
}

/// Fired when a delayed reward-rate update is proposed.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardRateProposedEvent {
    pub new_rate: i128,
    pub effective_at: u64,
    pub timestamp: u64,
}

/// Fired when a delayed reward-rate update is applied.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardRateAppliedEvent {
    pub new_rate: i128,
    pub timestamp: u64,
}

/// Fired when the reward-rate delay configuration changes.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateChangeDelaySetEvent {
    pub delay: u64,
    pub timestamp: u64,
}

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

/// Fired when an unauthorized action is attempted.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessViolationEvent {
    pub caller: Address,
    pub action: String,
    pub required_permission: String,
    pub timestamp: u64,
}

/// Fired when a reward-rate change is proposed with a delay.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardRateProposedEvent {
    pub new_rate: i128,
    pub effective_at: u64,
    pub timestamp: u64,
}

/// Fired when the rate-change delay is updated.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateChangeDelaySetEvent {
    pub delay: u64,
    pub timestamp: u64,
}

// ── Publishers ──────────────────────────────────────────────────────────────

pub fn publish_initialized(
    env: &Env,
    admin: Address,
    stake_token: Address,
    reward_token: Address,
    reward_rate: i128,
    lock_period: u64,
) {
    env.events().publish(
        (symbol_short!("INIT"),),
        InitializedEvent {
            admin,
            stake_token,
            reward_token,
            reward_rate,
            lock_period,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_staked(env: &Env, staker: Address, amount: i128, new_total_staked: i128) {
    env.events().publish(
        (symbol_short!("STAKED"), staker.clone()),
        StakedEvent {
            staker,
            amount,
            new_total_staked,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_unstake_requested(
    env: &Env,
    request_id: u64,
    staker: Address,
    amount: i128,
    unlock_at: u64,
) {
    env.events().publish(
        (symbol_short!("UNSTK_REQ"), staker.clone()),
        UnstakeRequestedEvent {
            request_id,
            staker,
            amount,
            unlock_at,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_withdrawn(env: &Env, request_id: u64, staker: Address, amount: i128) {
    env.events().publish(
        (symbol_short!("WITHDRAWN"), staker.clone()),
        WithdrawnEvent {
            request_id,
            staker,
            amount,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_reward_claimed(env: &Env, staker: Address, amount: i128) {
    env.events().publish(
        (symbol_short!("CLMD"), staker.clone()),
        RewardClaimedEvent {
            staker,
            amount,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_reward_rate_set(env: &Env, new_rate: i128) {
    env.events().publish(
        (symbol_short!("RWD_RATE"),),
        RewardRateSetEvent {
            new_rate,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_reward_rate_proposed(env: &Env, new_rate: i128, effective_at: u64) {
    env.events().publish(
        (symbol_short!("RWD_PROP"),),
        (new_rate, effective_at, env.ledger().timestamp()),
    );
}

pub fn publish_reward_rate_applied(env: &Env, new_rate: i128) {
    env.events().publish(
        (symbol_short!("RWD_APLD"),),
        (new_rate, env.ledger().timestamp()),
    );
}

pub fn publish_rate_change_delay_set(env: &Env, delay: u64) {
    env.events().publish(
        (symbol_short!("DLY_SET"),),
        (delay, env.ledger().timestamp()),
    );
}

pub fn publish_lock_period_set(env: &Env, new_period: u64) {
    env.events().publish(
        (symbol_short!("LOCK_SET"),),
        LockPeriodSetEvent {
            new_period,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_reward_rate_proposed(env: &Env, new_rate: i128, effective_at: u64) {
    env.events().publish(
        (symbol_short!("RATE_PROP"),),
        RewardRateProposedEvent {
            new_rate,
            effective_at,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_reward_rate_applied(env: &Env, new_rate: i128) {
    env.events().publish(
        (symbol_short!("RATE_APLY"),),
        RewardRateAppliedEvent {
            new_rate,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_rate_change_delay_set(env: &Env, delay: u64) {
    env.events().publish(
        (symbol_short!("RATE_DLY"),),
        RateChangeDelaySetEvent {
            delay,
            timestamp: env.ledger().timestamp(),
        },
    );
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

pub fn publish_access_violation(
    env: &Env,
    caller: Address,
    action: String,
    required_permission: String,
) {
    env.events().publish(
        (symbol_short!("ACC_VIOL"), caller.clone(), action.clone()),
        AccessViolationEvent {
            caller,
            action,
            required_permission,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_reward_rate_proposed(env: &Env, new_rate: i128, effective_at: u64) {
    env.events().publish(
        (symbol_short!("RWD_PROP"),),
        RewardRateProposedEvent {
            new_rate,
            effective_at,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_reward_rate_applied(env: &Env, new_rate: i128) {
    env.events().publish(
        (symbol_short!("RWD_APPL"),),
        RewardRateSetEvent {
            new_rate,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_rate_change_delay_set(env: &Env, delay: u64) {
    env.events().publish(
        (symbol_short!("DLY_SET"),),
        RateChangeDelaySetEvent {
            delay,
            timestamp: env.ledger().timestamp(),
        },
    );
}
