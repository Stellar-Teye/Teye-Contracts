// Integration tests for provider onboarding and management workflows

use super::{create_test_user, setup_test_env};
use soroban_sdk::{testutils::Address as _, Address, String, Vec};
use vision_records::{RecordType, Role, VerificationStatus};

/// Test complete provider onboarding workflow
#[test]
fn test_provider_onboarding_workflow() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    // Step 1: Register provider as user
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Dr. Smith"),
    );

    // Step 2: Register provider details
    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    let provider_id = ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. John Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    assert!(provider_id > 0);

    // Step 3: Verify provider is registered
    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(provider_data.address, provider);
    assert_eq!(
        provider_data.verification_status,
        VerificationStatus::Pending
    );

    // Step 4: Admin verifies provider
    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Verified);

    // Step 5: Verify provider is now verified
    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(
        provider_data.verification_status,
        VerificationStatus::Verified
    );
    assert!(provider_data.verified_at.is_some());
}

/// Test provider with licenses and specialties
#[test]
fn test_provider_with_credentials() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    // Register provider as user
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Dr. Jones"),
    );

    // Create licenses
    let mut licenses = Vec::new(&ctx.env);
    let license = vision_records::License {
        number: String::from_str(&ctx.env, "OD-12345"),
        issuing_authority: String::from_str(&ctx.env, "California State Board"),
        license_type: String::from_str(&ctx.env, "Optometry"),
        issued_date: 1000000u64,
        expiry_date: 2000000u64,
    };
    licenses.push_back(license);

    // Create specialties
    let mut specialties = Vec::new(&ctx.env);
    specialties.push_back(String::from_str(&ctx.env, "Pediatric Optometry"));
    specialties.push_back(String::from_str(&ctx.env, "Contact Lenses"));

    // Create certifications
    let mut certifications = Vec::new(&ctx.env);
    let cert = vision_records::Certification {
        name: String::from_str(&ctx.env, "Board Certified"),
        issuer: String::from_str(&ctx.env, "ABO"),
        credential_id: String::from_str(&ctx.env, "CERT-12345"),
        issued_date: 1000000u64,
        expiry_date: 2000000u64,
    };
    certifications.push_back(cert);

    // Create locations
    let mut locations = Vec::new(&ctx.env);
    let location = vision_records::Location {
        name: String::from_str(&ctx.env, "Main Office"),
        address: String::from_str(&ctx.env, "123 Main St"),
        city: String::from_str(&ctx.env, "San Francisco"),
        state: String::from_str(&ctx.env, "CA"),
        zip: String::from_str(&ctx.env, "94102"),
        country: String::from_str(&ctx.env, "USA"),
    };
    locations.push_back(location);

    // Register provider with credentials
    let provider_id = ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Jane Jones"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    assert!(provider_id > 0);

    // Verify provider data
    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(provider_data.licenses.len(), 1);
    assert_eq!(provider_data.specialties.len(), 2);
    assert_eq!(provider_data.certifications.len(), 1);
    assert_eq!(provider_data.locations.len(), 1);
}

/// Test provider verification workflow
#[test]
fn test_provider_verification_workflow() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    // Register provider
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Dr. New"),
    );

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. New Provider"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    // Initially pending
    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(
        provider_data.verification_status,
        VerificationStatus::Pending
    );

    // Admin verifies
    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Verified);

    // Now verified
    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(
        provider_data.verification_status,
        VerificationStatus::Verified
    );

    // Admin can revoke verification
    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Rejected);

    // Now rejected
    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(
        provider_data.verification_status,
        VerificationStatus::Rejected
    );
}

/// Test provider creating records workflow
#[test]
fn test_provider_create_records_workflow() {
    let ctx = setup_test_env();
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let patient = create_test_user(&ctx, Role::Patient, "Patient");

    // Grant provider permission to write records
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Provider creates examination record
    let hash1 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let record_id1 = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash1,
    );

    assert_eq!(record_id1, 1);

    // Provider creates prescription record
    let hash2 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let record_id2 = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Prescription,
        &hash2,
    );

    assert_eq!(record_id2, 2);

    // Verify records exist
    let record1 = ctx.client.get_record(&provider, &record_id1);
    assert_eq!(record1.record_type, RecordType::Examination);
    assert_eq!(record1.patient, patient);
    assert_eq!(record1.provider, provider);

    let record2 = ctx.client.get_record(&provider, &record_id2);
    assert_eq!(record2.record_type, RecordType::Prescription);
}

/// Test provider searching for patients
#[test]
fn test_provider_search_patients() {
    let ctx = setup_test_env();
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let patient1 = create_test_user(&ctx, Role::Patient, "Patient 1");
    let patient2 = create_test_user(&ctx, Role::Patient, "Patient 2");
    let patient3 = create_test_user(&ctx, Role::Patient, "Patient 3");

    // Grant provider permission
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Provider creates records for multiple patients
    let hash1 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    ctx.client.add_record(
        &provider,
        &patient1,
        &provider,
        &RecordType::Examination,
        &hash1,
    );

    let hash2 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    ctx.client.add_record(
        &provider,
        &patient2,
        &provider,
        &RecordType::Examination,
        &hash2,
    );

    // Provider can get records for each patient
    let records1 = ctx.client.get_patient_records(&patient1);
    assert_eq!(records1.len(), 1);

    let records2 = ctx.client.get_patient_records(&patient2);
    assert_eq!(records2.len(), 1);

    let records3 = ctx.client.get_patient_records(&patient3);
    assert_eq!(records3.len(), 0);
}

/// Test verified provider gets rate limit bypass
#[test]
fn test_verified_provider_rate_limit_bypass() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    // Register and verify provider
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Dr. Verified"),
    );

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Verified Provider"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    // Set rate limit
    let operation = String::from_str(&ctx.env, "add_record");
    ctx.client
        .set_rate_limit_config(&ctx.admin, &operation, &1u32, &3600u64);

    // Grant permission
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    let patient = create_test_user(&ctx, Role::Patient, "Patient");

    // Before verification, rate limit applies
    let hash1 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash1,
    );

    // Second request should fail (rate limit)
    let hash2 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let result = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash2,
    );
    assert!(result.is_err());

    // Verify provider
    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Verified);

    // After verification, bypass should be enabled
    assert!(ctx.client.has_rate_limit_bypass(&provider));

    // Now should be able to make multiple requests
    let hash3 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdI");
    let result3 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash3,
    );
    assert!(result3.is_ok());
}

/// Test provider can create records with default permission
#[test]
fn test_provider_has_default_permission() {
    let ctx = setup_test_env();
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let patient = create_test_user(&ctx, Role::Patient, "Patient");

    // Optometrists have WriteRecord permission by default
    // So they should be able to create records
    let hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let result = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash,
    );

    assert!(result.is_ok());
}
