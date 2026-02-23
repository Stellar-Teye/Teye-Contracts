// End-to-end integration tests covering complete user workflows

use super::{create_test_user, setup_test_env};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, String, Vec,
};
use vision_records::{AccessLevel, EmergencyCondition, RecordType, Role, VerificationStatus};

/// Test complete patient journey from registration to record access
#[test]
fn test_complete_patient_journey() {
    let ctx = setup_test_env();

    // Step 1: Patient registration
    let patient = create_test_user(&ctx, Role::Patient, "John Doe");
    let patient_data = ctx.client.get_user(&patient);
    assert_eq!(patient_data.role, Role::Patient);
    assert!(patient_data.is_active);

    // Step 2: Provider registration and verification
    let provider = Address::generate(&ctx.env);
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Dr. Smith"),
    );

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    let _provider_id = ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. John Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Verified);

    // Step 3: Provider creates records
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    let hash1 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let record_id1 = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash1,
    );

    let hash2 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let record_id2 = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Prescription,
        &hash2,
    );

    // Step 4: Patient views their records
    let records = ctx.client.get_patient_records(&patient);
    assert_eq!(records.len(), 2);
    assert!(records.contains(&record_id1));
    assert!(records.contains(&record_id2));

    // Step 5: Patient grants access to family member
    let family = create_test_user(&ctx, Role::Patient, "Family Member");
    ctx.client.grant_access(
        &patient,
        &patient,
        &family,
        &AccessLevel::Read,
        &604800u64, // 7 days
    );

    // Step 6: Family member can read records
    let record = ctx.client.get_record(&family, &record_id1);
    assert_eq!(record.id, record_id1);

    // Step 7: Patient revokes access
    ctx.client.revoke_access(&patient, &patient, &family);

    // Step 8: Family member can no longer access
    let result = ctx.client.try_get_record(&family, &record_id1);
    assert!(result.is_err());
}

/// Test complete provider workflow from onboarding to record management
#[test]
fn test_complete_provider_workflow() {
    let ctx = setup_test_env();

    // Step 1: Provider registration
    let provider = Address::generate(&ctx.env);
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Dr. Provider"),
    );

    // Step 2: Provider details registration
    let mut licenses = Vec::new(&ctx.env);
    let license = vision_records::License {
        number: String::from_str(&ctx.env, "OD-12345"),
        issuing_authority: String::from_str(&ctx.env, "California State Board"),
        license_type: String::from_str(&ctx.env, "Optometry"),
        issued_date: 1000000u64,
        expiry_date: 2000000u64,
    };
    licenses.push_back(license);

    let mut specialties = Vec::new(&ctx.env);
    specialties.push_back(String::from_str(&ctx.env, "Pediatric Optometry"));

    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    let _provider_id = ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Provider"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    // Step 3: Provider verification
    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Verified);

    // Step 4: Grant permissions
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Step 5: Provider creates records for multiple patients
    let patient1 = create_test_user(&ctx, Role::Patient, "Patient 1");
    let patient2 = create_test_user(&ctx, Role::Patient, "Patient 2");

    let hash1 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let record_id1 = ctx.client.add_record(
        &provider,
        &patient1,
        &provider,
        &RecordType::Examination,
        &hash1,
    );

    let hash2 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let record_id2 = ctx.client.add_record(
        &provider,
        &patient2,
        &provider,
        &RecordType::Examination,
        &hash2,
    );

    // Step 6: Provider can access their records
    let record1 = ctx.client.get_record(&provider, &record_id1);
    assert_eq!(record1.patient, patient1);

    let record2 = ctx.client.get_record(&provider, &record_id2);
    assert_eq!(record2.patient, patient2);

    // Step 7: Provider can view patient records
    let patient1_records = ctx.client.get_patient_records(&patient1);
    assert_eq!(patient1_records.len(), 1);
    assert!(patient1_records.contains(&record_id1));
}

