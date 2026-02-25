extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env};
use teye_common::admin_tiers::AdminTier;

use crate::{
    circuit_breaker::PauseScope, ContractError, VisionRecordsContract, VisionRecordsContractClient,
};

// ── Test helpers ─────────────────────────────────────────────────────────────

fn setup() -> (Env, VisionRecordsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

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

    let result = client.try_promote_admin(&contract_admin, &target, &AdminTier::OperatorAdmin);
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

    let result = client.try_promote_admin(&operator, &target, &AdminTier::OperatorAdmin);
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

// ── ContractAdmin can manage contract config ─────────────────────────────────

#[test]
fn test_contract_admin_can_set_rate_limit() {
    let (env, client, admin) = setup();
    let contract_admin = Address::generate(&env);

    client.promote_admin(&admin, &contract_admin, &AdminTier::ContractAdmin);
    client.set_rate_limit_config(&contract_admin, &100, &3600, &0u64);

    let config = client.get_rate_limit_config();
    assert_eq!(config, Some((100, 3600)));
}

#[test]
fn test_contract_admin_can_set_whitelist_enabled() {
    let (env, client, admin) = setup();
    let contract_admin = Address::generate(&env);

    client.promote_admin(&admin, &contract_admin, &AdminTier::ContractAdmin);
    client.set_whitelist_enabled(&contract_admin, &true);
    assert!(client.is_whitelist_enabled());
}

#[test]
fn test_contract_admin_can_add_to_whitelist() {
    let (env, client, admin) = setup();
    let contract_admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.promote_admin(&admin, &contract_admin, &AdminTier::ContractAdmin);
    client.add_to_whitelist(&contract_admin, &user);
    assert!(client.is_whitelisted(&user));
}

#[test]
fn test_contract_admin_can_remove_from_whitelist() {
    let (env, client, admin) = setup();
    let contract_admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.promote_admin(&admin, &contract_admin, &AdminTier::ContractAdmin);
    client.add_to_whitelist(&contract_admin, &user);
    assert!(client.is_whitelisted(&user));

    client.remove_from_whitelist(&contract_admin, &user);
    assert!(!client.is_whitelisted(&user));
}

// ── SuperAdmin can also call ContractAdmin-level functions ────────────────────

#[test]
fn test_super_admin_can_set_rate_limit() {
    let (_env, client, admin) = setup();
    client.set_rate_limit_config(&admin, &50, &1800, &0u64);
    let config = client.get_rate_limit_config();
    assert_eq!(config, Some((50, 1800)));
}

// ── OperatorAdmin can pause/unpause ──────────────────────────────────────────

#[test]
fn test_operator_admin_can_pause_contract() {
    let (env, client, admin) = setup();
    let operator = Address::generate(&env);

    // Register operator as a user first so they exist, then promote
    client.promote_admin(&admin, &operator, &AdminTier::OperatorAdmin);

    // OperatorAdmin should be able to pause
    client.pause_contract(&operator, &PauseScope::Global);
}

#[test]
fn test_operator_admin_can_resume_contract() {
    let (env, client, admin) = setup();
    let operator = Address::generate(&env);

    client.promote_admin(&admin, &operator, &AdminTier::OperatorAdmin);
    client.pause_contract(&operator, &PauseScope::Global);
    client.resume_contract(&operator, &PauseScope::Global);
}

// ── OperatorAdmin cannot call ContractAdmin-level functions ──────────────────

#[test]
fn test_operator_admin_cannot_set_rate_limit() {
    let (env, client, admin) = setup();
    let operator = Address::generate(&env);

    client.promote_admin(&admin, &operator, &AdminTier::OperatorAdmin);

    let result = client.try_set_rate_limit_config(&operator, &100, &3600, &0u64);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

#[test]
fn test_operator_admin_cannot_set_whitelist_enabled() {
    let (env, client, admin) = setup();
    let operator = Address::generate(&env);

    client.promote_admin(&admin, &operator, &AdminTier::OperatorAdmin);

    let result = client.try_set_whitelist_enabled(&operator, &true);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}

// ── Non-admin cannot call any admin function ─────────────────────────────────

#[test]
fn test_non_admin_cannot_set_rate_limit() {
    let (env, client, _admin) = setup();
    let intruder = Address::generate(&env);

    let result = client.try_set_rate_limit_config(&intruder, &100, &3600, &0u64);
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

// ── Demoted admin loses access ───────────────────────────────────────────────

#[test]
fn test_demoted_contract_admin_cannot_set_rate_limit() {
    let (env, client, admin) = setup();
    let contract_admin = Address::generate(&env);

    client.promote_admin(&admin, &contract_admin, &AdminTier::ContractAdmin);
    client.set_rate_limit_config(&contract_admin, &100, &3600, &0u64);

    // Demote
    client.demote_admin(&admin, &contract_admin);

    // Should now fail
    let result = client.try_set_rate_limit_config(&contract_admin, &200, &7200, &0u64);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => unreachable!("Expected Unauthorized error"),
    }
}
