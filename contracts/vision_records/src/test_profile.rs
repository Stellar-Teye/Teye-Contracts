#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Events;
use soroban_sdk::{
    testutils::Address as _, testutils::Ledger as _, Address, Env, IntoVal, String, Vec,
};

fn setup_test() -> (Env, VisionRecordsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

#[test]
fn test_create_profile_success() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let dob_hash = String::from_str(&env, "hash_dob_123");
    let gender_hash = String::from_str(&env, "hash_gender_456");
    let blood_type_hash = String::from_str(&env, "hash_blood_789");

    // Patient creates their own profile
    client.create_profile(
        &patient,
        &patient,
        &dob_hash,
        &gender_hash,
        &blood_type_hash,
    );

    let profile = client.get_profile(&patient);
    assert_eq!(profile.patient, patient);
    assert_eq!(profile.date_of_birth_hash, dob_hash);
    assert_eq!(profile.gender_hash, gender_hash);
    assert_eq!(profile.blood_type_hash, blood_type_hash);
    assert!(profile.is_active);
    assert!(profile.emergency_contact.is_none());
    assert!(profile.insurance_info.is_none());
    assert_eq!(profile.medical_history_refs.len(), 0);

    // Verify event was emitted
    let events = env.events().all();
    assert!(!events.is_empty());
    let event = events.last().unwrap();
    assert_eq!(
        event.1,
        (symbol_short!("PROF_CRT"), patient.clone()).into_val(&env)
    );
}

#[test]
fn test_create_profile_by_authorized_user() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let staff = Address::generate(&env);
    let dob_hash = String::from_str(&env, "hash_dob_123");
    let gender_hash = String::from_str(&env, "hash_gender_456");
    let blood_type_hash = String::from_str(&env, "hash_blood_789");

    // Register staff with ManageUsers permission
    client.register_user(
        &admin,
        &staff,
        &Role::Staff,
        &String::from_str(&env, "Staff"),
    );

    // Staff creates profile for patient
    client.create_profile(&staff, &patient, &dob_hash, &gender_hash, &blood_type_hash);

    let profile = client.get_profile(&patient);
    assert_eq!(profile.patient, patient);
}

#[test]
fn test_create_profile_duplicate_rejection() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let dob_hash = String::from_str(&env, "hash_dob_123");
    let gender_hash = String::from_str(&env, "hash_gender_456");
    let blood_type_hash = String::from_str(&env, "hash_blood_789");

    // Create profile first time
    client.create_profile(
        &patient,
        &patient,
        &dob_hash,
        &gender_hash,
        &blood_type_hash,
    );

    // Try to create again - should fail
    let result = client.try_create_profile(
        &patient,
        &patient,
        &dob_hash,
        &gender_hash,
        &blood_type_hash,
    );
    assert!(result.is_err());
}

#[test]
fn test_create_profile_unauthorized_user() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let dob_hash = String::from_str(&env, "hash_dob_123");
    let gender_hash = String::from_str(&env, "hash_gender_456");
    let blood_type_hash = String::from_str(&env, "hash_blood_789");

    // Unauthorized user tries to create profile for patient
    let result = client.try_create_profile(
        &unauthorized,
        &patient,
        &dob_hash,
        &gender_hash,
        &blood_type_hash,
    );
    assert!(result.is_err());
}

#[test]
fn test_update_demographics() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let dob_hash = String::from_str(&env, "hash_dob_123");
    let gender_hash = String::from_str(&env, "hash_gender_456");
    let blood_type_hash = String::from_str(&env, "hash_blood_789");

    // Create profile first
    client.create_profile(
        &patient,
        &patient,
        &dob_hash,
        &gender_hash,
        &blood_type_hash,
    );

    // Update with new values
    let new_dob_hash = String::from_str(&env, "hash_dob_new");
    let new_gender_hash = String::from_str(&env, "hash_gender_new");
    let new_blood_type_hash = String::from_str(&env, "hash_blood_new");

    let old_timestamp = client.get_profile(&patient).updated_at;

    client.update_demographics(
        &patient,
        &patient,
        &new_dob_hash,
        &new_gender_hash,
        &new_blood_type_hash,
    );

    let updated_profile = client.get_profile(&patient);
    assert_eq!(updated_profile.date_of_birth_hash, new_dob_hash);
    assert_eq!(updated_profile.gender_hash, new_gender_hash);
    assert_eq!(updated_profile.blood_type_hash, new_blood_type_hash);
    assert!(updated_profile.updated_at > old_timestamp);

    // Verify update event was emitted
    let events = env.events().all();
    let event = events.get(events.len() - 1).unwrap();
    assert_eq!(
        event.1,
        (symbol_short!("PROF_UPD"), patient.clone()).into_val(&env)
    );
}

#[test]
fn test_update_demographics_unauthorized() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let other_user = Address::generate(&env);
    let dob_hash = String::from_str(&env, "hash_dob_123");
    let gender_hash = String::from_str(&env, "hash_gender_456");
    let blood_type_hash = String::from_str(&env, "hash_blood_789");

    // Create profile first
    client.create_profile(
        &patient,
        &patient,
        &dob_hash,
        &gender_hash,
        &blood_type_hash,
    );

    // Other user tries to update - should fail
    let new_dob_hash = String::from_str(&env, "hash_dob_new");
    let result = client.try_update_demographics(
        &other_user,
        &patient,
        &new_dob_hash,
        &gender_hash,
        &blood_type_hash,
    );
    assert!(result.is_err());
}

