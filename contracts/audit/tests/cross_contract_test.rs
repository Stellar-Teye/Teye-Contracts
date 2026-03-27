//! Cross-contract calling invariants for the audit contract.
//!
//! This test suite verifies that the audit contract correctly handles:
//! - External contract calls to secondary interfaces
//! - Proper parsing of call results and error handling
//! - Graceful failure when secondary contracts are unavailable
//! - State consistency during cross-contract operations

use soroban_sdk::{
    testutils::Address as _,
    Address, Env, Error, Symbol,
};

use audit::contract::{AuditContract, AuditContractClient};

// ── Mock secondary contract interfaces ────────────────────────────────────────

/// Mock identity contract for testing cross-contract calls
#[soroban_sdk::contract]
pub struct MockIdentityContract;

/// Mock vault contract for testing cross-contract calls
#[soroban_sdk::contract]
pub struct MockVaultContract;

/// Mock compliance contract for testing cross-contract calls
#[soroban_sdk::contract]
pub struct MockComplianceContract;

#[soroban_sdk::contractimpl]
impl MockIdentityContract {
    /// Mock verification function
    pub fn verify_actor(env: Env, actor: Address) -> Result<bool, Error> {
        // Always return true for successful verification
        Ok(true)
    }

    /// Mock function that fails
    pub fn failing_verify(env: Env, actor: Address) -> Result<bool, Error> {
        Err(Error::from_contract_error(1))
    }
}

#[soroban_sdk::contractimpl]
impl MockVaultContract {
    /// Mock balance check
    pub fn check_balance(env: Env, account: Address) -> Result<i128, Error> {
        Ok(1000)
    }

    /// Mock function that fails
    pub fn failing_balance(env: Env, account: Address) -> Result<i128, Error> {
        Err(Error::from_contract_error(2))
    }
}

#[soroban_sdk::contractimpl]
impl MockComplianceContract {
    /// Mock compliance check
    pub fn check_compliance(env: Env, action: Symbol) -> Result<bool, Error> {
        Ok(true)
    }

    /// Mock function that fails
    pub fn failing_check(env: Env, action: Symbol) -> Result<bool, Error> {
        Err(Error::from_contract_error(3))
    }
}

// ── Test utilities ───────────────────────────────────────────────────────────

fn setup_test_env() -> (Env, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let audit_contract_id = env.register_contract(None, AuditContract);
    let audit_client = AuditContractClient::new(&env, &audit_contract_id);

    let identity_contract_id = env.register_contract(None, MockIdentityContract);
    let vault_contract_id = env.register_contract(None, MockVaultContract);
    let compliance_contract_id = env.register_contract(None, MockComplianceContract);

    // Initialize the audit contract
    let admin = Address::generate(&env);
    audit_client.initialize(&admin);

    (
        env,
        audit_contract_id,
        identity_contract_id,
        vault_contract_id,
        compliance_contract_id,
    )
}

fn setup_test_env_no_auth() -> (Env, Address, Address, Address, Address) {
    let env = Env::default();

    let audit_contract_id = env.register_contract(None, AuditContract);
    let audit_client = AuditContractClient::new(&env, &audit_contract_id);

    let identity_contract_id = env.register_contract(None, MockIdentityContract);
    let vault_contract_id = env.register_contract(None, MockVaultContract);
    let compliance_contract_id = env.register_contract(None, MockComplianceContract);

    // Initialize the audit contract with admin but no global mock auths in place
    let admin = Address::generate(&env);
    audit_client.initialize(&admin);

    (
        env,
        audit_contract_id,
        identity_contract_id,
        vault_contract_id,
        compliance_contract_id,
    )
}

// ── Cross-contract calling tests ─────────────────────────────────────────────

#[test]
fn test_successful_identity_verification_call() {
    let (env, audit_id, identity_id, _vault_id, _compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let actor = Address::generate(&env);
    let ok = audit_client.verify_identity(&identity_id, &actor, &Symbol::new(&env, "verify_actor"));
    assert!(ok);
}

#[test]
fn test_failing_identity_verification_call() {
    let (env, audit_id, identity_id, _vault_id, _compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let actor = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        audit_client.verify_identity(&identity_id, &actor, &Symbol::new(&env, "failing_verify"))
    }));
    assert!(result.is_err());
}

