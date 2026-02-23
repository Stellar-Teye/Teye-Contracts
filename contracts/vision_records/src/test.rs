use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::*;

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert!(client.is_initialized());
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_register_user() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);
    let name = String::from_str(&env, "Dr. Smith");

    client.register_user(&admin, &user, &Role::Optometrist, &name);

    let user_data = client.get_user(&user);
    assert_eq!(user_data.role, Role::Optometrist);
    assert!(user_data.is_active);
}

#[test]
fn test_add_and_get_record() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let data_hash = String::from_str(&env, "QmHash123");

    let record_id = client.add_record(
        &admin, // Use admin since they have SystemAdmin permission
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    assert_eq!(record_id, 1);

    let record = client.get_record(&provider, &record_id);
    assert_eq!(record.patient, patient);
    assert_eq!(record.provider, provider);
}

#[test]
fn test_access_control() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    // Initially no access
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);

    // Grant access (patient represents themselves)
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);

    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);

    // Revoke access
    client.revoke_access(&patient, &patient, &doctor);
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
}
