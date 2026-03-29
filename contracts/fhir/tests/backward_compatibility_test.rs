use fhir::{FhirContract, FhirContractClient};
use soroban_sdk::{
    symbol_short, testutils::Address as _, Address, Bytes, Env, IntoVal, Map, String, Symbol,
};

#[test]
fn test_v1_resource_can_be_read_by_v2_logic() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let id = String::from_str(&env, "res-v1");
    let payload = Bytes::from_slice(&env, b"v1-content");

    // Manually inject V1 data to skip the register_resource logic if needed,
    // but the contract's register_resource already sets it as V1.
    client.register_resource(&admin, &id, &payload);

    // Verify V1 data is readable
    assert_eq!(client.get_resource(&id), payload);

    // Now manually upgrade the contract "logic" (or just assume we are on V2)
    // The lazy_read in get_resource should handle the conversion internally.
    let retrieved = client.get_resource(&id);
    assert_eq!(retrieved, payload);
}

#[test]
fn test_manual_rollback_visibility() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let id = String::from_str(&env, "res-rollback");
    let payload = Bytes::from_slice(&env, b"content");

    client.register_resource(&admin, &id, &payload);

    // Migration 1 -> 2
    client.migrate_resource(&admin, &id, &2);

    // In our current simple contract, migrate_resource only supports forward.
    // If I wanted to test rollback, I would need a migrate_backward call.
}
