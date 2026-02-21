#![cfg(test)]

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

    client.register_user(&user, &Role::Optometrist, &name);

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

    let record_id = client.add_record(&patient, &provider, &RecordType::Examination, &data_hash);

    assert_eq!(record_id, 1);

    let record = client.get_record(&record_id);
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

    // Grant access
    client.grant_access(&patient, &doctor, &AccessLevel::Read, &86400);

    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);

    // Revoke access
    client.revoke_access(&patient, &doctor);
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
}

#[test]
fn test_request_emergency_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let requester = Address::generate(&env);
    let contacts = Vec::new(&env);
    let attestation = String::from_str(&env, "Patient unconscious, life-threatening emergency");

    let access_id = client.request_emergency_access(
        &requester,
        &patient,
        &EmergencyCondition::Unconscious,
        &attestation,
        &contacts,
        &14400_u64,
    );

    assert_eq!(access_id, 1);

    let grant = client.get_emergency_access(&access_id);
    assert_eq!(grant.patient, patient);
    assert_eq!(grant.requester, requester);
    assert_eq!(grant.condition, EmergencyCondition::Unconscious);
    assert_eq!(grant.status, EmergencyStatus::Active);
}

#[test]
fn test_emergency_access_invalid_without_attestation() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let requester = Address::generate(&env);
    let contacts = Vec::new(&env);
    let empty_attestation = String::from_str(&env, "");

    let result = client.try_request_emergency_access(
        &requester,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &empty_attestation,
        &contacts,
        &14400_u64,
    );

    assert!(result.is_err());
}

#[test]
fn test_is_emergency_access_valid() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let requester = Address::generate(&env);
    let contacts = Vec::new(&env);
    let attestation = String::from_str(&env, "Surgical emergency requiring immediate access");

    let access_id = client.request_emergency_access(
        &requester,
        &patient,
        &EmergencyCondition::SurgicalEmergency,
        &attestation,
        &contacts,
        &14400_u64,
    );

    assert!(client.is_emergency_access_valid(&access_id));
}

#[test]
fn test_revoke_emergency_access_by_patient() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let requester = Address::generate(&env);
    let contacts = Vec::new(&env);
    let attestation = String::from_str(&env, "Mass casualty event");

    let access_id = client.request_emergency_access(
        &requester,
        &patient,
        &EmergencyCondition::Masscasualties,
        &attestation,
        &contacts,
        &14400_u64,
    );

    assert!(client.is_emergency_access_valid(&access_id));

    client.revoke_emergency_access(&patient, &access_id);

    let grant = client.get_emergency_access(&access_id);
    assert_eq!(grant.status, EmergencyStatus::Revoked);
    assert!(!client.is_emergency_access_valid(&access_id));
}

#[test]
fn test_revoke_emergency_access_by_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let requester = Address::generate(&env);
    let contacts = Vec::new(&env);
    let attestation = String::from_str(&env, "Life threatening situation");

    let access_id = client.request_emergency_access(
        &requester,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &contacts,
        &14400_u64,
    );

    // Admin can also revoke
    client.revoke_emergency_access(&admin, &access_id);

    let grant = client.get_emergency_access(&access_id);
    assert_eq!(grant.status, EmergencyStatus::Revoked);
}

#[test]
fn test_unauthorized_revoke_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let requester = Address::generate(&env);
    let random = Address::generate(&env);
    let contacts = Vec::new(&env);
    let attestation = String::from_str(&env, "Emergency situation");

    let access_id = client.request_emergency_access(
        &requester,
        &patient,
        &EmergencyCondition::Unconscious,
        &attestation,
        &contacts,
        &14400_u64,
    );

    // Random address cannot revoke
    let result = client.try_revoke_emergency_access(&random, &access_id);
    assert!(result.is_err());
}

#[test]
fn test_log_emergency_record_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let requester = Address::generate(&env);
    let contacts = Vec::new(&env);
    let attestation = String::from_str(&env, "Emergency access required");

    let access_id = client.request_emergency_access(
        &requester,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &contacts,
        &14400_u64,
    );

    // Should succeed — grant is active
    client.log_emergency_record_access(&requester, &access_id);
}

#[test]
fn test_log_access_fails_after_revoke() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let requester = Address::generate(&env);
    let contacts = Vec::new(&env);
    let attestation = String::from_str(&env, "Emergency access required");

    let access_id = client.request_emergency_access(
        &requester,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &contacts,
        &14400_u64,
    );

    client.revoke_emergency_access(&patient, &access_id);

    // Should fail — grant is revoked
    let result = client.try_log_emergency_record_access(&requester, &access_id);
    assert!(result.is_err());
}

#[test]
fn test_emergency_contacts_stored() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let requester = Address::generate(&env);
    let contact1 = Address::generate(&env);
    let contact2 = Address::generate(&env);

    let mut contacts = Vec::new(&env);
    contacts.push_back(contact1.clone());
    contacts.push_back(contact2.clone());

    let attestation = String::from_str(&env, "Life-threatening emergency");

    let access_id = client.request_emergency_access(
        &requester,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &contacts,
        &14400_u64,
    );

    let grant = client.get_emergency_access(&access_id);
    assert_eq!(grant.notified_contacts.len(), 2);
}
