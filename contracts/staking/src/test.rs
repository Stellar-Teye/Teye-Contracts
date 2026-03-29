extern crate std;

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger as _},
    token::{Client as TokenClient, StellarAssetClient},
    xdr::{ContractEventBody, ScVal},
    Address, Env, IntoVal, Val, Vec, String,
};

use crate::{
    events::{
        AccessViolationEvent, AdminTransferAcceptedEvent, AdminTransferCancelledEvent,
        AdminTransferProposedEvent, InitializedEvent, LockPeriodSetEvent,
        RateChangeDelaySetEvent, RewardClaimedEvent, RewardRateAppliedEvent,
        RewardRateProposedEvent, RewardRateSetEvent, SlashedEvent, SlippageExceededEvent,
        StakedEvent, UnstakeRequestedEvent, WithdrawnEvent,
    },
    rewards, ContractError, StakingContract, StakingContractClient,
};

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
fn assert_event_at<T>(env: &Env, index: usize, expected_topics: Vec<Val>, expected_data: &T)
where
    T: Clone + IntoVal<Env, Val>,
{
    let events = env.events().all();
    let event = events
        .events()
        .get(index)
        .unwrap_or_else(|| panic!("No event at index {}", index));
    let ContractEventBody::V0(body) = &event.body;

    let mut expected_topics_scval = std::vec::Vec::new();
    for topic in expected_topics.iter() {
        expected_topics_scval.push(ScVal::try_from_val(env, &topic).unwrap());
    }
    assert_eq!(body.topics.as_slice(), expected_topics_scval.as_slice());

    let expected_val: Val = expected_data.clone().into_val(env);
    let expected_data_scval = ScVal::try_from_val(env, &expected_val).unwrap();
    assert_eq!(body.data, expected_data_scval);
}

fn assert_last_event<T>(env: &Env, expected_topics: Vec<Val>, expected_data: &T)
where
    T: Clone + IntoVal<Env, Val>,
{
    let len = env.events().all().events().len();
    assert!(len > 0, "No events emitted");
    assert_event_at(env, len - 1, expected_topics, expected_data);
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

#[test]
fn test_initialize_emits_initialized_event() {
    let (env, client, admin, stake_token, reward_token) = setup(10, 86_400);

    assert!(client.is_initialized());

    let expected_topics: Vec<Val> = (symbol_short!("INIT"),).into_val(&env);
    let expected_data = InitializedEvent {
        admin: admin.clone(),
        stake_token: stake_token.clone(),
        reward_token: reward_token.clone(),
        reward_rate: 10,
        lock_period: 86_400,
        timestamp: env.ledger().timestamp(),
    };

    assert_last_event(&env, expected_topics, &expected_data);
}

#[test]
fn test_stake_emits_staked_event() {
    let (env, client, _admin, stake_token, _) = setup(10, 86_400);
    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);
    env.ledger().set_timestamp(1);

    client.stake(&staker, &1_000);

    // Initialize + 2 staked events because stake currently publishes twice.
    let events_len = env.events().all().events().len();
    assert_eq!(events_len, 3);

    let expected_topics: Vec<Val> = (symbol_short!("STAKED"), staker.clone()).into_val(&env);
    let expected_data = StakedEvent {
        staker: staker.clone(),
        amount: 1_000,
        new_total_staked: 1_000,
        timestamp: env.ledger().timestamp(),
    };

    assert_last_event(&env, expected_topics, &expected_data);

    // First staked event should be identical too.
    assert_event_at(&env, 1, expected_topics, &expected_data);
}

#[test]
fn test_request_unstake_and_withdraw_emit_events() {
    let (env, client, _admin, stake_token, _) = setup(10, 86_400);
    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);
    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    let request_id = client.request_unstake(&staker, &500);
    let expected_unstake_topics: Vec<Val> = (symbol_short!("UNSTK_REQ"), staker.clone()).into_val(&env);
    let expected_unstake_data = UnstakeRequestedEvent {
        request_id,
        staker: staker.clone(),
        amount: 500,
        unlock_at: 86_400,
        timestamp: env.ledger().timestamp(),
    };

    assert_last_event(&env, expected_unstake_topics, &expected_unstake_data);

    env.ledger().set_timestamp(86_401);
    client.withdraw(&staker, &request_id);

    let expected_withdraw_topics: Vec<Val> = (symbol_short!("WITHDRAWN"), staker.clone()).into_val(&env);
    let expected_withdraw_data = WithdrawnEvent {
        request_id,
        staker: staker.clone(),
        amount: 500,
        timestamp: env.ledger().timestamp(),
    };

    assert_last_event(&env, expected_withdraw_topics, &expected_withdraw_data);
}

