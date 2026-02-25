//! # Property-Based Test Generators
//!
//! Composable `proptest` strategies for generating valid and adversarial inputs
//! across all Teye contract operations.
//!
//! ## Design Decisions
//!
//! - Generators produce *semantic* values (amounts, durations, action sequences),
//!   not raw bytes, so tests exercise real code paths rather than hitting
//!   deserialization errors.
//! - Edge-case weights are tuned: ~20% of values are boundary cases (0, 1,
//!   i128::MAX) to maximize bug-finding per test iteration.
//! - Action sequence generators model realistic user behaviour patterns to
//!   achieve higher state-space coverage than uniform random sampling.

extern crate std;

use proptest::prelude::*;
use std::vec::Vec;

// ── Scalar Generators ────────────────────────────────────────────────────────

/// Strategy for token amounts (i128), biased toward edge cases.
///
/// Distribution:
///   10% → 0
///   10% → 1
///   10% → MAX safe amount (10^15, realistic for Stellar 7-decimal tokens)
///   70% → uniform in [1, 10^15]
pub fn amount_strategy() -> impl Strategy<Value = i128> {
    prop_oneof![
        1 => Just(0i128),
        1 => Just(1i128),
        1 => Just(1_000_000_000_000_000i128),   // 10^15
        7 => (1i128..=1_000_000_000_000_000i128),
    ]
}

/// Strategy for strictly positive token amounts.
pub fn positive_amount_strategy() -> impl Strategy<Value = i128> {
    prop_oneof![
        1 => Just(1i128),
        1 => Just(1_000_000_000_000_000i128),
        8 => (1i128..=1_000_000_000_000_000i128),
    ]
}

/// Strategy for amounts that should be rejected (negative or zero).
pub fn invalid_amount_strategy() -> impl Strategy<Value = i128> {
    prop_oneof![
        5 => Just(0i128),
        3 => (-1_000_000i128..=-1i128),
        2 => Just(i128::MIN),
    ]
}

/// Strategy for reward rates, covering common contract configurations.
pub fn reward_rate_strategy() -> impl Strategy<Value = i128> {
    prop_oneof![
        1 => Just(0i128),
        1 => Just(1i128),
        2 => (1i128..=100i128),
        3 => (1i128..=1_000_000i128),
        1 => Just(1_000_000_000i128),
    ]
}

/// Strategy for time durations in seconds.
pub fn duration_strategy() -> impl Strategy<Value = u64> {
    prop_oneof![
        1 => Just(0u64),
        1 => Just(1u64),
        2 => (1u64..=3_600u64),         // up to 1 hour
        3 => (1u64..=86_400u64),        // up to 1 day
        2 => (1u64..=604_800u64),       // up to 1 week
        1 => Just(31_536_000u64),       // 1 year
    ]
}

/// Strategy for lock periods.
pub fn lock_period_strategy() -> impl Strategy<Value = u64> {
    prop_oneof![
        2 => Just(0u64),
        3 => (1u64..=86_400u64),
        3 => (86_400u64..=604_800u64),
        2 => Just(2_592_000u64),        // 30 days
    ]
}

/// Strategy for timestamps.
pub fn timestamp_strategy() -> impl Strategy<Value = u64> {
    prop_oneof![
        1 => Just(0u64),
        8 => (0u64..=31_536_000u64),    // up to 1 year of seconds
        1 => Just(u64::MAX / 2),
    ]
}

// ── Action Generators ────────────────────────────────────────────────────────

/// Enumeration of all possible staking contract actions for state exploration.
///
/// Each variant carries the minimal data needed to execute the action.
/// The `user_index` field selects from a pool of test users (modular indexing).
#[derive(Debug, Clone)]
pub enum StakingAction {
    /// Stake tokens.
    Stake { user_index: usize, amount: i128 },
    /// Request unstake.
    RequestUnstake { user_index: usize, amount: i128 },
    /// Withdraw from an unstake request.
    Withdraw { user_index: usize, request_id: u64 },
    /// Claim rewards.
    ClaimRewards { user_index: usize },
    /// Advance time.
    AdvanceTime { delta: u64 },
    /// Admin: set reward rate.
    SetRewardRate { new_rate: i128 },
    /// Admin: set lock period.
    SetLockPeriod { new_period: u64 },
    /// Admin: pause contract.
    Pause,
    /// Admin: unpause contract.
    Unpause,
}