/// Test emergency access complete workflow
#[test]
fn test_complete_emergency_workflow() {
    let ctx = setup_test_env();

    // Setup: Patient with existing records
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let regular_provider = create_test_user(&ctx, Role::Optometrist, "Regular Provider");

    ctx.client.grant_custom_permission(
        &ctx.admin,
        &regular_provider,
        &vision_records::Permission::WriteRecord,
    );

    let hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let record_id = ctx.client.add_record(
        &regular_provider,
        &patient,
        &regular_provider,
        &RecordType::Examination,
        &hash,
    );

    // Step 1: Emergency provider registration and verification
    let emergency_provider = Address::generate(&ctx.env);
    ctx.client.register_user(
        &ctx.admin,
        &emergency_provider,
        &Role::Ophthalmologist,
        &String::from_str(&ctx.env, "Emergency Provider"),
    );

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &emergency_provider,
        &String::from_str(&ctx.env, "Emergency Provider"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client.verify_provider(
        &ctx.admin,
        &emergency_provider,
        &VerificationStatus::Verified,
    );

    // Step 2: Emergency access request
    let attestation = String::from_str(
        &ctx.env,
        "Patient unconscious, requires immediate vision assessment",
    );
    let mut contacts = Vec::new(&ctx.env);
    contacts.push_back(patient.clone());

    let emergency_id = ctx.client.grant_emergency_access(
        &emergency_provider,
        &patient,
        &EmergencyCondition::Unconscious,
        &attestation,
        &3600u64, // 1 hour
        &contacts,
    );

    // Step 3: Emergency provider accesses records
    ctx.client
        .access_record_via_emergency(&emergency_provider, &patient, &Some(record_id));

    // Step 4: Emergency access expires
    ctx.env.ledger().set_timestamp(3601);

    // Expire emergency accesses
    ctx.client.expire_emergency_accesses();

    // Step 5: Emergency access is no longer valid
    let emergency = ctx.client.get_emergency_access(&emergency_id);
    assert_eq!(emergency.status, vision_records::EmergencyStatus::Expired);
}

/// Test multi-provider collaboration workflow
#[test]
fn test_multi_provider_collaboration() {
    let ctx = setup_test_env();

    // Setup: Patient
    let patient = create_test_user(&ctx, Role::Patient, "Patient");

    // Setup: Multiple providers
    let optometrist = create_test_user(&ctx, Role::Optometrist, "Optometrist");
    let ophthalmologist = create_test_user(&ctx, Role::Ophthalmologist, "Ophthalmologist");

    ctx.client.grant_custom_permission(
        &ctx.admin,
        &optometrist,
        &vision_records::Permission::WriteRecord,
    );
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &ophthalmologist,
        &vision_records::Permission::WriteRecord,
    );

    // Optometrist creates initial examination
    let exam_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let exam_id = ctx.client.add_record(
        &optometrist,
        &patient,
        &optometrist,
        &RecordType::Examination,
        &exam_hash,
    );

    // Patient grants access to ophthalmologist
    ctx.client.grant_access(
        &patient,
        &patient,
        &ophthalmologist,
        &AccessLevel::Read,
        &86400u64,
    );

    // Ophthalmologist reviews and creates diagnosis
    let _exam_record = ctx.client.get_record(&ophthalmologist, &exam_id);

    let diag_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdI");
    let diag_id = ctx.client.add_record(
        &ophthalmologist,
        &patient,
        &ophthalmologist,
        &RecordType::Diagnosis,
        &diag_hash,
    );

    // Optometrist can see ophthalmologist's diagnosis
    ctx.client.grant_access(
        &patient,
        &patient,
        &optometrist,
        &AccessLevel::Read,
        &86400u64,
    );

    let diag_record = ctx.client.get_record(&optometrist, &diag_id);
    assert_eq!(diag_record.record_type, RecordType::Diagnosis);
    assert_eq!(diag_record.provider, ophthalmologist);
}

/// Test appointment scheduling integration workflow
#[test]
fn test_appointment_integration_workflow() {
    let ctx = setup_test_env();
    ctx.env.ledger().set_timestamp(100000);

    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");

    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Schedule appointment
    let appointment_time = 200000u64; // Future time
    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &vision_records::AppointmentType::Examination,
        &appointment_time,
        &60u32, // 60 minutes
        &Some(String::from_str(&ctx.env, "Routine eye exam")),
    );

    assert!(appointment_id > 0);

    // Confirm appointment
    ctx.client.confirm_appointment(&provider, &appointment_id);

    // Advance time to appointment
    ctx.env.ledger().set_timestamp(200000);

    // Complete appointment
    ctx.client.complete_appointment(&provider, &appointment_id);

    // Create record from appointment
    let hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let record_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash,
    );

    // Verify record exists
    let record = ctx.client.get_record(&patient, &record_id);
    assert_eq!(record.id, record_id);
}

/// Test rate limiting integration with workflows
#[test]
fn test_rate_limiting_integration() {
    let ctx = setup_test_env();
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let patient = create_test_user(&ctx, Role::Patient, "Patient");

    // Set rate limit
    let operation = String::from_str(&ctx.env, "add_record");
    ctx.client
        .set_rate_limit_config(&ctx.admin, &operation, &2u32, &3600u64);

    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // First two requests should succeed
    let hash1 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let result1 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash1,
    );
    assert!(result1.is_ok());

    let hash2 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let result2 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash2,
    );
    assert!(result2.is_ok());

    // Third request should fail (rate limit)
    let hash3 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdI");
    let result3 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash3,
    );
    assert!(result3.is_err());
}

/// Test audit logging integration across workflows
#[test]
fn test_audit_logging_integration() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let doctor = create_test_user(&ctx, Role::Ophthalmologist, "Doctor");

    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Create record
    let hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let record_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash,
    );

    // Get audit log for record
    let audit_log = ctx.client.get_record_audit_log(&record_id);
    assert!(!audit_log.is_empty());

    // Grant access
    ctx.client
        .grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400u64);

    // Read record
    let _record = ctx.client.get_record(&doctor, &record_id);

    // Audit log should include access
    let audit_log_after = ctx.client.get_record_audit_log(&record_id);
    assert!(audit_log_after.len() > audit_log.len());

    // Get user audit log
    let user_audit = ctx.client.get_user_audit_log(&doctor);
    assert!(!user_audit.is_empty());
}