#[test]
fn test_claim_rewards_emits_reward_claimed_event() {
    let (env, client, _admin, stake_token, reward_token) = setup(10, 0);
    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);
    env.ledger().set_timestamp(100);

    let claimed = client.claim_rewards(&staker);
    assert_eq!(claimed, 1_000);

    let expected_topics: Vec<Val> = (symbol_short!("CLMD"), staker.clone()).into_val(&env);
    let expected_data = RewardClaimedEvent {
        staker: staker.clone(),
        amount: 1_000,
        timestamp: env.ledger().timestamp(),
    };
    assert_last_event(&env, expected_topics, &expected_data);

    assert_eq!(TokenClient::new(&env, &reward_token).balance(&staker), 1_000);
}

#[test]
fn test_set_reward_rate_sets_event_direct_and_delayed() {
    let (env, client, admin, stake_token, _) = setup(10, 3_600);
    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    // With delay configured, set_reward_rate should produce RATE_PROP
    env.ledger().set_timestamp(100);
    client.set_reward_rate(&admin, &20, &0);

    let expected_topics_prop: Vec<Val> = (symbol_short!("RATE_PROP"),).into_val(&env);
    let expected_prop_data = RewardRateProposedEvent {
        new_rate: 20,
        effective_at: 3_700,
        timestamp: env.ledger().timestamp(),
    };
    assert_last_event(&env, expected_topics_prop, &expected_prop_data);

    // Trying to apply before time should fail and not emit apply event.
    env.ledger().set_timestamp(3_699);
    let result = client.try_apply_reward_rate(&admin);
    assert_eq!(result, Err(Ok(ContractError::RateChangeNotReady)));

    // Apply after delay.
    env.ledger().set_timestamp(3_701);
    client.apply_reward_rate(&admin);

    let expected_topics_aply: Vec<Val> = (symbol_short!("RATE_APLY"),).into_val(&env);
    let expected_aply_data = RewardRateAppliedEvent {
        new_rate: 20,
        timestamp: env.ledger().timestamp(),
    };
    assert_last_event(&env, expected_topics_aply, &expected_aply_data);
}

#[test]
fn test_rate_change_delay_and_lock_period_events() {
    let (env, client, admin, _, _) = setup(10, 0);

    client.set_rate_change_delay(&admin, &3_600);
    let expected_delay_topics: Vec<Val> = (symbol_short!("RATE_DLY"),).into_val(&env);
    assert_last_event(&env, expected_delay_topics, &RateChangeDelaySetEvent { delay: 3_600, timestamp: env.ledger().timestamp() });

    client.set_lock_period(&admin, &172_800, &0);
    let expected_lock_topics: Vec<Val> = (symbol_short!("LOCK_SET"),).into_val(&env);
    assert_last_event(&env, expected_lock_topics, &LockPeriodSetEvent { new_period: 172_800, timestamp: env.ledger().timestamp() });
}

#[test]
fn test_admin_transfer_lifecycle_emits_events() {
    let (env, client, admin, _, _) = setup(10, 0);
    let candidate = Address::generate(&env);

    client.propose_admin(&admin, &candidate);
    assert_last_event(&env, (symbol_short!("ADM_PROP"), admin.clone()).into_val(&env), &AdminTransferProposedEvent { current_admin: admin.clone(), proposed_admin: candidate.clone(), timestamp: env.ledger().timestamp() });

    // Accept as proposed admin
    env.ledger().set_timestamp(env.ledger().timestamp() + 1);
    client.accept_admin(&candidate);
    assert_last_event(&env, (symbol_short!("ADM_ACPT"), candidate.clone()).into_val(&env), &AdminTransferAcceptedEvent { old_admin: admin.clone(), new_admin: candidate.clone(), timestamp: env.ledger().timestamp() });

    // Set pending admin again so we can cancel it.
    env.ledger().set_timestamp(env.ledger().timestamp() + 1);
    let new_candidate = Address::generate(&env);
    client.propose_admin(&candidate, &new_candidate);
    client.cancel_admin_transfer(&candidate);
    assert_last_event(&env, (symbol_short!("ADM_CNCL"), candidate.clone()).into_val(&env), &AdminTransferCancelledEvent { admin: candidate.clone(), cancelled_proposed: new_candidate.clone(), timestamp: env.ledger().timestamp() });
}

