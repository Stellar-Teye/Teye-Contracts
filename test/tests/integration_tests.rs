//! # Contract Testing Framework — Integration Tests
//!
//! Comprehensive tests exercising the testing framework itself:
//! - Property-based testing with invariant verification
//! - State space exploration
//! - Scenario DSL
//! - Mutation testing detection

extern crate std;

use proptest::prelude::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

use test_framework::generators::*;
use test_framework::invariants::*;
use test_framework::scenario_dsl::Scenario;
use test_framework::state_explorer::*;
use test_framework::*;

// ═════════════════════════════════════════════════════════════════════════════
//  Property-Based Tests
// ═════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// **Property**: Staking `amount` always increases `total_staked` by exactly `amount`.
    #[test]
    fn prop_stake_increases_total(amount in positive_amount_strategy()) {
        let mut env = TestEnv::new();
        let harness = StakingTestHarness::new(&mut env, 10, 86_400);
        let staker = harness.create_staker(amount);

        let before = harness.total_staked();
        harness.stake(&staker, amount);
        let after = harness.total_staked();

        prop_assert_eq!(after, before + amount,
            "total_staked did not increase by the staked amount");
    }

    /// **Property**: Staking then fully unstaking returns total_staked to its prior value.
    #[test]
    fn prop_stake_unstake_identity(amount in positive_amount_strategy()) {
        let mut env = TestEnv::new();
        let harness = StakingTestHarness::new(&mut env, 10, 0);
        let staker = harness.create_staker(amount);

        let before = harness.total_staked();
        harness.stake(&staker, amount);
        harness.request_unstake(&staker, amount);
        let after = harness.total_staked();

        prop_assert_eq!(after, before,
            "total_staked should return to original after full unstake");
    }

    /// **Property**: Invalid amounts (zero/negative) are always rejected.
    #[test]
    fn prop_invalid_amounts_rejected(amount in invalid_amount_strategy()) {
        let mut env = TestEnv::new();
        let harness = StakingTestHarness::new(&mut env, 10, 86_400);
        let staker = harness.create_staker(1_000_000);

        let result = harness.client.try_stake(&staker, &amount);
        prop_assert!(result.is_err(),
            "Staking amount {} should have been rejected", amount);
    }

    /// **Property**: Reward accrual is proportional to time elapsed.
    #[test]
    fn prop_rewards_proportional_to_time(
        rate in 1i128..=100i128,
        elapsed in 1u64..=10_000u64,
    ) {
        let mut env = TestEnv::new();
        let harness = StakingTestHarness::new(&mut env, rate, 0);
        let staker = harness.create_staker(1_000);

        harness.env.set_timestamp(0);
        harness.stake(&staker, 1_000);

        harness.env.set_timestamp(elapsed);
        let rewards = harness.pending_rewards(&staker);

        // With a single staker holding all the stake, rewards = rate * elapsed.
        let expected = rate * (elapsed as i128);
        prop_assert_eq!(rewards, expected,
            "rewards ({}) != rate ({}) * elapsed ({}) = {}",
            rewards, rate, elapsed, expected);
    }

    /// **Property**: Invariants hold after arbitrary action sequences.
    #[test]
    fn prop_invariants_hold_under_random_actions(
        actions in staking_action_sequence(3, 15),
    ) {
        let mut env = TestEnv::new();
        let harness = StakingTestHarness::new(&mut env, 10, 86_400);
        let users: std::vec::Vec<Address> = (0..3)
            .map(|_| harness.create_staker(1_000_000_000))
            .collect();

        let invariants = InvariantSet::staking_defaults();
        let config = ExplorerConfig {
            max_steps: 15,
            fail_fast: true,
            record_snapshots: false,
        };

        let mut explorer = StateExplorer::new(
            &harness,
            invariants,
            config,
            users,
        );

        let result = explorer.explore(&actions);

        prop_assert!(result.passed(),
            "Invariant violations: {:?}", result.summary.invariant_violations);
    }

    /// **Property**: Double claim returns zero (idempotency).
    #[test]
    fn prop_double_claim_idempotent(
        amount in 1i128..=100_000i128,
        elapsed in 1u64..=1_000u64,
    ) {
        let mut env = TestEnv::new();
        let harness = StakingTestHarness::new(&mut env, 10, 0);
        let staker = harness.create_staker(amount);

        harness.env.set_timestamp(0);
        harness.stake(&staker, amount);
        harness.env.set_timestamp(elapsed);

        let first = harness.claim_rewards(&staker);
        let second = harness.claim_rewards(&staker);

        prop_assert!(first >= 0);
        prop_assert_eq!(second, 0,
            "Second claim should return 0, got {}", second);
    }

    /// **Property**: Multiple stakers' rewards sum to total distribution.
    #[test]
    fn prop_rewards_sum_conservation(
        elapsed in 10u64..=1_000u64,
    ) {
        let mut env = TestEnv::new();
        let harness = StakingTestHarness::new(&mut env, 100, 0);

        let alice = harness.create_staker(10_000);
        let bob = harness.create_staker(10_000);

        harness.env.set_timestamp(0);
        harness.stake(&alice, 3_000);
        harness.stake(&bob, 7_000);

        harness.env.set_timestamp(elapsed);
        let r_alice = harness.pending_rewards(&alice);
        let r_bob = harness.pending_rewards(&bob);
        let total_expected = 100i128 * (elapsed as i128);

        prop_assert_eq!(r_alice + r_bob, total_expected,
            "Rewards don't sum to expected: {} + {} != {}",
            r_alice, r_bob, total_expected);
    }
}

