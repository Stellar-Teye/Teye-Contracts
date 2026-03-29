#![allow(clippy::unwrap_used)]
use soroban_sdk::{testutils::Accounts, Address, Env};
// Assume ComplianceContract and ComplianceContractClient exist and are exported
use compliance::contract::{ComplianceContract, ComplianceContractClient};

#[test]
fn test_initialize_sets_state_and_prevents_reinit() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = env.accounts().generate();
    let contract_id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &contract_id);
    let audit_key = [0x42u8; 32];

    // First initialization should succeed
    let result1 = client.initialize(&admin, &audit_key);
    assert!(result1.is_ok(), "First initialization should succeed");

    // Second initialization should fail
    let admin2 = env.accounts().generate();
    let audit_key2 = [0x99u8; 32];
    let result2 = client.initialize(&admin2, &audit_key2);
    assert!(result2.is_err(), "Second initialization should fail");
}