#[test]
fn test_slash_emits_slashed_and_unauthorized_emits_access_violation() {
    let (env, client, admin, stake_token, _) = setup(10, 0);
    let validator = Address::generate(&env);
    mint_stake(&env, &stake_token, &validator, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&validator, &1_000);

    // Run authorized slash
    let slash_amount = client.slash(&admin, &validator, &500).unwrap();
    assert_eq!(slash_amount, 500);
    assert_last_event(&env, (symbol_short!("SLASHED"), validator.clone()).into_val(&env), &SlashedEvent { admin: admin.clone(), validator: validator.clone(), amount: 500, new_validator_stake: 500, new_total_staked: 500, timestamp: env.ledger().timestamp() });

    // unauthorized slash should emit access violation event
    let intruder = Address::generate(&env);
    let res = client.try_slash(&intruder, &validator, &100);
    assert_eq!(res, Err(Ok(ContractError::SlashingUnauthorized)));

    let expected_topics: Vec<Val> = (symbol_short!("ACC_VIOL"), intruder.clone(), String::from_str(&env, "slash")).into_val(&env);
    assert_last_event(&env, expected_topics, &AccessViolationEvent { caller: intruder.clone(), action: String::from_str(&env, "slash"), required_permission: String::from_str(&env, "admin_tier:ContractAdmin"), timestamp: env.ledger().timestamp() });
}

#[test]
fn test_stake_with_tolerance_slippage_exceeded_emits_event() {
    let (env, client, _admin, stake_token, _) = setup(10, 0);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    mint_stake(&env, &stake_token, &alice, 100);
    mint_stake(&env, &stake_token, &bob, 9_900);

    env.ledger().set_timestamp(0);
    client.stake(&alice, &100);
    client.stake(&bob, &9_900);

    // Alice stakes 100 again with strict min share > 2000 bps (actual 2000)
    let res = client.try_stake_with_tolerance(&alice, &100, &2_001);
    assert_eq!(res, Err(Ok(ContractError::SlippageExceeded)));

    let expected_topics: Vec<Val> = (symbol_short!("SLIP_EXC"), alice.clone()).into_val(&env);
    assert_last_event(&env, expected_topics, &SlippageExceededEvent { staker: alice.clone(), expected_share_bps: 2_001, actual_share_bps: 2000, timestamp: env.ledger().timestamp() });
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
fn test_high_latency_does_not_apply_implicit_slashing() {
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 0);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    // Simulate very delayed finality / network latency.
    env.ledger().set_timestamp(86_400 * 7);
    let _ = client.claim_rewards(&staker);

    // Staked principal must be untouched unless explicit unstake/withdraw occurs.
    assert_eq!(client.get_staked(&staker), 1_000);
    assert_eq!(client.get_total_staked(), 1_000);
}

#[test]
fn test_failed_early_withdraw_does_not_penalize_stake() {
    let (env, client, _admin, stake_token, _reward_token) = setup(10, 86_400);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);
    let request_id = client.request_unstake(&staker, &500);

    // Withdraw attempted before timelock expiration should not burn/slash funds.
    env.ledger().set_timestamp(300);
    let early = client.try_withdraw(&staker, &request_id);
    assert_eq!(early, Err(Ok(ContractError::TimelockNotExpired)));

    assert_eq!(client.get_staked(&staker), 500);
    let req = client.get_unstake_request(&request_id);
    assert!(!req.withdrawn);

    env.ledger().set_timestamp(86_401);
    client.withdraw(&staker, &request_id);

    assert_eq!(client.get_staked(&staker), 500);
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
    client.set_reward_rate(&admin, &5, &0);
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
    let result = client.try_set_reward_rate(&intruder, &999, &0);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

