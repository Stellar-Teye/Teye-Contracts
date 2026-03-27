#![cfg(test)]

use soroban_sdk::{Env, testutils::Address as _};
use crate::KeyManagerContract; // adjust path if needed

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

#[test]
fn test_u64_overflow_protection() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let max_value: u64 = u64::MAX;

    // Example: storing or incrementing a max value
    let result = client.set_quota(&max_value);

    // Now attempt operation that would overflow
    let overflow_attempt = client.increment_quota(&1);

    // Expect error instead of wraparound
    assert_eq!(overflow_attempt.is_err(), true);
}

#[test]
fn test_u64_underflow_protection() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let zero: u64 = 0;

    client.set_quota(&zero);

    // Attempt decrement below zero
    let result = client.decrement_quota(&1);

    assert_eq!(result.is_err(), true);
}

#[test]
fn test_i128_overflow_protection() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let max_value: i128 = i128::MAX;

    client.set_balance(&max_value);

    // Attempt overflow
    let result = client.add_balance(&1);

    assert_eq!(result.is_err(), true);
}

#[test]
fn test_i128_underflow_protection() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let min_value: i128 = i128::MIN;

    client.set_balance(&min_value);

    // Attempt underflow
    let result = client.sub_balance(&1);

    assert_eq!(result.is_err(), true);
}

#[test]
fn test_boundary_exact_values() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    // Ensure contract accepts max safely WITHOUT computation
    let result = client.set_quota(&u64::MAX);

    assert_eq!(result.is_ok(), true);
}