/// Strategy for individual staking actions.
///
/// Weights model realistic usage: stakes and time advancement are most common,
/// admin operations are rare.
pub fn staking_action_strategy(num_users: usize) -> impl Strategy<Value = StakingAction> {
    let user_idx = 0..num_users;

    prop_oneof![
        // User actions (high weight for realistic distribution)
        30 => (user_idx.clone(), positive_amount_strategy()).prop_map(|(u, a)| StakingAction::Stake { user_index: u, amount: a }),
        15 => (user_idx.clone(), positive_amount_strategy()).prop_map(|(u, a)| StakingAction::RequestUnstake { user_index: u, amount: a }),
        10 => (user_idx.clone(), 1u64..=20u64).prop_map(|(u, r)| StakingAction::Withdraw { user_index: u, request_id: r }),
        15 => user_idx.clone().prop_map(|u| StakingAction::ClaimRewards { user_index: u }),
        // Time advancement (frequent)
        20 => duration_strategy().prop_map(|d| StakingAction::AdvanceTime { delta: d }),
        // Admin actions (rare)
        4 => reward_rate_strategy().prop_map(|r| StakingAction::SetRewardRate { new_rate: r }),
        3 => lock_period_strategy().prop_map(|p| StakingAction::SetLockPeriod { new_period: p }),
        2 => Just(StakingAction::Pause),
        1 => Just(StakingAction::Unpause),
    ]
}

/// Strategy for a sequence of staking actions.
///
/// Produces 1–`max_len` actions. Sequence length is bounded to keep test
/// execution time manageable while still exploring deep state spaces.
pub fn staking_action_sequence(
    num_users: usize,
    max_len: usize,
) -> impl Strategy<Value = Vec<StakingAction>> {
    prop::collection::vec(staking_action_strategy(num_users), 1..=max_len)
}

// ── Staking Config Generators ────────────────────────────────────────────────

/// Complete staking contract configuration for property-based initialization tests.
#[derive(Debug, Clone)]
pub struct StakingConfig {
    pub reward_rate: i128,
    pub lock_period: u64,
    pub num_stakers: usize,
    pub initial_balances: Vec<i128>,
}

/// Strategy for complete staking configurations.
pub fn staking_config_strategy() -> impl Strategy<Value = StakingConfig> {
    let num_stakers = 1usize..=8usize;
    (reward_rate_strategy(), lock_period_strategy(), num_stakers).prop_flat_map(
        |(reward_rate, lock_period, num_stakers)| {
            let balances =
                prop::collection::vec(positive_amount_strategy(), num_stakers..=num_stakers);
            balances.prop_map(move |initial_balances| StakingConfig {
                reward_rate,
                lock_period,
                num_stakers,
                initial_balances,
            })
        },
    )
}

// ── Mutation Testing Support ─────────────────────────────────────────────────

/// Describes a single mutation to apply to contract inputs for mutation testing.
///
/// Mutation testing verifies that the test suite catches intentional bugs.
/// Each variant corresponds to a class of semantic mutation.
#[derive(Debug, Clone)]
pub enum Mutation {
    /// Replace an amount with zero.
    ZeroAmount,
    /// Negate an amount.
    NegateAmount,
    /// Use the maximum possible value.
    MaxAmount,
    /// Swap two user addresses.
    SwapUsers,
    /// Skip time advancement (freeze time).
    FreezeTime,
    /// Double the intended amount.
    DoubleAmount,
    /// Use an off-by-one on the amount.
    OffByOne,
}

/// Strategy for selecting a mutation.
pub fn mutation_strategy() -> impl Strategy<Value = Mutation> {
    prop_oneof![
        Just(Mutation::ZeroAmount),
        Just(Mutation::NegateAmount),
        Just(Mutation::MaxAmount),
        Just(Mutation::SwapUsers),
        Just(Mutation::FreezeTime),
        Just(Mutation::DoubleAmount),
        Just(Mutation::OffByOne),
    ]
}

/// Apply a mutation to an amount.
pub fn mutate_amount(amount: i128, mutation: &Mutation) -> i128 {
    match mutation {
        Mutation::ZeroAmount => 0,
        Mutation::NegateAmount => amount.checked_neg().unwrap_or(i128::MAX),
        Mutation::MaxAmount => i128::MAX,
        Mutation::DoubleAmount => amount.saturating_mul(2),
        Mutation::OffByOne => amount.saturating_add(1),
        _ => amount, // Non-amount mutations pass through
    }
}

