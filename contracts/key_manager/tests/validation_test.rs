#![cfg(test)]

use soroban_sdk::{Env, Vec, Address, testutils::Address as _};
use crate::KeyManagerContract; // adjust path if needed

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

#[test]
fn test_zero_expiry_rejected() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let zero_expiry: u64 = 0;

    let result = client.create_key(&zero_expiry);

    assert_eq!(result.is_err(), true);
}

#[test]
fn test_zero_quota_rejected() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let zero: u64 = 0;

    let result = client.set_quota(&zero);

    assert_eq!(result.is_err(), true);
}

#[test]
fn test_empty_vector_rejected() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let empty_vec: Vec<u64> = Vec::new(&env);

    // Example: function that expects non-empty list
    let result = client.batch_create_keys(&empty_vec);

    assert_eq!(result.is_err(), true);
}

#[test]
fn test_invalid_address_rejected() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    // Generate address but simulate invalid usage scenario
    let addr = Address::generate(&env);

    // Example: passing address where additional validation should fail
    let result = client.assign_key(&addr, &0u64);

    assert_eq!(result.is_err(), true);
}

#[test]
fn test_zero_key_id_usage_fails() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let invalid_key_id: u64 = 0;

    let result = client.use_key(&invalid_key_id);

    assert_eq!(result.is_err(), true);
}

#[test]
fn test_empty_state_no_side_effects() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let invalid_key_id: u64 = 0;

    let _ = client.use_key(&invalid_key_id);

    // Ensure no events emitted for invalid input
    let events = env.events().all();
    assert_eq!(events.len(), 0);
}