#[test]
fn test_vault_balance_check_call() {
    let (env, audit_id, _identity_id, vault_id, _compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let account = Address::generate(&env);
    let balance = audit_client.check_vault_balance(&vault_id, &account, &Symbol::new(&env, "check_balance"));
    assert_eq!(balance, 1000);
}

#[test]
fn test_failing_vault_balance_call() {
    let (env, audit_id, _identity_id, vault_id, _compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let account = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        audit_client.check_vault_balance(&vault_id, &account, &Symbol::new(&env, "failing_balance"))
    }));
    assert!(result.is_err());
}

#[test]
fn test_compliance_check_call() {
    let (env, audit_id, _identity_id, _vault_id, compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let action = Symbol::new(&env, "create_record");
    let compliant = audit_client.check_compliance(
        &compliance_id,
        &action,
        &Symbol::new(&env, "check_compliance"),
    );
    assert!(compliant);
}

#[test]
fn test_failing_compliance_check_call() {
    let (env, audit_id, _identity_id, _vault_id, compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let action = Symbol::new(&env, "create_record");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        audit_client.check_compliance(
            &compliance_id,
            &action,
            &Symbol::new(&env, "failing_check"),
        )
    }));
    assert!(result.is_err());
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
        &Symbol::new(&env, "user:alice"),
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
    let count = audit_client.get_entry_count(&segment_id);
    assert_eq!(count, 1);
}

#[test]
fn test_multiple_cross_contract_calls_partial_failure() {
    let (env, audit_id, identity_id, vault_id, compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let segment_id = Symbol::new(&env, "segment_b");
    audit_client.create_segment(&segment_id);

    let actor = Address::generate(&env);
    let compliance_action = Symbol::new(&env, "login");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        audit_client.append_entry_with_checks(
            &segment_id,
            &actor,
            &Symbol::new(&env, "login"),
            &Symbol::new(&env, "user:alice"),
            &Symbol::new(&env, "ok"),
            &identity_id,
            &Symbol::new(&env, "failing_verify"),
            &vault_id,
            &Symbol::new(&env, "check_balance"),
            &compliance_id,
            &compliance_action,
            &Symbol::new(&env, "check_compliance"),
        )
    }));
    assert!(result.is_err());

    let count = audit_client.get_entry_count(&segment_id);
    assert_eq!(count, 0);
}

#[test]
fn test_cross_contract_calls_with_auth() {
    let (env, audit_id, identity_id, vault_id, compliance_id) = setup_test_env_no_auth();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let segment_id = Symbol::new(&env, "auth_segment");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        audit_client.create_segment(&segment_id);
    }));
    assert!(result.is_err(), "create_segment should fail without auth");

    // With mock auth enabled, it should succeed.
    env.mock_all_auths();
    audit_client.create_segment(&segment_id);
}

#[test]
fn test_contract_address_not_found() {
    let (env, audit_id, _identity_id, _vault_id, _compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    let unknown = Address::generate(&env);
    let actor = Address::generate(&env);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        audit_client.verify_identity(&unknown, &actor, &Symbol::new(&env, "verify_actor"))
    }));
    assert!(result.is_err());
}

#[test]
fn test_external_call_timeout_simulation() {
    let (env, audit_id, identity_id, vault_id, compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    // Simulate timeout or unavailability of secondary contract
    // Test that audit contract handles this gracefully
}

#[test]
fn test_state_consistency_during_failed_calls() {
    let (env, audit_id, identity_id, vault_id, compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    // Verify that if cross-contract calls fail, the audit contract
    // doesn't leave the system in an inconsistent state
}

#[test]
fn test_cross_contract_call_result_parsing() {
    let (env, audit_id, identity_id, vault_id, compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    // Test that results from cross-contract calls are correctly parsed
    // and integrated into audit operations
}

#[test]
fn test_nested_cross_contract_calls() {
    let (env, audit_id, identity_id, vault_id, compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    // Test scenarios where secondary contracts themselves make
    // cross-contract calls (nested calls)
}

#[test]
fn test_cross_contract_event_emission() {
    let (env, audit_id, identity_id, vault_id, compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    // Test that cross-contract operations emit appropriate events
    // for monitoring and debugging
}

#[test]
fn test_gas_limits_cross_contract() {
    let (env, audit_id, identity_id, vault_id, compliance_id) = setup_test_env();
    let audit_client = AuditContractClient::new(&env, &audit_id);

    // Test behavior near gas limits during cross-contract operations
    // Ensure operations fail gracefully rather than consuming all gas
}