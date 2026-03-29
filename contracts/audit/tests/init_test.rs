#![cfg(test)]

use audit::contract::{AuditContract, AuditContractClient, AuditContractError};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn deploy() -> (Env, AuditContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(AuditContract, ());
    let client = AuditContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    (env, client, admin)
}

#[test]
fn test_initialize_once_succeeds() {
    let (_, client, admin) = deploy();
    let result = client.try_initialize(&admin);
    assert!(result.is_ok(), "First initialization must succeed");
}

#[test]
fn test_initialize_twice_reverts() {
    let (_, client, admin) = deploy();
    client.initialize(&admin);
    let result = client.try_initialize(&admin);
    assert!(result.is_err(), "Second initialization must fail");
}

#[test]
fn test_initialize_twice_different_admin_reverts() {
    let (env, client, admin) = deploy();
    let admin2 = Address::generate(&env);
    client.initialize(&admin);
    let result = client.try_initialize(&admin2);
    assert!(
        result.is_err(),
        "Re-init with a different admin must still fail"
    );
}