// ── Historical Pattern Generators ────────────────────────────────────────────

/// Models common real-world transaction patterns for fuzz input generation.
///
/// Each pattern produces a sequence of actions that mimics observed on-chain
/// behaviour, achieving higher state-space coverage than pure random sampling.
#[derive(Debug, Clone)]
pub enum TransactionPattern {
    /// Single user stakes, waits, claims, unstakes.
    SimpleStakeAndClaim,
    /// Multiple users stake, partial unstakes interleaved.
    MultiUserPartialUnstake,
    /// Rapid stake/unstake cycles.
    FlashStake,
    /// Admin changes rate while users are staking.
    RateChangeUnderLoad,
    /// User stakes, waits for timelock, withdraws.
    FullUnstakeLifecycle,
}

/// Generate a concrete action sequence from a transaction pattern.
pub fn pattern_to_actions(pattern: &TransactionPattern, num_users: usize) -> Vec<StakingAction> {
    match pattern {
        TransactionPattern::SimpleStakeAndClaim => {
            vec![
                StakingAction::Stake { user_index: 0, amount: 10_000 },
                StakingAction::AdvanceTime { delta: 100 },
                StakingAction::ClaimRewards { user_index: 0 },
                StakingAction::RequestUnstake { user_index: 0, amount: 10_000 },
                StakingAction::AdvanceTime { delta: 86_400 },
                StakingAction::Withdraw { user_index: 0, request_id: 1 },
            ]
        }
        TransactionPattern::MultiUserPartialUnstake => {
            let mut actions = Vec::new();
            for i in 0..num_users.min(4) {
                actions.push(StakingAction::Stake { user_index: i, amount: (i as i128 + 1) * 1_000 });
            }
            actions.push(StakingAction::AdvanceTime { delta: 50 });
            for i in 0..num_users.min(4) {
                actions.push(StakingAction::RequestUnstake { user_index: i, amount: (i as i128 + 1) * 500 });
            }
            actions.push(StakingAction::AdvanceTime { delta: 100 });
            for i in 0..num_users.min(4) {
                actions.push(StakingAction::ClaimRewards { user_index: i });
            }
            actions
        }
        TransactionPattern::FlashStake => {
            vec![
                StakingAction::Stake { user_index: 0, amount: 100_000 },
                StakingAction::AdvanceTime { delta: 1 },
                StakingAction::RequestUnstake { user_index: 0, amount: 100_000 },
                StakingAction::AdvanceTime { delta: 1 },
                StakingAction::Stake { user_index: 0, amount: 200_000 },
                StakingAction::AdvanceTime { delta: 1 },
                StakingAction::RequestUnstake { user_index: 0, amount: 200_000 },
            ]
        }
        TransactionPattern::RateChangeUnderLoad => {
            let mut actions = Vec::new();
            for i in 0..num_users.min(3) {
                actions.push(StakingAction::Stake { user_index: i, amount: 5_000 });
            }
            actions.push(StakingAction::AdvanceTime { delta: 50 });
            actions.push(StakingAction::SetRewardRate { new_rate: 20 });
            actions.push(StakingAction::AdvanceTime { delta: 50 });
            for i in 0..num_users.min(3) {
                actions.push(StakingAction::ClaimRewards { user_index: i });
            }
            actions
        }
        TransactionPattern::FullUnstakeLifecycle => {
            vec![
                StakingAction::Stake { user_index: 0, amount: 50_000 },
                StakingAction::AdvanceTime { delta: 200 },
                StakingAction::ClaimRewards { user_index: 0 },
                StakingAction::RequestUnstake { user_index: 0, amount: 50_000 },
                StakingAction::AdvanceTime { delta: 86_401 },
                StakingAction::Withdraw { user_index: 0, request_id: 1 },
            ]
        }
    }
}

/// Strategy that selects a transaction pattern.
pub fn transaction_pattern_strategy() -> impl Strategy<Value = TransactionPattern> {
    prop_oneof![
        Just(TransactionPattern::SimpleStakeAndClaim),
        Just(TransactionPattern::MultiUserPartialUnstake),
        Just(TransactionPattern::FlashStake),
        Just(TransactionPattern::RateChangeUnderLoad),
        Just(TransactionPattern::FullUnstakeLifecycle),
    ]
}