#[test]
fn test_set_lock_period_by_admin() {
    let (_env, client, admin, _, _) = setup(10, 86_400);

    client.set_lock_period(&admin, &172_800, &0); // 2 days
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
    client.set_reward_rate(&admin, &0, &0);

    // Advance time — no further rewards should accrue.
    env.ledger().set_timestamp(1_000);
    assert_eq!(client.get_pending_rewards(&staker), 500);
}

#[test]
fn test_reward_math_extreme_values_no_overflow() {
    let rpt = rewards::compute_reward_per_token(0, i128::MAX, u64::MAX, 1);
    assert_eq!(rpt, i128::MAX);

    let earned = rewards::earned(1, rpt, 0, 0);
    assert!(earned > 0);
}

// Input Validation Tests

#[test]
fn test_initialize_same_token_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let same_token = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let token_id = same_token.address();

    let contract_id = env.register(crate::StakingContract, ());
    let client = StakingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    // stake_token == reward_token must fail
    let result = client.try_initialize(&admin, &token_id, &token_id, &10, &86_400);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::TokensIdentical),
        _ => unreachable!("Expected TokensIdentical error"),
    }
}

#[test]
fn test_initialize_negative_reward_rate_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let stake_token = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let reward_token = env.register_stellar_asset_contract_v2(Address::generate(&env));

    let contract_id = env.register(crate::StakingContract, ());
    let client = StakingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let result = client.try_initialize(
        &admin,
        &stake_token.address(),
        &reward_token.address(),
        &-1,
        &86_400,
    );
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::InvalidInput),
        _ => unreachable!("Expected InvalidInput error"),
    }
}

// ── Delayed rate changes ──────────────────────────────────────────────────────

#[test]
fn test_set_rate_change_delay() {
    let (_env, client, admin, _, _) = setup(10, 0);

    client.set_rate_change_delay(&admin, &3_600);
    assert_eq!(client.get_rate_change_delay(), 3_600);
}

#[test]
fn test_delayed_reward_rate_proposal_and_apply() {
    let (env, client, admin, stake_token, _) = setup(10, 0);

    client.set_rate_change_delay(&admin, &3_600);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    // Propose a rate change at t=100; rate should remain unchanged.
    env.ledger().set_timestamp(100);
    client.set_reward_rate(&admin, &20, &0);
    assert_eq!(client.get_reward_rate(), 10);

    // Verify proposal is stored.
    let proposal = client.get_pending_rate_change();
    assert_eq!(proposal.new_rate, 20);
    assert_eq!(proposal.effective_at, 3_700);

    // Apply after delay has elapsed.
    env.ledger().set_timestamp(3_701);
    client.apply_reward_rate(&admin);
    assert_eq!(client.get_reward_rate(), 20);
}

#[test]
fn test_apply_reward_rate_before_delay_fails() {
    let (env, client, admin, _, _) = setup(10, 0);

    client.set_rate_change_delay(&admin, &3_600);

    env.ledger().set_timestamp(100);
    client.set_reward_rate(&admin, &20, &0);

    env.ledger().set_timestamp(3_699);
    let result = client.try_apply_reward_rate(&admin);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::RateChangeNotReady),
        _ => unreachable!("Expected RateChangeNotReady error"),
    }
}

#[test]
fn test_apply_reward_rate_no_pending_fails() {
    let (_env, client, admin, _, _) = setup(10, 0);

    let result = client.try_apply_reward_rate(&admin);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::NoPendingRateChange),
        _ => unreachable!("Expected NoPendingRateChange error"),
    }
}

#[test]
fn test_delayed_rate_change_rewards_are_correct() {
    let (env, client, admin, stake_token, _) = setup(10, 0);

    client.set_rate_change_delay(&admin, &3_600);

    let staker = Address::generate(&env);
    mint_stake(&env, &stake_token, &staker, 1_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &1_000);

    // Propose rate increase at t=100.
    env.ledger().set_timestamp(100);
    client.set_reward_rate(&admin, &20, &0);

    // Apply at t=3701. Old rate (10) active from t=0 to t=3701.
    env.ledger().set_timestamp(3_701);
    client.apply_reward_rate(&admin);

    // Advance to t=3801. New rate (20) active for 100s.
    // Old: 10 * 3701 = 37_010
    // New: 20 * 100  = 2_000
    // Total: 39_010
    env.ledger().set_timestamp(3_801);
    assert_eq!(client.get_pending_rewards(&staker), 39_010);
}
