#![cfg(test)]

use soroban_sdk::{Env, testutils::{Address as _, Events}};
use crate::KeyManagerContract; // adjust if needed

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

#[test]
fn test_key_creation_emits_event() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let expiry: u64 = 1000;

    // Perform action
    let key_id = client.create_key(&expiry);

    // Fetch emitted events
    let events = env.events().all();

    // Ensure at least one event was emitted
    assert_eq!(events.len() > 0, true);

    // Inspect the last event (most recent)
    let last_event = events.last().unwrap();

    // Example expected structure:
    // topics: ["key_created", key_id]
    // data: expiry

    assert_eq!(last_event.topics.len(), 2);

    // Match event name
    assert_eq!(last_event.topics.get(0).unwrap().to_symbol().unwrap(), "key_created");

    // Match key_id in topics (if applicable)
    assert_eq!(last_event.topics.get(1).unwrap(), key_id.into_val(&env));

    // Match payload
    assert_eq!(last_event.data, expiry.into_val(&env));
}

#[test]
fn test_key_usage_emits_event() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let expiry = 2000;
    let key_id = client.create_key(&expiry);

    // Clear previous events if needed
    env.events().clear();

    // Use key
    client.use_key(&key_id);

    let events = env.events().all();
    assert_eq!(events.len(), 1);

    let event = events.first().unwrap();

    assert_eq!(event.topics.get(0).unwrap().to_symbol().unwrap(), "key_used");
    assert_eq!(event.topics.get(1).unwrap(), key_id.into_val(&env));
}

#[test]
fn test_key_revocation_emits_event() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let expiry = 3000;
    let key_id = client.create_key(&expiry);

    env.events().clear();

    // Revoke key
    client.revoke_key(&key_id);

    let events = env.events().all();
    assert_eq!(events.len(), 1);

    let event = events.first().unwrap();

    assert_eq!(event.topics.get(0).unwrap().to_symbol().unwrap(), "key_revoked");
    assert_eq!(event.topics.get(1).unwrap(), key_id.into_val(&env));
}

#[test]
fn test_no_event_on_failed_operation() {
    let env = setup_env();

    let contract_id = env.register_contract(None, KeyManagerContract);
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let fake_key_id = 999u64;

    env.events().clear();

    // Attempt invalid operation
    let result = client.use_key(&fake_key_id);

    assert_eq!(result.is_err(), true);

    let events = env.events().all();

    // Ensure no event emitted on failure
    assert_eq!(events.len(), 0);
}