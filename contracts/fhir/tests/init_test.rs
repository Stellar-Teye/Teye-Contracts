#![no_std]
use fhir::{FhirContract, FhirContractClient, FhirError};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_double_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let registry = Address::generate(&env);

    // First initialization should succeed
    client.initialize(&admin, &registry);

    // Second initialization should fail with AlreadyInitialized error
    let result = client.try_initialize(&admin, &registry);
    assert_eq!(result, Err(Ok(FhirError::AlreadyInitialized.into())));
}
