use crate::{
    types::{Gender, ObservationStatus},
    FhirContract, FhirContractClient,
};
use soroban_sdk::{Env, String};

#[test]
fn test_patient_creation_and_validation() {
    let env = Env::default();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);

    let id = String::from_str(&env, "pat-123");
    let identifier = String::from_str(&env, "MRN-456");
    let name = String::from_str(&env, "John Doe");
    let gender = Gender::Male;
    let birth_date = 631152000; // 1990-01-01

    let patient = client.create_patient(&id, &identifier, &name, &gender, &birth_date);

    assert_eq!(patient.id, id);
    assert_eq!(patient.identifier, identifier);
    assert_eq!(patient.name, name);
    assert_eq!(patient.gender, gender);
    assert!(patient.active);
    assert_eq!(patient.birth_date, birth_date);

    let is_valid = client.validate_patient(&patient);
    assert!(is_valid);
}

#[test]
fn test_observation_creation_and_validation() {
    let env = Env::default();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);

    let id = String::from_str(&env, "obs-1");
    let status = ObservationStatus::Final;
    let code_system = String::from_str(&env, "LOINC");
    let code_value = String::from_str(&env, "8302-2"); // Body height
    let subject_id = String::from_str(&env, "pat-123");
    let value = String::from_str(&env, "180 cm");
    let effective_datetime = 1704067200; // 2024-01-01

    let observation = client.create_observation(
        &id,
        &status,
        &code_system,
        &code_value,
        &subject_id,
        &value,
        &effective_datetime,
    );

    assert_eq!(observation.id, id);
    assert_eq!(observation.status, status);
    assert_eq!(observation.code_system, code_system);
    assert_eq!(observation.code_value, code_value);
    assert_eq!(observation.subject_id, subject_id);
    assert_eq!(observation.value, value);
    assert_eq!(observation.effective_datetime, effective_datetime);

    let is_valid = client.validate_observation(&observation);
    assert!(is_valid);
}
