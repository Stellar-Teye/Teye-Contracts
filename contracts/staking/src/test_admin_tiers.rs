extern crate std;

use common::admin_tiers::AdminTier;
use soroban_sdk::{
    testutils::Address as _,
    token::StellarAssetClient,
    Address, Env,
};

use crate::{ContractError, StakingContract, StakingContractClient};

// ── Test helpers ─────────────────────────────────────────────────────────────

fn setup() -> (Env, StakingContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let stake_token = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let reward_token = env.register_stellar_asset_contract_v2(Address::generate(&env));

    let contract_id = env.register(StakingContract, ());
    let client = StakingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(
        &admin,
        &stake_token.address(),
        &reward_token.address(),
        &10,
        &86_400,
    );

    // Pre-fund the contract with reward tokens
    StellarAssetClient::new(&env, &reward_token.address())
        .mock_all_auths()
        .mint(&contract_id, &1_000_000_000i128);

    (env, client, admin)
}

// ── SuperAdmin bootstrapped on initialize ────────────────────────────────────

#[test]
fn test_admin_is_super_admin_after_init() {
    let (_env, client, admin) = setup();
    let tier = client.get_admin_tier(&admin);
    assert_eq!(tier, Some(AdminTier::SuperAdmin));
}

// ── SuperAdmin can promote to all tiers ──────────────────────────────────────

#[test]
fn test_super_admin_promotes_contract_admin() {
    let (env, client, admin) = setup();
    let target = Address::generate(&env);

    client.promote_admin(&admin, &target, &AdminTier::ContractAdmin);
    assert_eq!(client.get_admin_tier(&target), Some(AdminTier::ContractAdmin));
}

#[test]
fn test_super_admin_promotes_operator_admin() {
    let (env, client, admin) = setup();
    let target = Address::generate(&env);

    client.promote_admin(&admin, &target, &AdminTier::OperatorAdmin);
    assert_eq!(client.get_admin_tier(&target), Some(AdminTier::OperatorAdmin));
}

#[test]
fn test_super_admin_promotes_another_super_admin() {
    let (env, client, admin) = setup();
    let target = Address::generate(&env);

    client.promote_admin(&admin, &target, &AdminTier::SuperAdmin);
    assert_eq!(client.get_admin_tier(&target), Some(AdminTier::SuperAdmin));
}

// ── SuperAdmin can demote ────────────────────────────────────────────────────

#[test]
fn test_super_admin_demotes_admin() {
    let (env, client, admin) = setup();
    let target = Address::generate(&env);

    client.promote_admin(&admin, &target, &AdminTier::ContractAdmin);
    assert_eq!(client.get_admin_tier(&target), Some(AdminTier::ContractAdmin));

    client.demote_admin(&admin, &target);
    assert_eq!(client.get_admin_tier(&target), None);
}

// ── Lower tiers cannot promote/demote ────────────────────────────────────────

#[test]
fn test_contract_admin_cannot_promote() {
    let (env, client, admin) = setup();
    let contract_admin = Address::generate(&env);
    let target = Address::generate(&env);

    client.promote_admin(&admin, &contract_admin, &AdminTier::ContractAdmin);

    let result = client.try_promote_admin(
        &contract_admin,
        &target,
        &AdminTier::OperatorAdmin,
    );
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

#[test]
fn test_operator_admin_cannot_promote() {
    let (env, client, admin) = setup();
    let operator = Address::generate(&env);
    let target = Address::generate(&env);

    client.promote_admin(&admin, &operator, &AdminTier::OperatorAdmin);

    let result = client.try_promote_admin(
        &operator,
        &target,
        &AdminTier::OperatorAdmin,
    );
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

#[test]
fn test_contract_admin_cannot_demote() {
    let (env, client, admin) = setup();
    let contract_admin = Address::generate(&env);
    let operator = Address::generate(&env);

    client.promote_admin(&admin, &contract_admin, &AdminTier::ContractAdmin);
    client.promote_admin(&admin, &operator, &AdminTier::OperatorAdmin);

    let result = client.try_demote_admin(&contract_admin, &operator);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

#[test]
fn test_operator_admin_cannot_demote() {
    let (env, client, admin) = setup();
    let operator = Address::generate(&env);
    let target = Address::generate(&env);

    client.promote_admin(&admin, &operator, &AdminTier::OperatorAdmin);
    client.promote_admin(&admin, &target, &AdminTier::OperatorAdmin);

    let result = client.try_demote_admin(&operator, &target);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

// ── ContractAdmin can call set_reward_rate / set_lock_period ─────────────────

#[test]
fn test_contract_admin_can_set_reward_rate() {
    let (env, client, admin) = setup();
    let contract_admin = Address::generate(&env);

    client.promote_admin(&admin, &contract_admin, &AdminTier::ContractAdmin);
    client.set_reward_rate(&contract_admin, &20);
    assert_eq!(client.get_reward_rate(), 20);
}

#[test]
fn test_contract_admin_can_set_lock_period() {
    let (env, client, admin) = setup();
    let contract_admin = Address::generate(&env);

    client.promote_admin(&admin, &contract_admin, &AdminTier::ContractAdmin);
    client.set_lock_period(&contract_admin, &172_800);
    assert_eq!(client.get_lock_period(), 172_800);
}

// ── SuperAdmin can also call ContractAdmin-level functions ────────────────────

#[test]
fn test_super_admin_can_set_reward_rate() {
    let (_env, client, admin) = setup();
    client.set_reward_rate(&admin, &50);
    assert_eq!(client.get_reward_rate(), 50);
}

// ── OperatorAdmin cannot call ContractAdmin-level functions ──────────────────

#[test]
fn test_operator_admin_cannot_set_reward_rate() {
    let (env, client, admin) = setup();
    let operator = Address::generate(&env);

    client.promote_admin(&admin, &operator, &AdminTier::OperatorAdmin);

    let result = client.try_set_reward_rate(&operator, &99);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

#[test]
fn test_operator_admin_cannot_set_lock_period() {
    let (env, client, admin) = setup();
    let operator = Address::generate(&env);

    client.promote_admin(&admin, &operator, &AdminTier::OperatorAdmin);

    let result = client.try_set_lock_period(&operator, &999);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

// ── Non-admin cannot call any admin function ─────────────────────────────────

#[test]
fn test_non_admin_cannot_set_reward_rate() {
    let (env, client, _admin) = setup();
    let intruder = Address::generate(&env);

    let result = client.try_set_reward_rate(&intruder, &999);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

#[test]
fn test_non_admin_cannot_promote() {
    let (env, client, _admin) = setup();
    let intruder = Address::generate(&env);
    let target = Address::generate(&env);

    let result = client.try_promote_admin(&intruder, &target, &AdminTier::OperatorAdmin);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

#[test]
fn test_non_admin_has_no_tier() {
    let (env, client, _admin) = setup();
    let random = Address::generate(&env);
    assert_eq!(client.get_admin_tier(&random), None);
}
