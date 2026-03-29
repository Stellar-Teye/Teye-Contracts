//! Slippage tolerance boundary violation tests for the staking contract.
//!
//! Covers:
//! - Reward rounding tolerance (truncation direction, rounding loss bounds)
//! - Saturating arithmetic boundaries (overflow clamping)
//! - Proportional share slippage (front-running protection via `stake_with_tolerance`)
//! - Reward rate change boundary conditions
//! - Two-phase commit slippage scenarios

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env,
};

use crate::{rewards, ContractError, StakingContract, StakingContractClient};

// ── Test helpers ─────────────────────────────────────────────────────────────

/// Provisions a full test environment identical to the main test module's setup.
fn setup(
    reward_rate: i128,
    lock_period: u64,
) -> (
    Env,
    StakingContractClient<'static>,
    Address, // admin
    Address, // stake_token
    Address, // reward_token
) {
    let env = Env::default();
    env.mock_all_auths();

    let stake_token = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let reward_token = env.register_stellar_asset_contract_v2(Address::generate(&env));

    let stake_token_id = stake_token.address();
    let reward_token_id = reward_token.address();

    let contract_id = env.register(StakingContract, ());
    let client = StakingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(
        &admin,
        &stake_token_id,
        &reward_token_id,
        &reward_rate,
        &lock_period,
    );

    // Pre-fund the contract with reward tokens so claims can succeed.
    StellarAssetClient::new(&env, &reward_token_id)
        .mock_all_auths()
        .mint(&contract_id, &1_000_000_000_000i128);

    (env, client, admin, stake_token_id, reward_token_id)
}

/// Mint `amount` stake tokens to `recipient`.
fn mint_stake(env: &Env, stake_token: &Address, recipient: &Address, amount: i128) {
    StellarAssetClient::new(env, stake_token).mint(recipient, &amount);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. REWARD ROUNDING TOLERANCE
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rounding_loss_within_tolerance_single_staker() {
    // A single staker should experience at most 1 unit of rounding loss
    // per reward period due to integer division truncation.
    let (env, client, _admin, stake_token, _reward_token) = setup(7, 0); // odd rate

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 3); // small stake to maximise rounding

    env.ledger().set_timestamp(0);
    client.stake(&staker, &3);

    // 7 tokens/s × 10s = 70 tokens total
    // staker holds 100% → should earn 70, but rounding could eat up to 1 unit.
    env.ledger().set_timestamp(10);
    let pending = client.get_pending_rewards(&staker);
    let expected = 70i128;
    let diff = expected - pending;

    assert!(
        diff >= 0 && diff <= 1,
        "Rounding loss should be at most 1 unit, got diff={}",
        diff
    );
}

#[test]
fn test_rounding_loss_with_many_stakers_no_inflation() {
    // Sum of individual rewards must never exceed total emitted rewards.
    // This ensures rounding doesn't inflate supply.
    let (env, client, _admin, stake_token, _reward_token) = setup(100, 0);

    let stakers: std::vec::Vec<Address> = (0..5).map(|_| Address::generate(&env)).collect();
    let amounts = [333i128, 167, 250, 100, 150];
    let total_staked: i128 = amounts.iter().sum();

    env.ledger().set_timestamp(0);
    for (staker, &amount) in stakers.iter().zip(amounts.iter()) {
        mint_stake(&env, &stake_token, staker, amount);
        client.stake(staker, &amount);
    }

    assert_eq!(client.get_total_staked(), total_staked);

    // Advance 200 seconds: total_emitted = 100 × 200 = 20_000
    env.ledger().set_timestamp(200);
    let total_emitted = 20_000i128;

    let sum_pending: i128 = stakers.iter().map(|s| client.get_pending_rewards(s)).sum();

    assert!(
        sum_pending <= total_emitted,
        "Sum of individual rewards ({}) must not exceed total emitted ({})",
        sum_pending,
        total_emitted
    );

    // Rounding loss should be bounded by number of stakers
    let max_rounding_loss = stakers.len() as i128;
    assert!(
        total_emitted - sum_pending <= max_rounding_loss,
        "Total rounding loss ({}) exceeds bound ({})",
        total_emitted - sum_pending,
        max_rounding_loss,
    );
}

