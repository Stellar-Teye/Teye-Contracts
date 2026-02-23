#![allow(clippy::unwrap_used, clippy::expect_used)]
use crate::{CrossChainContract, CrossChainContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_initialization() {
    let env = Env::default();
    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // Initialize should succeed
    assert_eq!(client.initialize(&admin), ());
}

#[test]
fn test_add_relayer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);

    // Admin adding relayer should succeed
    assert_eq!(client.add_relayer(&admin, &relayer), ());
    assert!(client.is_relayer(&relayer));

    // Non-admin should fail (already caught by mock_all_auths if it tries to auth as admin without permission,
    // but in reality we expect the require_auth() to pass if caller is simulated properly, and the admin check to fail).
}

#[test]
fn test_map_identity() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let foreign_chain = String::from_str(&env, "ethereum");
    let foreign_address = String::from_str(&env, "0x12345");
    let local_patient = Address::generate(&env);

    assert_eq!(
        client.map_identity(&admin, &foreign_chain, &foreign_address, &local_patient),
        ()
    );

    let retrieved_address = client
        .get_local_address(&foreign_chain, &foreign_address)
        .unwrap();
    assert_eq!(retrieved_address, local_patient);
}