// ═════════════════════════════════════════════════════════════════════════════
//  Invariant Tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_all_invariants_hold_on_fresh_contract() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 10, 86_400);
    let stakers = std::vec::Vec::<Address>::new();

    let invariants = InvariantSet::staking_defaults();
    let snapshot = harness.snapshot(&stakers);
    invariants.assert_all(&snapshot);
}

#[test]
fn test_invariants_after_stake_and_unstake_cycle() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 10, 0);

    let alice = harness.create_staker(100_000);
    let bob = harness.create_staker(50_000);
    let stakers = std::vec![alice.clone(), bob.clone()];

    let invariants = InvariantSet::staking_defaults();

    // Initial state.
    invariants.assert_all(&harness.snapshot(&stakers));

    // Alice stakes.
    harness.env.set_timestamp(0);
    harness.stake(&alice, 50_000);
    invariants.assert_all(&harness.snapshot(&stakers));

    // Bob stakes.
    harness.stake(&bob, 25_000);
    invariants.assert_all(&harness.snapshot(&stakers));

    // Time passes.
    harness.env.set_timestamp(100);
    invariants.assert_all(&harness.snapshot(&stakers));

    // Alice partially unstakes.
    harness.request_unstake(&alice, 20_000);
    invariants.assert_all(&harness.snapshot(&stakers));

    // Alice claims rewards.
    harness.claim_rewards(&alice);
    invariants.assert_all(&harness.snapshot(&stakers));
}

#[test]
fn test_transition_invariant_stake_conservation() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 10, 0);

    let alice = harness.create_staker(100_000);
    let stakers = std::vec![alice.clone()];

    let before = harness.snapshot(&stakers);
    harness.stake(&alice, 10_000);
    let after = harness.snapshot(&stakers);

    let inv = StakeConservation { amount: 10_000 };
    assert!(inv.check(&before, &after).is_ok());
}

#[test]
fn test_transition_invariant_unstake_conservation() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 10, 0);

    let alice = harness.create_staker(100_000);
    let stakers = std::vec![alice.clone()];

    harness.stake(&alice, 10_000);
    let before = harness.snapshot(&stakers);
    harness.request_unstake(&alice, 5_000);
    let after = harness.snapshot(&stakers);

    let inv = UnstakeConservation { amount: 5_000 };
    assert!(inv.check(&before, &after).is_ok());
}

// ═════════════════════════════════════════════════════════════════════════════
//  State Space Explorer Tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_explorer_simple_sequence() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 10, 86_400);
    let users: std::vec::Vec<Address> = (0..2)
        .map(|_| harness.create_staker(1_000_000))
        .collect();

    let actions = vec![
        StakingAction::Stake { user_index: 0, amount: 10_000 },
        StakingAction::AdvanceTime { delta: 100 },
        StakingAction::Stake { user_index: 1, amount: 5_000 },
        StakingAction::AdvanceTime { delta: 50 },
        StakingAction::ClaimRewards { user_index: 0 },
        StakingAction::RequestUnstake { user_index: 0, amount: 5_000 },
    ];

    let mut explorer = StateExplorer::with_defaults(&harness, users);
    let result = explorer.explore(&actions);

    assert!(result.passed(), "Violations: {:?}", result.summary.invariant_violations);
    assert_eq!(result.summary.actions_executed, 6);
    assert!(result.summary.entry_points_hit.contains("stake"));
    assert!(result.summary.entry_points_hit.contains("claim_rewards"));
    assert!(result.summary.entry_points_hit.contains("request_unstake"));
}