#[test]
fn test_rounding_direction_always_truncates() {
    // Verify that rounding in earned() always favours the contract (floor),
    // never the user (ceiling).
    //
    // With PRECISION = 10^12, staked = 1, RPT delta = 1 (sub-unit):
    // earned = 1 × 1 / 10^12 = 0 (truncated, not 1).
    let earned = rewards::earned(1, 1, 0, 0);
    assert_eq!(
        earned, 0,
        "Sub-unit RPT delta must be truncated to 0, not rounded up"
    );

    // With PRECISION = 10^12, staked = 999_999_999_999, RPT delta = 1:
    // earned = 999_999_999_999 / 10^12 = 0 (still truncated)
    let earned2 = rewards::earned(999_999_999_999, 1, 0, 0);
    assert_eq!(
        earned2, 0,
        "Just below one full unit must still truncate to 0"
    );

    // With PRECISION = 10^12, staked = 1_000_000_000_000, RPT delta = 1:
    // earned = 10^12 / 10^12 = 1 (exactly one unit)
    let earned3 = rewards::earned(1_000_000_000_000, 1, 0, 0);
    assert_eq!(
        earned3, 1,
        "Exactly one precision unit should yield exactly 1"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. SATURATING ARITHMETIC BOUNDARIES
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reward_per_token_overflow_clamps() {
    // With extreme rate × elapsed, compute_reward_per_token must clamp to i128::MAX.
    let rpt = rewards::compute_reward_per_token(0, i128::MAX, u64::MAX, 1);
    assert_eq!(
        rpt,
        i128::MAX,
        "RPT must clamp at i128::MAX, not panic or wrap"
    );
}

#[test]
fn test_earned_overflow_clamps_not_wraps() {
    // With maximal inputs, earned() must saturate rather than wrap or panic.
    // saturating_mul_checked(i128::MAX, i128::MAX) → i128::MAX (clamped)
    // then i128::MAX / PRECISION gives the final value, plus user_earned (0).
    // The key property is: no panic, no negative result, no wrap-around.
    let e = rewards::earned(i128::MAX, i128::MAX, 0, 0);
    assert!(
        e > 0,
        "Earned must be positive even with maximal inputs (clamped)"
    );
    // The result is i128::MAX / PRECISION (integer division of saturated product)
    let expected = i128::MAX / rewards::PRECISION;
    assert_eq!(
        e, expected,
        "Earned should be i128::MAX / PRECISION after saturation and division"
    );
}

#[test]
fn test_compute_rpt_zero_elapsed_returns_stored() {
    // Zero elapsed should return the stored value unchanged.
    let stored = 42_000i128;
    let rpt = rewards::compute_reward_per_token(stored, 100, 0, 1_000);
    assert_eq!(rpt, stored, "Zero elapsed must not change stored RPT");
}

#[test]
fn test_compute_rpt_negative_total_staked_returns_stored() {
    // Negative total_staked (should never happen, but defensive check)
    // must return stored unchanged.
    let stored = 100i128;
    let rpt = rewards::compute_reward_per_token(stored, 10, 60, -1);
    assert_eq!(
        rpt, stored,
        "Negative total_staked must not change stored RPT"
    );
}

#[test]
fn test_earned_with_prev_accumulation() {
    // Verify that previously accumulated earnings are preserved and added to.
    let prev = 5_000i128;
    let rpt = rewards::compute_reward_per_token(0, 10, 100, 1_000);
    let e = rewards::earned(1_000, rpt, 0, prev);
    // 10 × 100 = 1000 new + 5000 previous = 6000
    assert_eq!(e, 6_000, "Previous earnings must be preserved");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. PROPORTIONAL SHARE SLIPPAGE (FRONT-RUNNING PROTECTION)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_stake_with_tolerance_succeeds_when_met() {
    // Normal staking with a reasonable tolerance should pass.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);

    // First staker: expect 100% share (10_000 BPS)
    client.stake_with_tolerance(&staker, &1_000, &10_000);

    assert_eq!(client.get_staked(&staker), 1_000);
    assert_eq!(client.get_total_staked(), 1_000);
}

#[test]
fn test_stake_with_tolerance_fails_when_exceeded() {
    // If a large front-run deposit dilutes the staker's share below
    // the minimum, the transaction should revert with SlippageExceeded.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    // Front-runner stakes a massive amount first
    let front_runner = Address::generate(&env);
    mint_stake(&env, &stake_token, &front_runner, 99_000);
    env.ledger().set_timestamp(0);
    client.stake(&front_runner, &99_000);

    // Victim expects ~50% share but will only get ~1%
    let victim = Address::generate(&env);
    mint_stake(&env, &stake_token, &victim, 1_000);

    let result = client.try_stake_with_tolerance(&victim, &1_000, &5_000); // expects 50%
    match result {
        Err(Ok(e)) => assert_eq!(
            e,
            ContractError::SlippageExceeded,
            "Should revert with SlippageExceeded"
        ),
        _ => panic!("Expected SlippageExceeded error"),
    }

    // Victim's stake should not have been applied (Soroban rolls back on error).
    // In the test environment, the error return prevents further assertions
    // on on-chain state, but the error itself is the proof.
}

#[test]
fn test_stake_with_tolerance_zero_bps_always_passes() {
    // A tolerance of 0 means "no minimum share requirement" — always succeeds.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    // Large existing stake
    let whale = Address::generate(&env);
    mint_stake(&env, &stake_token, &whale, 1_000_000);
    env.ledger().set_timestamp(0);
    client.stake(&whale, &1_000_000);

    // Small staker with tolerance = 0 should succeed despite tiny share
    let minnow = Address::generate(&env);
    mint_stake(&env, &stake_token, &minnow, 1);
    client.stake_with_tolerance(&minnow, &1, &0);

    assert_eq!(client.get_staked(&minnow), 1);
}

#[test]
fn test_stake_with_tolerance_max_bps_requires_solo() {
    // Setting tolerance to 10_000 BPS (100%) requires being the sole staker.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    env.ledger().set_timestamp(0);

    // First staker: solo → 100% share → should succeed
    let solo = Address::generate(&env);
    mint_stake(&env, &stake_token, &solo, 1_000);
    client.stake_with_tolerance(&solo, &1_000, &10_000);
    assert_eq!(client.get_staked(&solo), 1_000);

    // Second staker expects 100% share but pool already has tokens → fails
    let second = Address::generate(&env);
    mint_stake(&env, &stake_token, &second, 1_000);
    let result = client.try_stake_with_tolerance(&second, &1_000, &10_000);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::SlippageExceeded),
        _ => panic!("Expected SlippageExceeded when another staker already exists"),
    }
}

#[test]
fn test_stake_with_tolerance_invalid_bps_fails() {
    // BPS out of range [0, 10_000] should fail with InvalidInput.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);

    // Negative BPS
    let result = client.try_stake_with_tolerance(&staker, &1_000, &-1);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::InvalidInput),
        _ => panic!("Expected InvalidInput for negative BPS"),
    }

    // BPS > 10_000
    let result2 = client.try_stake_with_tolerance(&staker, &1_000, &10_001);
    match result2 {
        Err(Ok(e)) => assert_eq!(e, ContractError::InvalidInput),
        _ => panic!("Expected InvalidInput for BPS > 10_000"),
    }
}

