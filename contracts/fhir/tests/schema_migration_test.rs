use fhir::{FhirContract, FhirContractClient};
use soroban_sdk::{
    symbol_short, testutils::Address as _, Address, Bytes, Env, Map, String, Symbol,
};

#[test]
fn test_forward_migration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let id = String::from_str(&env, "res-1");
    let payload = Bytes::from_slice(&env, b"v1-payload");

    // Register as V1
    client.register_resource(&admin, &id, &payload);

    // Verify V1 data
    assert_eq!(client.get_resource(&id), payload);

    // Manual migration to V2 (which adds 'meta' field)
    client.migrate_resource(&admin, &id, &2);

    // Verify it still works and now has V2 structure (internally)
    assert_eq!(client.get_resource(&id), payload);
}

#[test]
fn test_lazy_migration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let id = String::from_str(&env, "res-2");
    let payload = Bytes::from_slice(&env, b"lazy-v1");

    // Register as V1
    client.register_resource(&admin, &id, &payload);

    // get_resource should automatically trigger lazy migration if CURRENT_VERSION > 1
    // In our case, we manually registered a migration 1->2.
    // If we assume CURRENT_VERSION is at least 2, lazy migration should happen.
    let retrieved = client.get_resource(&id);
    assert_eq!(retrieved, payload);
}
