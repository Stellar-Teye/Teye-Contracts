extern crate std;

use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Address, Env};

use crate::{ContractError, StakingContract, StakingContractClient};

// ── Test helpers ──────────────────────────────────────────────────────────────

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

    StellarAssetClient::new(&env, &reward_token_id)
        .mock_all_auths()
        .mint(&contract_id, &1_000_000_000i128);

    (env, client, admin, stake_token_id, reward_token_id)
}

fn mint_and_stake(
    env: &Env,
    client: &StakingContractClient,
    stake_token: &Address,
    staker: &Address,
    amount: i128,
) {
    StellarAssetClient::new(env, stake_token).mint(staker, &amount);
    client.stake(staker, &amount);
}

// ── Authorization ─────────────────────────────────────────────────────────────

#[test]
fn test_slash_by_admin_succeeds() {
    let (env, client, admin, stake_token, _) = setup(0, 0);
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);

    let slashed = client.slash(&admin, &validator, &500);
    assert_eq!(
        slashed, 500,
        "admin slash must return the actual slashed amount"
    );
}

#[test]
fn test_slash_by_non_admin_returns_slashing_unauthorized() {
    let (env, client, _, stake_token, _) = setup(0, 0);
    let validator = Address::generate(&env);
    let attacker = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);

    let result = client.try_slash(&attacker, &validator, &500);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::SlashingUnauthorized),
        _ => panic!("expected SlashingUnauthorized"),
    }
}

// ── Balance accounting ────────────────────────────────────────────────────────

#[test]
fn test_slash_reduces_validator_staked_balance() {
    let (env, client, admin, stake_token, _) = setup(0, 0);
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 2_000);

    client.slash(&admin, &validator, &700);

    assert_eq!(
        client.get_staked(&validator),
        1_300,
        "validator balance must decrease by the slashed amount"
    );
}

#[test]
fn test_slash_reduces_total_staked() {
    let (env, client, admin, stake_token, _) = setup(0, 0);
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 2_000);

    let total_before = client.get_total_staked();
    client.slash(&admin, &validator, &800);

    assert_eq!(
        client.get_total_staked(),
        total_before - 800,
        "global total staked must decrease by the slashed amount"
    );
}

#[test]
fn test_slash_capped_at_validator_balance() {
    // Requesting more than the validator has should slash the entire balance.
    let (env, client, admin, stake_token, _) = setup(0, 0);
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 500);

    let slashed = client.slash(&admin, &validator, &1_000);

    assert_eq!(
        slashed, 500,
        "slash must be capped at the validator's balance"
    );
    assert_eq!(client.get_staked(&validator), 0);
    assert_eq!(client.get_total_staked(), 0);
}

#[test]
fn test_slash_does_not_affect_other_stakers() {
    let (env, client, admin, stake_token, _) = setup(0, 0);
    let validator = Address::generate(&env);
    let bystander = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);
    mint_and_stake(&env, &client, &stake_token, &bystander, 2_000);

    client.slash(&admin, &validator, &500);

    assert_eq!(
        client.get_staked(&bystander),
        2_000,
        "bystander balance must be unaffected by a slash targeting another validator"
    );
}

// ── Input validation ──────────────────────────────────────────────────────────

#[test]
fn test_slash_zero_amount_returns_invalid_input() {
    let (env, client, admin, stake_token, _) = setup(0, 0);
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);

    let result = client.try_slash(&admin, &validator, &0);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::InvalidInput),
        _ => panic!("expected InvalidInput for zero amount"),
    }
}

#[test]
fn test_slash_unstaked_validator_returns_insufficient_balance() {
    let (env, client, admin, _, _) = setup(0, 0);
    let validator = Address::generate(&env);
    // validator has never staked

    let result = client.try_slash(&admin, &validator, &100);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::InsufficientBalance),
        _ => panic!("expected InsufficientBalance for an unstaked validator"),
    }
}

// ── Multi-slash idempotency ───────────────────────────────────────────────────

#[test]
fn test_multiple_slashes_accumulate_correctly() {
    let (env, client, admin, stake_token, _) = setup(0, 0);
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 3_000);

    client.slash(&admin, &validator, &500);
    client.slash(&admin, &validator, &700);

    assert_eq!(
        client.get_staked(&validator),
        1_800,
        "two successive slashes must both be applied"
    );
}

#[test]
fn test_slash_all_then_slash_again_returns_insufficient_balance() {
    let (env, client, admin, stake_token, _) = setup(0, 0);
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);

    client.slash(&admin, &validator, &1_000);

    let result = client.try_slash(&admin, &validator, &1);
    match result {
        Err(Ok(e)) => assert_eq!(
            e,
            ContractError::InsufficientBalance,
            "second slash after full slash must fail with InsufficientBalance"
        ),
        _ => panic!("expected InsufficientBalance"),
    }
}
