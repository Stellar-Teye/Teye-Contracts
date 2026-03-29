use fhir::types::{Gender, ObservationStatus};
use fhir::{FhirContract, FhirContractClient};
use soroban_sdk::{
    symbol_short, testutils::Address as _, Address, Bytes, Env, Map, String, Symbol,
};

#[test]
fn test_validation_logic_on_versioned_data() {
    let env = Env::default();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);

    let id = String::from_str(&env, "p1");
    let name = String::from_str(&env, "John Doe");
    let patient = client.create_patient(
        &id,
        &String::from_str(&env, "id1"),
        &name,
        &Gender::Male,
        &0,
    );

    // Check validation
    assert!(client.validate_patient(&patient));

    let invalid_patient = client.create_patient(
        &String::from_str(&env, ""),
        &String::from_str(&env, "id1"),
        &name,
        &Gender::Male,
        &0,
    );
    assert!(!client.validate_patient(&invalid_patient));
}

#[test]
fn test_observation_validation() {
    let env = Env::default();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);

    let id = String::from_str(&env, "o1");
    let obs = client.create_observation(
        &id,
        &ObservationStatus::Final,
        &String::from_str(&env, "sys"),
        &String::from_str(&env, "val"),
        &String::from_str(&env, "p1"),
        &String::from_str(&env, "100"),
        &0,
    );

    assert!(client.validate_observation(&obs));
}
