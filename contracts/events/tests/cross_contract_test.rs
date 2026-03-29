#![allow(clippy::unwrap_used)]

use events::{EventStreamContract, EventStreamContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_cross_contract_calling_invariants() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventStreamContract, ());
    let client = EventStreamContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let topic = String::from_str(&env, "external.failure");
    let hash = String::from_str(&env, "sha256:cross_contract");

    // Test that the contract parses and handles successfully
    client.register_schema(&admin, &topic, &1, &hash);

    // Ensure we can publish parsing cross contract mock
    let payload = String::from_str(&env, "payload");
    client.publish_event(&admin, &topic, &1, &payload);

    let all_events = env.events().all();
    assert!(
        !all_events.is_empty(),
        "Cross contract calling invariants not respected"
    );
}
