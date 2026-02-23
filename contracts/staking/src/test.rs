extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env,
};

use crate::{ContractError, StakingContract, StakingContractClient};

// ── Test helpers ─────────────────────────────────────────────────────────────

/// Provisions a full test environment:
/// - Two SAC token contracts (stake + reward)
/// - A deployed StakingContract
/// - Mints `initial_balance` of `stake_token` to `staker`
/// - Mints a generous reward supply into the contract itself
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

    // Deploy two SAC tokens.
    let stake_token = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let reward_token = env.register_stellar_asset_contract_v2(Address::generate(&env));

    let stake_token_id = stake_token.address();
    let reward_token_id = reward_token.address();

    // Deploy the staking contract.
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
        .mint(&contract_id, &1_000_000_000i128);

    (env, client, admin, stake_token_id, reward_token_id)
}

/// Mint `amount` stake tokens to `recipient`.
fn mint_stake(env: &Env, stake_token: &Address, recipient: &Address, amount: i128) {
    StellarAssetClient::new(env, stake_token).mint(recipient, &amount);
}

// ── Initialisation ────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (_env, client, admin, stake_token, reward_token) = setup(10, 86_400);

    assert!(client.is_initialized());
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_reward_rate(), 10);
    assert_eq!(client.get_total_staked(), 0);
    assert_eq!(client.get_lock_period(), 86_400);

    // Duplicate initialisation must fail.
    let result = client.try_initialize(&admin, &stake_token, &reward_token, &10, &86_400);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::AlreadyInitialized),
        _ => unreachable!("Expected AlreadyInitialized error"),
    }
}

// ── Staking ───────────────────────────────────────────────────────────────────

#[test]
fn test_stake_increases_balance() {
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 86_400);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    client.stake(&staker, &1_000);

    assert_eq!(client.get_staked(&staker), 1_000);
    assert_eq!(client.get_total_staked(), 1_000);
}

#[test]
fn test_stake_zero_fails() {
    let (env, client, _admin, stake_token, _) = setup(10, 86_400);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    let result = client.try_stake(&staker, &0);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::InvalidInput),
        _ => unreachable!("Expected InvalidInput error"),
    }
}

#[test]
fn test_stake_negative_fails() {
    let (env, client, _admin, stake_token, _) = setup(10, 86_400);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    let result = client.try_stake(&staker, &-1);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::InvalidInput),
        _ => unreachable!("Expected InvalidInput error"),
    }
}

// ── Reward accrual ────────────────────────────────────────────────────────────

#[test]
fn test_reward_accrual_over_time() {
    let (env, client, _admin, stake_token, _) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    // Stake at t=0.
    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    // No time has passed — no rewards yet.
    assert_eq!(client.get_pending_rewards(&staker), 0);

    // Advance 100 seconds:
    // reward = rate × time = 10 × 100 = 1_000 tokens for the sole staker.
    env.ledger().set_timestamp(100);
    assert_eq!(client.get_pending_rewards(&staker), 1_000);
}

#[test]
fn test_no_rewards_when_nothing_staked() {
    let (env, client, _admin, _stake_token, _) = setup(10, 0);

    let staker = Address::generate(&env);

    // Advance time with no staking activity — RPT must not accumulate.
    env.ledger().set_timestamp(1_000);

    // Nobody staked, so rewards should not accumulate.
    assert_eq!(client.get_pending_rewards(&staker), 0);
    assert_eq!(client.get_total_staked(), 0);
}

// ── Proportional rewards ──────────────────────────────────────────────────────

#[test]
fn test_proportional_rewards_two_stakers() {
    let (env, client, _admin, stake_token, _) = setup(100, 0);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    mint_stake(&env, &stake_token, &alice, 3_000);
    mint_stake(&env, &stake_token, &bob, 1_000);

    // Both stake at t=0.
    env.ledger().set_timestamp(0);
    client.stake(&alice, &3_000); // 75 % of total
    client.stake(&bob, &1_000); // 25 % of total

    // After 100 seconds:
    // Total rewards = 100 × 100 = 10_000
    // Alice earns 75 % → 7_500
    // Bob earns 25 % → 2_500
    env.ledger().set_timestamp(100);

    let alice_earned = client.get_pending_rewards(&alice);
    let bob_earned = client.get_pending_rewards(&bob);

    assert_eq!(alice_earned, 7_500, "Alice should earn 75% of rewards");
    assert_eq!(bob_earned, 2_500, "Bob should earn 25% of rewards");
    // Total is conserved.
    assert_eq!(alice_earned + bob_earned, 10_000);
}

// ── Claim rewards ─────────────────────────────────────────────────────────────