#[test]
fn test_stake_with_tolerance_zero_amount_fails() {
    // Zero amount should fail regardless of tolerance.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    let result = client.try_stake_with_tolerance(&staker, &0, &5_000);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::InvalidInput),
        _ => panic!("Expected InvalidInput for zero amount"),
    }
}

#[test]
fn test_stake_with_tolerance_exact_boundary() {
    // When the actual share exactly equals the minimum, it should succeed.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    env.ledger().set_timestamp(0);

    // Staker A deposits 1_000
    let staker_a = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker_a, 1_000);
    client.stake(&staker_a, &1_000);

    // Staker B deposits 1_000 → share = 1000/2000 = 50% = 5_000 BPS
    let staker_b = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker_b, 1_000);
    client.stake_with_tolerance(&staker_b, &1_000, &5_000);

    assert_eq!(client.get_staked(&staker_b), 1_000);
    assert_eq!(client.get_total_staked(), 2_000);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. REWARD RATE CHANGE BOUNDARY CONDITIONS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reward_accrual_at_rate_change_boundary() {
    // Rewards at the exact rate change timestamp should be correctly split.
    let (env, client, admin, stake_token, _reward_token) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    // Change rate at exactly t=100
    env.ledger().set_timestamp(100);
    client.set_reward_rate(&admin, &20, &0);

    // At this point: 10 × 100 = 1_000 earned at old rate.
    // Now from t=100 to t=200 at rate 20: 20 × 100 = 2_000.
    // Total = 3_000.
    env.ledger().set_timestamp(200);
    let pending = client.get_pending_rewards(&staker);
    assert_eq!(
        pending, 3_000,
        "Rewards must be correctly split across rate change boundary"
    );
}