#[test]
fn test_explorer_coverage_tracking() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 10, 0);
    let users: std::vec::Vec<Address> = (0..2)
        .map(|_| harness.create_staker(1_000_000))
        .collect();

    let actions = vec![
        StakingAction::Stake { user_index: 0, amount: 10_000 },
        StakingAction::AdvanceTime { delta: 100 },
        StakingAction::ClaimRewards { user_index: 0 },
        StakingAction::RequestUnstake { user_index: 0, amount: 10_000 },
        StakingAction::AdvanceTime { delta: 86_401 },
        StakingAction::Withdraw { user_index: 0, request_id: 1 },
        StakingAction::SetRewardRate { new_rate: 5 },
        StakingAction::SetLockPeriod { new_period: 3_600 },
    ];

    let mut explorer = StateExplorer::with_defaults(&harness, users);
    let result = explorer.explore(&actions);

    assert!(result.passed());

    let coverage = result.summary.entry_point_coverage(STAKING_ENTRY_POINTS.len());
    assert!(
        coverage >= 0.5,
        "Expected at least 50% coverage, got {:.1}%",
        coverage * 100.0
    );
}

#[test]
fn test_explorer_with_historical_patterns() {
    let patterns = vec![
        TransactionPattern::SimpleStakeAndClaim,
        TransactionPattern::FlashStake,
        TransactionPattern::FullUnstakeLifecycle,
    ];

    for pattern in &patterns {
        let mut local_env = TestEnv::new();
        let local_harness = StakingTestHarness::new(&mut local_env, 10, 86_400);
        let local_users: std::vec::Vec<Address> = (0..4)
            .map(|_| local_harness.create_staker(1_000_000))
            .collect();

        let actions = pattern_to_actions(pattern, 4);
        let mut explorer = StateExplorer::with_defaults(&local_harness, local_users);
        let result = explorer.explore(&actions);

        assert!(
            result.passed(),
            "Pattern {:?} failed: {:?}",
            pattern,
            result.summary.invariant_violations
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════
//  Scenario DSL Tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_scenario_simple_stake_and_verify() {
    Scenario::new("Simple stake and verify")
        .with_reward_rate(10)
        .with_lock_period(0)
        .given(|ctx| {
            let staker = ctx.harness.create_staker(100_000);
            ctx.stakers.push(staker);
        })
        .when("user stakes 50,000 tokens", |ctx| {
            ctx.harness.stake(&ctx.stakers[0], 50_000);
        })
        .then("total staked equals 50,000", |ctx| {
            assert_eq!(ctx.harness.total_staked(), 50_000);
        })
        .then("user's staked balance is 50,000", |ctx| {
            assert_eq!(ctx.harness.user_staked(&ctx.stakers[0]), 50_000);
        })
        .run();
}

#[test]
fn test_scenario_proportional_rewards() {
    Scenario::new("Proportional reward distribution")
        .with_reward_rate(100)
        .with_lock_period(0)
        .with_invariants(InvariantSet::staking_defaults())
        .given(|ctx| {
            let alice = ctx.harness.create_staker(100_000);
            let bob = ctx.harness.create_staker(100_000);
            ctx.stakers.push(alice);
            ctx.stakers.push(bob);
        })
        .when("Alice stakes 75% and Bob stakes 25%", |ctx| {
            ctx.harness.env.set_timestamp(0);
            ctx.harness.stake(&ctx.stakers[0], 75_000);
            ctx.harness.stake(&ctx.stakers[1], 25_000);
            ctx.harness.env.set_timestamp(100);
        })
        .then("Alice earns 75% of rewards", |ctx| {
            let r = ctx.harness.pending_rewards(&ctx.stakers[0]);
            assert_eq!(r, 7_500);
        })
        .then("Bob earns 25% of rewards", |ctx| {
            let r = ctx.harness.pending_rewards(&ctx.stakers[1]);
            assert_eq!(r, 2_500);
        })
        .run();
}

#[test]
fn test_scenario_full_unstake_lifecycle() {
    Scenario::new("Full unstake lifecycle with timelock")
        .with_reward_rate(10)
        .with_lock_period(86_400)
        .given(|ctx| {
            let staker = ctx.harness.create_staker(100_000);
            ctx.stakers.push(staker);
        })
        .when("user stakes, waits, and begins unstake", |ctx| {
            ctx.harness.env.set_timestamp(0);
            ctx.harness.stake(&ctx.stakers[0], 50_000);
            ctx.harness.env.set_timestamp(100);
            let rid = ctx.harness.request_unstake(&ctx.stakers[0], 50_000);
            ctx.store("request_id", rid as i128);
        })
        .then("staked balance is zero after unstake request", |ctx| {
            assert_eq!(ctx.harness.user_staked(&ctx.stakers[0]), 0);
        })
        .then("total staked is zero", |ctx| {
            assert_eq!(ctx.harness.total_staked(), 0);
        })
        .run();
}

// ═════════════════════════════════════════════════════════════════════════════
//  Mutation Testing
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_mutation_zero_amount_detected() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 10, 0);
    let staker = harness.create_staker(100_000);

    let mutated = mutate_amount(10_000, &Mutation::ZeroAmount);
    let result = harness.client.try_stake(&staker, &mutated);
    assert!(result.is_err(), "Zero-amount mutation was not caught");
}