#[test]
fn test_update_emergency_contact() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let dob_hash = String::from_str(&env, "hash_dob_123");
    let gender_hash = String::from_str(&env, "hash_gender_456");
    let blood_type_hash = String::from_str(&env, "hash_blood_789");

    // Create profile first
    client.create_profile(
        &patient,
        &patient,
        &dob_hash,
        &gender_hash,
        &blood_type_hash,
    );

    // Add emergency contact
    let contact = EmergencyContact {
        name: String::from_str(&env, "John Doe"),
        relationship: String::from_str(&env, "Spouse"),
        phone: String::from_str(&env, "+1234567890"),
        email: String::from_str(&env, "john@example.com"),
    };

    client.update_emergency_contact(&patient, &patient, &Some(contact.clone()));

    let profile = client.get_profile(&patient);
    assert!(profile.emergency_contact.is_some());
    let stored_contact = profile.emergency_contact.unwrap();
    assert_eq!(stored_contact.name, contact.name);
    assert_eq!(stored_contact.relationship, contact.relationship);
    assert_eq!(stored_contact.phone, contact.phone);
    assert_eq!(stored_contact.email, contact.email);
}

#[test]
fn test_update_insurance_info() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let dob_hash = String::from_str(&env, "hash_dob_123");
    let gender_hash = String::from_str(&env, "hash_gender_456");
    let blood_type_hash = String::from_str(&env, "hash_blood_789");

    // Create profile first
    client.create_profile(
        &patient,
        &patient,
        &dob_hash,
        &gender_hash,
        &blood_type_hash,
    );

    // Add insurance information (hashed values)
    let insurance = InsuranceInfo {
        provider_hash: String::from_str(&env, "hash_provider_123"),
        policy_id_hash: String::from_str(&env, "hash_policy_456"),
        group_id_hash: String::from_str(&env, "hash_group_789"),
        verified_at: env.ledger().timestamp(),
    };

    client.update_insurance(&patient, &patient, &Some(insurance.clone()));

    let profile = client.get_profile(&patient);
    assert!(profile.insurance_info.is_some());
    let stored_insurance = profile.insurance_info.unwrap();
    assert_eq!(stored_insurance.provider_hash, insurance.provider_hash);
    assert_eq!(stored_insurance.policy_id_hash, insurance.policy_id_hash);
    assert_eq!(stored_insurance.group_id_hash, insurance.group_id_hash);
    assert_eq!(stored_insurance.verified_at, insurance.verified_at);
}

#[test]
fn test_add_medical_history_reference() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let dob_hash = String::from_str(&env, "hash_dob_123");
    let gender_hash = String::from_str(&env, "hash_gender_456");
    let blood_type_hash = String::from_str(&env, "hash_blood_789");

    // Create profile first
    client.create_profile(
        &patient,
        &patient,
        &dob_hash,
        &gender_hash,
        &blood_type_hash,
    );

    // Add medical history references
    let reference1 = String::from_str(&env, "ipfs://QmReference1");
    let reference2 = String::from_str(&env, "record_id_12345");

    client.add_medical_history_reference(&patient, &patient, &reference1);
    client.add_medical_history_reference(&patient, &patient, &reference2);

    let profile = client.get_profile(&patient);
    assert_eq!(profile.medical_history_refs.len(), 2);
    assert_eq!(profile.medical_history_refs.get(0).unwrap(), reference1);
    assert_eq!(profile.medical_history_refs.get(1).unwrap(), reference2);
}

#[test]
fn test_profile_exists() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let dob_hash = String::from_str(&env, "hash_dob_123");
    let gender_hash = String::from_str(&env, "hash_gender_456");
    let blood_type_hash = String::from_str(&env, "hash_blood_789");

    // Profile doesn't exist yet
    assert!(!client.profile_exists(&patient));

    // Create profile
    client.create_profile(
        &patient,
        &patient,
        &dob_hash,
        &gender_hash,
        &blood_type_hash,
    );

    // Profile now exists
    assert!(client.profile_exists(&patient));
}

#[test]
fn test_get_profile_not_found() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);

    // Try to get non-existent profile
    let result = client.try_get_profile(&patient);
    assert!(result.is_err());
}

#[test]
fn test_profile_storage_collision_prevention() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let dob_hash = String::from_str(&env, "hash_dob_123");
    let gender_hash = String::from_str(&env, "hash_gender_456");
    let blood_type_hash = String::from_str(&env, "hash_blood_789");

    // Create profile with specific storage key pattern
    client.create_profile(
        &patient,
        &patient,
        &dob_hash,
        &gender_hash,
        &blood_type_hash,
    );

    // Verify the storage key pattern is correct and doesn't collide
    let profile_key = (symbol_short!("PAT_PROF"), patient.clone());

    // The key should exist in storage
    assert!(env.storage().persistent().has(&profile_key));

    // Getting the profile should work
    let profile = client.get_profile(&patient);
    assert_eq!(profile.patient, patient);
}
