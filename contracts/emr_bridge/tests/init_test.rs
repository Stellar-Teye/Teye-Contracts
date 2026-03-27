use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{EmrBridgeContract, EmrBridgeContractClient, EmrBridgeError};

/// Helper function to set up the test environment, register the contract,
/// and create a client. This follows the same pattern as the identity contract tests.
fn setup() -> (Env, EmrBridgeContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EmrBridgeContract);
    let client = EmrBridgeContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    (env, client, admin)
}

#[test]
fn test_initialize_succeeds() {
    let (env, client, admin) = setup();

    // First initialization should succeed
    let result = client.try_initialize(&admin);
    assert_eq!(result, Ok(()));
}

#[test]
fn test_double_initialization_fails() {
    let (env, client, admin) = setup();

    // First initialization should succeed
    client.initialize(&admin).unwrap();

    // Second initialization should fail with AlreadyInitialized error
    let second_admin = Address::generate(&env);
    let result = client.try_initialize(&second_admin);
    assert_eq!(result, Err(Ok(EmrBridgeError::AlreadyInitialized)));
}

#[test]
fn test_multiple_initialization_attempts_all_fail() {
    let (env, client, admin) = setup();

    // First initialization succeeds
    client.initialize(&admin).unwrap();

    // Attempt to initialize multiple times with different admins
    for _ in 0..3 {
        let new_admin = Address::generate(&env);
        let result = client.try_initialize(&new_admin);
        assert_eq!(result, Err(Ok(EmrBridgeError::AlreadyInitialized)));
    }
}