#[test]
fn test_mutation_negative_amount_detected() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 10, 0);
    let staker = harness.create_staker(100_000);

    let mutated = mutate_amount(10_000, &Mutation::NegateAmount);
    let result = harness.client.try_stake(&staker, &mutated);
    assert!(result.is_err(), "Negative-amount mutation was not caught");
}

#[test]
fn test_mutation_double_amount_invariants_hold() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 10, 0);

    let original_amount = 10_000i128;
    let staker = harness.create_staker(original_amount * 3);

    let mutated = mutate_amount(original_amount, &Mutation::DoubleAmount);

    let stakers = std::vec![staker.clone()];
    let invariants = InvariantSet::staking_defaults();

    let _ = harness.client.try_stake(&staker, &mutated);
    let snapshot = harness.snapshot(&stakers);

    invariants.assert_all(&snapshot);
}

// ═════════════════════════════════════════════════════════════════════════════
//  Snapshot Tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_snapshot_captures_correct_state() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 10, 86_400);

    let alice = harness.create_staker(100_000);
    let bob = harness.create_staker(50_000);
    let stakers = std::vec![alice.clone(), bob.clone()];

    harness.env.set_timestamp(0);
    harness.stake(&alice, 30_000);
    harness.stake(&bob, 20_000);
    harness.env.set_timestamp(100);

    let snapshot = harness.snapshot(&stakers);

    assert_eq!(snapshot.timestamp, 100);
    assert_eq!(snapshot.total_staked, 50_000);
    assert_eq!(snapshot.reward_rate, 10);
    assert_eq!(snapshot.lock_period, 86_400);
    assert_eq!(snapshot.sum_user_stakes(), 50_000);
}

#[test]
fn test_snapshot_consistency_invariant() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 10, 0);

    let alice = harness.create_staker(100_000);
    let bob = harness.create_staker(100_000);
    let stakers = std::vec![alice.clone(), bob.clone()];

    harness.stake(&alice, 40_000);
    harness.stake(&bob, 60_000);

    let snapshot = harness.snapshot(&stakers);

    assert_eq!(
        snapshot.total_staked,
        snapshot.sum_user_stakes(),
        "Snapshot invariant: total_staked must equal sum of individual stakes"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
//  Edge Case Tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_zero_reward_rate_no_rewards_accrue() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 0, 0);
    let staker = harness.create_staker(100_000);

    harness.env.set_timestamp(0);
    harness.stake(&staker, 50_000);
    harness.env.set_timestamp(10_000);

    assert_eq!(harness.pending_rewards(&staker), 0);
}

#[test]
fn test_no_stakers_no_rewards() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 100, 0);
    let phantom_staker = Address::generate(&harness.env.env);

    harness.env.set_timestamp(0);
    harness.env.set_timestamp(10_000);

    assert_eq!(harness.pending_rewards(&phantom_staker), 0);
}

#[test]
fn test_single_token_stake() {
    let mut env = TestEnv::new();
    let harness = StakingTestHarness::new(&mut env, 1, 0);
    let staker = harness.create_staker(1);

    harness.env.set_timestamp(0);
    harness.stake(&staker, 1);
    harness.env.set_timestamp(1);

    assert_eq!(harness.pending_rewards(&staker), 1);
}
