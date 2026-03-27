#![allow(clippy::unwrap_used)]
use soroban_sdk::{testutils::{Accounts}, Env, Symbol, Address};
use audit::contract::{AuditContract, AuditContractClient};

#[test]
fn test_create_segment_unauthenticated_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = env.accounts().generate();
    let contract_id = env.register_contract(None, AuditContract);
    let client = AuditContractClient::new(&env, &contract_id);
    client.initialize(&admin);
    let segment = Symbol::short("UNAUTH");

    // Try to call as a random address (not admin)
    let random_user = env.accounts().generate();
    // Remove all auths to simulate unauthenticated call
    env.reset_auths();
    let result = client.create_segment(&segment);
    assert!(result.is_err(), "Unauthenticated create_segment should fail");
}
