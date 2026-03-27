#![no_std]
#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, Error, Symbol};

// This pulls in AuditContract and AuditContractClient from your src/lib.rs
use audit::*;

// ── Mock Contracts for Cross-Contract Testing ────────────────────────────────

#[soroban_sdk::contract]
pub struct MockIdentityContract;

#[soroban_sdk::contract]
pub struct MockVaultContract;

#[soroban_sdk::contract]
pub struct MockComplianceContract;

#[soroban_sdk::contractimpl]
impl MockIdentityContract {
    pub fn verify_actor(_env: Env, _actor: Address) -> Result<bool, Error> {
        Ok(true)
    }
    pub fn failing_verify(_env: Env, _actor: Address) -> Result<bool, Error> {
        Err(Error::from_contract_error(1))
    }
}

#[soroban_sdk::contractimpl]
impl MockVaultContract {
    pub fn check_balance(_env: Env, _account: Address) -> Result<i128, Error> {
        Ok(1000)
    }
    pub fn failing_balance(_env: Env, _account: Address) -> Result<i128, Error> {
        Err(Error::from_contract_error(2))
    }
}

#[soroban_sdk::contractimpl]
impl MockComplianceContract {
    pub fn check_compliance(_env: Env, _action: Symbol) -> Result<bool, Error> {
        Ok(true)
    }
    pub fn failing_check(_env: Env, _action: Symbol) -> Result<bool, Error> {
        Err(Error::from_contract_error(3))
    }
}

// ── Test Utilities ───────────────────────────────────────────────────────────

fn setup_test_env() -> (Env, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    // Register the primary Audit contract using the modern 'register' method
    let audit_id = env.register(AuditContract, ());
    let audit_client = AuditContractClient::new(&env, &audit_id);

    // Register secondary mock contracts
    let identity_id = env.register(MockIdentityContract, ());
    let vault_id = env.register(MockVaultContract, ());
    let compliance_id = env.register(MockComplianceContract, ());

    // Initialize with a generated admin address
    let admin = Address::generate(&env);
    audit_client.initialize(&admin);

    (env, audit_id, identity_id, vault_id, compliance_id)
}

// ── Cross-contract calling tests ─────────────────────────────────────────────

#[test]
fn test_successful_identity_verification_call() {
    let (env, audit_id, identity_id, _, _) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let actor = Address::generate(&env);
    let ok = audit_client.verify_identity(&identity_id, &actor, &Symbol::new(&env, "verify_actor"));
    assert!(ok);
}

#[test]
fn test_failing_identity_verification_call() {
    let (env, audit_id, identity_id, _, _) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let actor = Address::generate(&env);
    // try_verify_identity catches contract errors without panicking
    let result = audit_client.try_verify_identity(
        &identity_id,
        &actor,
        &Symbol::new(&env, "failing_verify"),
    );
    assert!(result.is_err());
}

#[test]
fn test_vault_balance_check_call() {
    let (env, audit_id, _, vault_id, _) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let account = Address::generate(&env);
    let balance =
        audit_client.check_vault_balance(&vault_id, &account, &Symbol::new(&env, "check_balance"));
    assert_eq!(balance, 1000);
}

#[test]
fn test_multiple_cross_contract_calls_success() {
    let (env, audit_id, identity_id, vault_id, compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let segment_id = Symbol::new(&env, "segment_a");
    audit_client.create_segment(&segment_id);

    let actor = Address::generate(&env);
    let compliance_action = Symbol::new(&env, "login");

    let seq = audit_client.append_entry_with_checks(
        &segment_id,
        &actor,
        &Symbol::new(&env, "login"),
        &Symbol::new(&env, "user_alice"),
        &Symbol::new(&env, "ok"),
        &identity_id,
        &Symbol::new(&env, "verify_actor"),
        &vault_id,
        &Symbol::new(&env, "check_balance"),
        &compliance_id,
        &compliance_action,
        &Symbol::new(&env, "check_compliance"),
    );

    assert_eq!(seq, 1);
    assert_eq!(audit_client.get_entry_count(&segment_id), 1);
}

#[test]
fn test_contract_address_not_found() {
    let (env, audit_id, _, _, _) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let unknown = Address::generate(&env);
    let actor = Address::generate(&env);

    let result =
        audit_client.try_verify_identity(&unknown, &actor, &Symbol::new(&env, "verify_actor"));
    assert!(result.is_err());
}