#[test]
fn test_claim_rewards_transfers_tokens() {
    let (env, client, _admin, stake_token, reward_token) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    env.ledger().set_timestamp(100);
    let claimed = client.claim_rewards(&staker);

    assert_eq!(claimed, 1_000); // 10 tokens/s × 100 s

    // Staker's reward token balance should have increased.
    let balance = TokenClient::new(&env, &reward_token).balance(&staker);
    assert_eq!(balance, 1_000);

    // Pending rewards are cleared after claim.
    assert_eq!(client.get_pending_rewards(&staker), 0);
}

#[test]
fn test_double_claim_returns_zero() {
    let (env, client, _admin, stake_token, _) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);
    env.ledger().set_timestamp(100);

    client.claim_rewards(&staker); // first claim
    let second = client.claim_rewards(&staker); // same timestamp, nothing new

    assert_eq!(second, 0);
}

// ── Unstake & timelock ────────────────────────────────────────────────────────

#[test]
fn test_request_unstake_queues_request() {
    let (env, client, _admin, stake_token, _) = setup(10, 86_400);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    let request_id = client.request_unstake(&staker, &500);

    assert_eq!(request_id, 1);
    assert_eq!(client.get_staked(&staker), 500); // reduced immediately

    let req = client.get_unstake_request(&request_id);
    assert_eq!(req.amount, 500);
    assert_eq!(req.unlock_at, 86_400);
    assert!(!req.withdrawn);
}

#[test]
fn test_withdraw_before_timelock_fails() {
    let (env, client, _admin, stake_token, _) = setup(10, 86_400);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);
    let request_id = client.request_unstake(&staker, &1_000);

    // Still inside the lock window.
    env.ledger().set_timestamp(3_600); // only 1 hour in
    let result = client.try_withdraw(&staker, &request_id);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::TimelockNotExpired),
        _ => unreachable!("Expected TimelockNotExpired error"),
    }
}

#[test]
fn test_withdraw_after_timelock_succeeds() {
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 86_400);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);
    let request_id = client.request_unstake(&staker, &1_000);

    // Advance past the lock period.
    env.ledger().set_timestamp(86_401);
    client.withdraw(&staker, &request_id);

    // Verify the request is marked withdrawn.
    let req = client.get_unstake_request(&request_id);
    assert!(req.withdrawn);

    // Staked balance should be zero now.
    assert_eq!(client.get_staked(&staker), 0);

    // Token balance is returned (mock env handles the actual SAC transfer).
    let stake_balance = TokenClient::new(&env, &stake_token).balance(&staker);
    assert_eq!(stake_balance, 1_000);
}

#[test]
fn test_double_withdraw_fails() {
    let (env, client, _admin, stake_token, _) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);
    let request_id = client.request_unstake(&staker, &1_000);

    env.ledger().set_timestamp(1);
    client.withdraw(&staker, &request_id);

    let result = client.try_withdraw(&staker, &request_id);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::AlreadyWithdrawn),
        _ => unreachable!("Expected AlreadyInitialized error"),
    }
}

#[test]
fn test_unstake_more_than_staked_fails() {
    let (env, client, _admin, stake_token, _) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 500);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &500);

    let result = client.try_request_unstake(&staker, &1_000);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::InsufficientBalance),
        _ => unreachable!("Expected InsufficientBalance error"),
    }
}

// ── Admin ─────────────────────────────────────────────────────────────────────

#[test]
fn test_set_reward_rate_by_admin() {
    let (env, client, admin, stake_token, _) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    // Admin halves the rate at t=50.
    env.ledger().set_timestamp(50);
    client.set_reward_rate(&admin, &5);
    assert_eq!(client.get_reward_rate(), 5);

    // From t=0 to t=50: 10 × 50 = 500 earned at old rate.
    // From t=50 to t=150: 5 × 100 = 500 earned at new rate.
    // Total = 1_000.
    env.ledger().set_timestamp(150);
    assert_eq!(client.get_pending_rewards(&staker), 1_000);
}

#[test]
fn test_set_reward_rate_by_non_admin_fails() {
    let (env, client, _admin, _stake_token, _) = setup(10, 0);

    let intruder = Address::generate(&env);
    let result = client.try_set_reward_rate(&intruder, &999);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

#[test]
fn test_set_lock_period_by_admin() {
    let (_env, client, admin, _, _) = setup(10, 86_400);

    client.set_lock_period(&admin, &172_800); // 2 days
    assert_eq!(client.get_lock_period(), 172_800);
}

#[test]
fn test_rewards_after_rate_set_to_zero() {
    let (env, client, admin, stake_token, _) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    // Earn 10 × 50 = 500, then stop emissions.
    env.ledger().set_timestamp(50);
    client.set_reward_rate(&admin, &0);

    // Advance time — no further rewards should accrue.
    env.ledger().set_timestamp(1_000);
    assert_eq!(client.get_pending_rewards(&staker), 500);
}
