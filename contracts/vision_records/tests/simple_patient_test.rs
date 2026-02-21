#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Bytes, Env, String};
use vision_records::{
    patient::{Demographics, PatientProfile},
    Role, VisionRecordsContract, VisionRecordsContractClient,
};

#[test]
fn test_patient_profile_basic_functionality() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let patient_name = String::from_str(&env, "John Doe");
    
    // Register user as patient
    client.register_user(&patient, &Role::Patient, &patient_name);
    
    // Create demographics
    let demographics = Demographics {
        full_name: String::from_str(&env, "John Doe"),
        date_of_birth: String::from_str(&env, "1990-01-01"),
        gender: Some(String::from_str(&env, "Male")),
        address: Some(String::from_str(&env, "123 Main St")),
        phone: Some(String::from_str(&env, "+1234567890")),
        email: Some(String::from_str(&env, "john@example.com")),
    };
    
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