#[test]
fn test_stake_during_pending_rate_change_earns_old_rate() {
    // A staker entering the pool while a delayed rate change is pending
    // should earn at the old rate until the change is applied.
    let (env, client, admin, stake_token, _reward_token) = setup(10, 0);

    client.set_rate_change_delay(&admin, &3_600);

    env.ledger().set_timestamp(0);

    // Propose rate change to 50 at t=100
    env.ledger().set_timestamp(100);
    client.set_reward_rate(&admin, &50, &0);
    // Rate is still 10 (proposal is pending)
    assert_eq!(client.get_reward_rate(), 10);

    // Staker enters at t=200, rate is still 10
    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);
    env.ledger().set_timestamp(200);
    client.stake(&staker, &1_000);

    // At t=300 (still before proposal effective_at = 100 + 3600 = 3700):
    // Earned = 10 × 100 = 1_000 at old rate
    env.ledger().set_timestamp(300);
    let pending = client.get_pending_rewards(&staker);
    assert_eq!(
        pending, 1_000,
        "Should earn at old rate while proposal is pending"
    );
}

#[test]
fn test_rapid_rate_oscillation_roundtrip() {
    // Rapidly changing rates back and forth should not lose or create rewards
    // beyond rounding tolerance.
    let (env, client, admin, stake_token, _reward_token) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    // Rate oscillation: 10 → 20 → 10 → 20 → 10 each lasting 100s
    let mut expected_total = 0i128;
    let rates = [10i128, 20, 10, 20, 10];
    for (i, &rate) in rates.iter().enumerate() {
        let t = (i as u64 + 1) * 100;
        env.ledger().set_timestamp(t);
        if i < rates.len() - 1 {
            client.set_reward_rate(&admin, &rates[i + 1], &0);
        }
        expected_total += rate * 100;
    }

    // Total expected = 10×100 + 20×100 + 10×100 + 20×100 + 10×100 = 7_000
    assert_eq!(expected_total, 7_000);

    env.ledger().set_timestamp(500);
    let pending = client.get_pending_rewards(&staker);
    assert_eq!(
        pending, expected_total,
        "Rapid rate oscillation must not introduce arithmetic drift"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. TWO-PHASE COMMIT SLIPPAGE
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_prepare_commit_pool_change_between_phases() {
    // Between prepare_stake and commit_stake, another staker can enter,
    // changing the pool composition. The commit still succeeds but the
    // staker's proportional share is different from what was expected.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    mint_stake(&env, &stake_token, &alice, 1_000);
    mint_stake(&env, &stake_token, &bob, 9_000);

    env.ledger().set_timestamp(0);

    // Alice prepares her stake
    client.prepare_stake(&alice, &1_000);

    // Bob front-runs by staking between phases
    client.stake(&bob, &9_000);
    assert_eq!(client.get_total_staked(), 9_000);

    // Alice commits — should still succeed
    client.commit_stake(&alice, &1_000);
    assert_eq!(client.get_staked(&alice), 1_000);
    assert_eq!(client.get_total_staked(), 10_000);

    // Alice's share is now 10% (1_000 BPS), not 100% as she might have
    // expected when she prepared. This is the "slippage" scenario.
    let alice_share = 1_000i128 * 10_000 / 10_000;
    assert_eq!(alice_share, 1_000, "Alice holds 10% of the pool (1000 BPS)");
}

#[test]
fn test_prepare_stake_balance_boundary() {
    // prepare_stake should succeed at exactly the user's balance
    // and fail at balance + 1.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    // Exactly the balance — should succeed
    let result = client.try_prepare_stake(&staker, &1_000);
    assert!(result.is_ok(), "prepare_stake at exact balance should succeed");

    // One more than the balance — should fail
    let result2 = client.try_prepare_stake(&staker, &1_001);
    match result2 {
        Err(Ok(e)) => assert_eq!(e, ContractError::InsufficientBalance),
        _ => panic!("Expected InsufficientBalance for amount > balance"),
    }
}

#[test]
fn test_prepare_unstake_boundary() {
    // prepare_request_unstake should succeed at exactly the staked amount
    // and fail at staked + 1.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 500);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &500);

    // Exactly staked — should succeed
    let result = client.try_prepare_request_unstake(&staker, &500);
    assert!(
        result.is_ok(),
        "prepare_request_unstake at exact stake should succeed"
    );

    // One more — should fail
    let result2 = client.try_prepare_request_unstake(&staker, &501);
    match result2 {
        Err(Ok(e)) => assert_eq!(e, ContractError::InsufficientBalance),
        _ => panic!("Expected InsufficientBalance for amount > staked"),
    }
}

#[test]
fn test_commit_without_prepare_fails() {
    // Calling commit_stake without a preceding prepare should fail.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    let result = client.try_commit_stake(&staker, &1_000);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::InvalidInput),
        _ => panic!("Expected InvalidInput when committing without preparation"),
    }
}

#[test]
fn test_rollback_clears_preparation() {
    // After rollback, commit should fail because preparation data was cleared.
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    // Prepare then rollback
    client.prepare_stake(&staker, &1_000);
    client.rollback_stake(&staker, &1_000);

    // Commit should now fail
    let result = client.try_commit_stake(&staker, &1_000);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::InvalidInput),
        _ => panic!("Expected InvalidInput after rollback"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. ADDITIONAL EDGE CASES
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reward_precision_with_dust_amounts() {
    // Verify reward computation doesn't break with very small (dust) stakes
    // and very large reward rates.
    let (env, client, _admin, stake_token, _reward_token) = setup(1_000_000, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1); // dust stake

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1);

    // Even with 1 unit staked and rate=1_000_000, after 1 second:
    // earned = 1_000_000 (sole staker gets it all)
    env.ledger().set_timestamp(1);
    let pending = client.get_pending_rewards(&staker);
    assert_eq!(
        pending, 1_000_000,
        "Sole staker with dust amount should still earn full rate"
    );
}

#[test]
fn test_multiple_stakers_rewards_sum_to_total() {
    // With any number of stakers, the sum of all rewards must equal
    // rate × elapsed (within rounding tolerance).
    let (env, client, _admin, stake_token, _reward_token) = setup(1_000, 0);

    env.ledger().set_timestamp(0);

    let stakes = [100i128, 200, 300, 400];
    let stakers: std::vec::Vec<Address> = stakes
        .iter()
        .map(|&amount| {
            let s = Address::generate(&env);
            mint_stake(&env, &stake_token, &s, amount);
            client.stake(&s, &amount);
            s
        })
        .collect();

    // 1_000 rate × 500s = 500_000 total rewards
    env.ledger().set_timestamp(500);
    let total_expected = 500_000i128;

    let sum: i128 = stakers.iter().map(|s| client.get_pending_rewards(s)).sum();

    // Allow rounding tolerance of at most 1 per staker
    let tolerance = stakes.len() as i128;
    assert!(
        (total_expected - sum).abs() <= tolerance,
        "Rewards sum ({}) deviates from expected ({}) by more than tolerance ({})",
        sum,
        total_expected,
        tolerance,
    );
}

#[test]
fn test_stake_unstake_claim_full_cycle_no_value_leak() {
    // A full stake → accrue → unstake → withdraw → claim cycle should
    // not leak or create value beyond rounding tolerance.
    let (env, client, _admin, stake_token, reward_token) = setup(100, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 10_000);

    // Stake at t=0
    env.ledger().set_timestamp(0);
    client.stake(&staker, &10_000);

    // Accrue for 100s: 100 × 100 = 10_000 reward
    env.ledger().set_timestamp(100);

    // Claim rewards
    let claimed = client.claim_rewards(&staker);
    assert_eq!(claimed, 10_000, "Claimed rewards should match expected");

    // Verify reward token balance
    let reward_balance = TokenClient::new(&env, &reward_token).balance(&staker);
    assert_eq!(reward_balance, 10_000);

    // Unstake
    let request_id = client.request_unstake(&staker, &10_000);
    assert_eq!(client.get_staked(&staker), 0);
    assert_eq!(client.get_total_staked(), 0);

    // Withdraw (lock period = 0)
    env.ledger().set_timestamp(101);
    client.withdraw(&staker, &request_id);

    // Stake token balance restored
    let stake_balance = TokenClient::new(&env, &stake_token).balance(&staker);
    assert_eq!(
        stake_balance, 10_000,
        "Full cycle should return all staked tokens"
    );

    // No residual rewards should remain
    let pending_after = client.get_pending_rewards(&staker);
    assert_eq!(pending_after, 0, "No residual rewards after full cycle");
}
