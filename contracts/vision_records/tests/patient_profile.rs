#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Bytes, Env, String, Vec};
use vision_records::{
    patient::{Demographics, EmergencyContact, PatientProfile},
    AccessLevel, Role, VisionRecordsContract, VisionRecordsContractClient, ContractError, RecordType,
};

fn setup_test_env() -> (Env, VisionRecordsContractClient<'static>, soroban_sdk::Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

fn create_test_demographics(env: &Env) -> Demographics {
    Demographics {
        full_name: String::from_str(env, "John Doe"),
        date_of_birth: String::from_str(env, "1990-01-01"),
        gender: Some(String::from_str(env, "Male")),
        address: Some(String::from_str(env, "123 Main St, City, State")),
        phone: Some(String::from_str(env, "+1234567890")),
        email: Some(String::from_str(env, "john.doe@example.com")),
    }
}

fn create_test_emergency_contact(env: &Env) -> EmergencyContact {
    EmergencyContact {
        name: String::from_str(env, "Jane Doe"),
        relationship: String::from_str(env, "Spouse"),
        phone: String::from_str(env, "+1987654321"),
    }
}

#[test]
fn create_patient_profile_success() {
    let (env, client, _admin) = setup_test_env();
    
    let patient = Address::generate(&env);
    let patient_name = String::from_str(&env, "John Doe");
    
    // Register user as patient
    client.register_user(&patient, &Role::Patient, &patient_name);
    
    // Create demographics
    let demographics = create_test_demographics(&env);
    let insurance_data = Bytes::from_array(&env, &[1u8, 2u8, 3u8, 4u8]);
    
    // Create patient profile
    client.create_patient_profile(&patient, &demographics, &insurance_data);
    
    // Verify profile was created
    let profile = client.get_patient_profile(&patient);
    assert_eq!(profile.owner, patient);
    assert_eq!(profile.demographics.full_name, demographics.full_name);
    assert_eq!(profile.insurance_encrypted, insurance_data);
    assert_eq!(profile.medical_history.len(), 0);
    assert_eq!(profile.emergency_contacts.len(), 0);
}

#[test]
fn create_patient_profile_already_exists_fails() {
    let (env, client, _admin) = setup_test_env();
    
    let patient = Address::generate(&env);
    let patient_name = String::from_str(&env, "John Doe");
    
    // Register user as patient
    client.register_user(&patient, &Role::Patient, &patient_name);
    
    let demographics = create_test_demographics(&env);
    let insurance_data = Bytes::from_array(&env, &[1u8, 2u8, 3u8, 4u8]);
    
    // Create patient profile first time
    client.create_patient_profile(&patient, &demographics, &insurance_data);
    
    // Try to create again - should fail
    let result = client.try_create_patient_profile(&patient, &demographics, &insurance_data);
    assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
}

#[test]
fn create_patient_profile_non_patient_fails() {
    let (env, client, _admin) = setup_test_env();
    
    let user = Address::generate(&env);
    let user_name = String::from_str(&env, "Dr. Smith");
    
    // Register user as optometrist (not patient)
    client.register_user(&user, &Role::Optometrist, &user_name);
    
    let demographics = create_test_demographics(&env);
    let insurance_data = Bytes::from_array(&env, &[1u8, 2u8, 3u8, 4u8]);
    
    // Try to create patient profile - should fail
    let result = client.try_create_patient_profile(&user, &demographics, &insurance_data);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn update_demographics_by_owner_success() {
    let (env, client, _admin) = setup_test_env();
    
    let patient = Address::generate(&env);
    let patient_name = String::from_str(&env, "John Doe");
    
    // Setup patient profile
    client.register_user(&patient, &Role::Patient, &patient_name);
    let demographics = create_test_demographics(&env);
    let insurance_data = Bytes::from_array(&env, &[1u8, 2u8, 3u8, 4u8]);
    client.create_patient_profile(&patient, &demographics, &insurance_data);
    
    // Update demographics
    let new_demographics = Demographics {
        full_name: String::from_str(&env, "John Smith"),
        date_of_birth: String::from_str(&env, "1990-01-01"),
        gender: Some(String::from_str(&env, "Male")),
        address: Some(String::from_str(&env, "456 Oak Ave, City, State")),
        phone: Some(String::from_str(&env, "+1111111111")),
        email: Some(String::from_str(&env, "john.smith@example.com")),
    };
    
    client.update_demographics(&patient, &patient, &new_demographics);
    
    // Verify update
    let profile = client.get_patient_profile(&patient);
    assert_eq!(profile.demographics.full_name, new_demographics.full_name);
    assert_eq!(profile.demographics.address, new_demographics.address);
}

#[test]
fn update_demographics_by_non_owner_fails() {
    let (env, client, _admin) = setup_test_env();
    
    let patient = Address::generate(&env);
    let other_user = Address::generate(&env);
    let patient_name = String::from_str(&env, "John Doe");
    let other_name = String::from_str(&env, "Jane Smith");
    
    // Setup patient profile
    client.register_user(&patient, &Role::Patient, &patient_name);
    client.register_user(&other_user, &Role::Patient, &other_name);
    
    let demographics = create_test_demographics(&env);
    let insurance_data = Bytes::from_array(&env, &[1u8, 2u8, 3u8, 4u8]);
    client.create_patient_profile(&patient, &demographics, &insurance_data);
    
    // Try to update demographics as different user without access
    let new_demographics = create_test_demographics(&env);
    let result = client.try_update_demographics(&other_user, &patient, &new_demographics);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn add_medical_record_reference_by_provider_with_access_success() {
    let (env, client, _admin) = setup_test_env();
    
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let patient_name = String::from_str(&env, "John Doe");
    let provider_name = String::from_str(&env, "Dr. Smith");
    
    // Setup users
    client.register_user(&patient, &Role::Patient, &patient_name);
    client.register_user(&provider, &Role::Optometrist, &provider_name);
    
    // Create patient profile
    let demographics = create_test_demographics(&env);
    let insurance_data = Bytes::from_array(&env, &[1u8, 2u8, 3u8, 4u8]);
    client.create_patient_profile(&patient, &demographics, &insurance_data);
    
    // Grant access to provider
    client.grant_access(&patient, &provider, &AccessLevel::Write, &86400);
    
    // Create a medical record
    let data_hash = String::from_str(&env, "QmHash123");
    let record_id = client.add_record(&patient, &provider, &RecordType::Examination, &data_hash);
    
    // Add record reference to patient profile
    client.add_medical_record_reference(&provider, &patient, &record_id);
    
    // Verify record reference was added
    let profile = client.get_patient_profile(&patient);
    assert_eq!(profile.medical_history.len(), 1);
    assert_eq!(profile.medical_history.get(0).unwrap(), record_id);
}

#[test]
fn add_emergency_contact_success() {
    let (env, client, _admin) = setup_test_env();
    
    let patient = Address::generate(&env);
    let patient_name = String::from_str(&env, "John Doe");
    
    // Setup patient profile
    client.register_user(&patient, &Role::Patient, &patient_name);
    let demographics = create_test_demographics(&env);
    let insurance_data = Bytes::from_array(&env, &[1u8, 2u8, 3u8, 4u8]);
    client.create_patient_profile(&patient, &demographics, &insurance_data);
    
    // Add emergency contact
    let contact = create_test_emergency_contact(&env);
    client.add_emergency_contact(&patient, &patient, &contact);
    
    // Verify contact was added
    let profile = client.get_patient_profile(&patient);
    assert_eq!(profile.emergency_contacts.len(), 1);
    let stored_contact = profile.emergency_contacts.get(0).unwrap();
    assert_eq!(stored_contact.name, contact.name);
    assert_eq!(stored_contact.relationship, contact.relationship);
    assert_eq!(stored_contact.phone, contact.phone);
}

#[test]
fn insurance_is_stored_as_encrypted_blob() {
    let (env, client, _admin) = setup_test_env();
    
    let patient = Address::generate(&env);
    let patient_name = String::from_str(&env, "John Doe");
    
    // Setup patient profile
    client.register_user(&patient, &Role::Patient, &patient_name);
    let demographics = create_test_demographics(&env);
    
    // Create encrypted insurance data (simulating client-side encryption)
    let original_insurance_data = Bytes::from_array(&env, &[0x12, 0x34, 0x56, 0x78, 0xAB, 0xCD, 0xEF]);
    client.create_patient_profile(&patient, &demographics, &original_insurance_data);
    
    // Verify the encrypted data is stored exactly as provided
    let profile = client.get_patient_profile(&patient);
    assert_eq!(profile.insurance_encrypted, original_insurance_data);
    
    // Update insurance data
    let new_insurance_data = Bytes::from_array(&env, &[0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA]);
    client.update_insurance_encrypted(&patient, &patient, &new_insurance_data);
    
    // Verify updated data
    let updated_profile = client.get_patient_profile(&patient);
    assert_eq!(updated_profile.insurance_encrypted, new_insurance_data);
}

#[test]
fn remove_emergency_contact_success_and_out_of_range_fails() {
    let (env, client, _admin) = setup_test_env();
    
    let patient = Address::generate(&env);
    let patient_name = String::from_str(&env, "John Doe");
    
    // Setup patient profile
    client.register_user(&patient, &Role::Patient, &patient_name);
    let demographics = create_test_demographics(&env);
    let insurance_data = Bytes::from_array(&env, &[1u8, 2u8, 3u8, 4u8]);
    client.create_patient_profile(&patient, &demographics, &insurance_data);
    
    // Add two emergency contacts
    let contact1 = EmergencyContact {
        name: String::from_str(&env, "Jane Doe"),
        relationship: String::from_str(&env, "Spouse"),
        phone: String::from_str(&env, "+1111111111"),
    };
    let contact2 = EmergencyContact {
        name: String::from_str(&env, "Bob Doe"),
        relationship: String::from_str(&env, "Brother"),
        phone: String::from_str(&env, "+2222222222"),
    };
    
    client.add_emergency_contact(&patient, &patient, &contact1);
    client.add_emergency_contact(&patient, &patient, &contact2);
    
    // Verify both contacts exist
    let profile = client.get_patient_profile(&patient);
    assert_eq!(profile.emergency_contacts.len(), 2);
    
    // Remove first contact (index 0)
    client.remove_emergency_contact(&patient, &patient, &0);
    
    // Verify contact was removed
    let profile = client.get_patient_profile(&patient);
    assert_eq!(profile.emergency_contacts.len(), 1);
    assert_eq!(profile.emergency_contacts.get(0).unwrap().name, contact2.name);
    
    // Try to remove contact at invalid index
    let result = client.try_remove_emergency_contact(&patient, &patient, &5);
    assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
}

#[test]
fn get_patient_profile_not_found() {
    let (env, client, _admin) = setup_test_env();
    
    let patient = Address::generate(&env);
    
    // Try to get profile that doesn't exist
    let result = client.try_get_patient_profile(&patient);
    assert_eq!(result, Err(Ok(ContractError::UserNotFound)));
}

#[test]
fn update_insurance_by_admin_success() {
    let (env, client, admin) = setup_test_env();
    
    let patient = Address::generate(&env);
    let patient_name = String::from_str(&env, "John Doe");
    
    // Setup patient profile
    client.register_user(&patient, &Role::Patient, &patient_name);
    let demographics = create_test_demographics(&env);
    let insurance_data = Bytes::from_array(&env, &[1u8, 2u8, 3u8, 4u8]);
    client.create_patient_profile(&patient, &demographics, &insurance_data);
    
    // Admin updates insurance data
    let new_insurance_data = Bytes::from_array(&env, &[5u8, 6u8, 7u8, 8u8]);
    client.update_insurance_encrypted(&admin, &patient, &new_insurance_data);
    
    // Verify update
    let profile = client.get_patient_profile(&patient);
    assert_eq!(profile.insurance_encrypted, new_insurance_data);
}

#[test]
fn update_insurance_by_unauthorized_user_fails() {
    let (env, client, _admin) = setup_test_env();
    
    let patient = Address::generate(&env);
    let other_user = Address::generate(&env);
    let patient_name = String::from_str(&env, "John Doe");
    let other_name = String::from_str(&env, "Jane Smith");
    
    // Setup users
    client.register_user(&patient, &Role::Patient, &patient_name);
    client.register_user(&other_user, &Role::Patient, &other_name);
    
    // Create patient profile
    let demographics = create_test_demographics(&env);
    let insurance_data = Bytes::from_array(&env, &[1u8, 2u8, 3u8, 4u8]);
    client.create_patient_profile(&patient, &demographics, &insurance_data);
    
    // Try to update insurance as unauthorized user
    let new_insurance_data = Bytes::from_array(&env, &[5u8, 6u8, 7u8, 8u8]);
    let result = client.try_update_insurance_encrypted(&other_user, &patient, &new_insurance_data);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}