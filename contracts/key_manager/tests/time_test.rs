#![cfg(test)]

use soroban_sdk::{Env, testutils::{Address as _, Ledger}};
use crate::KeyManagerContract; // adjust if module path differs

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths(); // bypass auth for testing
    env
}

#[test]
fn test_key_expiry_after_deadline() {
    let env = setup_env();

    // Register contract
    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    // Initial timestamp
    let start_time = 1_000;
    env.ledger().with_mut(|li| {
        li.timestamp = start_time;
    });

    // Create key with expiry (example: expires at start_time + 100)
    let expiry = start_time + 100;
    let key_id = client.create_key(&expiry);

    // Move time forward beyond expiry
    env.ledger().with_mut(|li| {
        li.timestamp = expiry + 1;
    });

    // Attempt to use expired key
    let result = client.use_key(&key_id);

    // Expect failure (adjust error type to your contract)
    assert_eq!(result.is_err(), true);
}

#[test]
fn test_key_valid_before_expiry() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let start_time = 2_000;
    env.ledger().with_mut(|li| {
        li.timestamp = start_time;
    });

    let expiry = start_time + 100;
    let key_id = client.create_key(&expiry);

    // Move time but still before expiry
    env.ledger().with_mut(|li| {
        li.timestamp = expiry - 1;
    });

    let result = client.use_key(&key_id);

    assert_eq!(result.is_ok(), true);
}

#[test]
fn test_timelock_enforcement() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let start_time = 3_000;
    env.ledger().with_mut(|li| {
        li.timestamp = start_time;
    });

    let unlock_time = start_time + 200;

    let key_id = client.create_timelocked_key(&unlock_time);

    // Try using before unlock time
    let early_result = client.use_key(&key_id);
    assert_eq!(early_result.is_err(), true);

    // Advance time past unlock
    env.ledger().with_mut(|li| {
        li.timestamp = unlock_time + 1;
    });

    let late_result = client.use_key(&key_id);
    assert_eq!(late_result.is_ok(), true);
}