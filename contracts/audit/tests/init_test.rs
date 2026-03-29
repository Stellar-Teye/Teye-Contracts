cat > contracts/audit/tests/init_test.rs << 'RUSTEOF'
#![cfg(test)]

///
/// Issue #311 — Double Re-initialization Exploits
/// Goal: Calling initialize more than once must revert safely.
///

use soroban_sdk::{
    testutils::Address as _,
    Address, Env,
};

use audit_contract::{AuditContract, AuditContractClient, AuditError};

fn deploy() -> (Env, AuditContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AuditContract);
    let client      = AuditContractClient::new(&env, &contract_id);
    let admin       = Address::generate(&env);

    (env, client, admin)
}

// ─── Test 1: first initialize succeeds ───────────────────────────────────────

#[test]
fn test_initialize_once_succeeds() {
    let (env, client, admin) = deploy();

    let result = client.try_initialize(&admin);
    assert!(result.is_ok(), "First initialization must succeed");
}

// ─── Test 2: second initialize reverts ───────────────────────────────────────

#[test]
fn test_initialize_twice_reverts() {
    let (env, client, admin) = deploy();

    client.initialize(&admin);

    let result = client.try_initialize(&admin);

    assert_eq!(
        result,
        Err(Ok(AuditError::AlreadyInitialized)),
        "Second initialization must revert with AlreadyInitialized"
    );
}

// ─── Test 3: different admin on second call still reverts ────────────────────

#[test]
fn test_initialize_twice_different_admin_reverts() {
    let (env, client, admin) = deploy();
    let admin2 = Address::generate(&env);

    client.initialize(&admin);

    let result = client.try_initialize(&admin2);

    assert_eq!(
        result,
        Err(Ok(AuditError::AlreadyInitialized)),
        "Re-init with a different admin must still revert with AlreadyInitialized"
    );
}

// ─── Test 4: state is unchanged after failed re