// Integration tests for emergency access scenarios

use super::{create_test_user, setup_test_env};
use soroban_sdk::{testutils::Ledger, String, Vec};
use vision_records::{EmergencyCondition, RecordType, Role};

/// Test complete emergency access workflow
#[test]
fn test_emergency_access_workflow() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let emergency_provider = create_test_user(&ctx, Role::Ophthalmologist, "Emergency Provider");

    // Register and verify emergency provider
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
        &vision_records::VerificationStatus::Verified,
    );

    // Grant provider permission to create records
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Create some records for patient
    let hash1 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash1,
    );

    // Emergency provider requests emergency access
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

    assert!(emergency_id > 0);

    // Emergency provider can now access records
    let records = ctx.client.get_patient_records(&patient);
    assert!(!records.is_empty());

    // Access a specific record via emergency access
    let record_id = records.get(0).unwrap();
    ctx.client
        .access_record_via_emergency(&emergency_provider, &patient, &Some(record_id));
}

/// Test emergency access with different conditions
#[test]
fn test_emergency_access_conditions() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");

    // Register and verify provider
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Provider"),
    );

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Provider"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client.verify_provider(
        &ctx.admin,
        &provider,
        &vision_records::VerificationStatus::Verified,
    );

    // Test different emergency conditions
    let conditions = vec![
        EmergencyCondition::LifeThreatening,
        EmergencyCondition::Unconscious,
        EmergencyCondition::SurgicalEmergency,
        EmergencyCondition::Masscasualties,
    ];

    for condition in conditions {
        let attestation = String::from_str(&ctx.env, "Emergency situation");
        let contacts = Vec::new(&ctx.env);

        let emergency_id = ctx.client.grant_emergency_access(
            &provider,
            &patient,
            &condition,
            &attestation,
            &3600u64,
            &contacts,
        );

        assert!(emergency_id > 0);

        // Verify emergency access exists
        let emergency = ctx.client.get_emergency_access(&emergency_id);
        let em = emergency;
        assert_eq!(em.condition, condition);
        assert_eq!(em.status, vision_records::EmergencyStatus::Active);

        // Revoke for next test
        ctx.client.revoke_emergency_access(&provider, &emergency_id);
    }
}

/// Test emergency access expiration workflow
#[test]
fn test_emergency_access_expiration() {
    let ctx = setup_test_env();
    ctx.env.ledger().set_timestamp(100000);

    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");

    // Register and verify provider
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Provider"),
    );

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Provider"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client.verify_provider(
        &ctx.admin,
        &provider,
        &vision_records::VerificationStatus::Verified,
    );

    // Grant emergency access with short duration
    let attestation = String::from_str(&ctx.env, "Emergency");
    let contacts = Vec::new(&ctx.env);

    let emergency_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &10u64, // 10 seconds
        &contacts,
    );

    // Verify access is active
    let emergency = ctx.client.get_emergency_access(&emergency_id);
    assert_eq!(emergency.status, vision_records::EmergencyStatus::Active);

    // Advance time past expiration
    ctx.env.ledger().set_timestamp(100011);

    // Expire emergency accesses
    ctx.client.expire_emergency_accesses();

    // Access should be expired
    let emergency = ctx.client.get_emergency_access(&emergency_id);
    assert_eq!(emergency.status, vision_records::EmergencyStatus::Expired);
}

/// Test emergency access revocation workflow
#[test]
fn test_emergency_access_revocation() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let admin = ctx.admin;

    // Register and verify provider
    ctx.client.register_user(
        &admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Provider"),
    );

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &admin,
        &provider,
        &String::from_str(&ctx.env, "Provider"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client.verify_provider(
        &admin,
        &provider,
        &vision_records::VerificationStatus::Verified,
    );

    // Grant emergency access
    let attestation = String::from_str(&ctx.env, "Emergency");
    let contacts = Vec::new(&ctx.env);

    let emergency_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &3600u64,
        &contacts,
    );

    // Verify access is active
    let emergency = ctx.client.get_emergency_access(&emergency_id);
    assert_eq!(emergency.status, vision_records::EmergencyStatus::Active);

    // Patient revokes access
    ctx.client.revoke_emergency_access(&patient, &emergency_id);

    // Access should be revoked
    let emergency = ctx.client.get_emergency_access(&emergency_id);
    assert_eq!(emergency.status, vision_records::EmergencyStatus::Revoked);
}

/// Test emergency access with multiple contacts
#[test]
fn test_emergency_access_with_contacts() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let contact1 = create_test_user(&ctx, Role::Patient, "Contact 1");
    let contact2 = create_test_user(&ctx, Role::Patient, "Contact 2");

    // Register and verify provider
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Provider"),
    );

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Provider"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client.verify_provider(
        &ctx.admin,
        &provider,
        &vision_records::VerificationStatus::Verified,
    );

    // Grant emergency access with multiple contacts
    let attestation = String::from_str(&ctx.env, "Emergency with contacts");
    let mut contacts = Vec::new(&ctx.env);
    contacts.push_back(contact1.clone());
    contacts.push_back(contact2.clone());

    let emergency_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &3600u64,
        &contacts,
    );

    // Verify emergency access includes contacts
    let emergency = ctx.client.get_emergency_access(&emergency_id);
    assert_eq!(emergency.notified_contacts.len(), 2);
}

/// Test emergency access audit trail
#[test]
fn test_emergency_access_audit() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");

    // Register and verify provider
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Provider"),
    );

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Provider"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client.verify_provider(
        &ctx.admin,
        &provider,
        &vision_records::VerificationStatus::Verified,
    );

    // Grant emergency access
    let attestation = String::from_str(&ctx.env, "Emergency audit test");
    let contacts = Vec::new(&ctx.env);

    let emergency_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &3600u64,
        &contacts,
    );

    // Get emergency audit log
    let audit_log = ctx.client.get_emergency_audit_trail(&emergency_id);
    assert!(!audit_log.is_empty());

    // Revoke access
    ctx.client.revoke_emergency_access(&provider, &emergency_id);

    // Audit log should include revocation
    let audit_log_after = ctx.client.get_emergency_audit_trail(&emergency_id);
    assert!(audit_log_after.len() > audit_log.len());
}

/// Test emergency access requires verified provider
#[test]
fn test_emergency_access_requires_verification() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let unverified_provider = create_test_user(&ctx, Role::Optometrist, "Unverified Provider");

    // Register provider but don't verify
    ctx.client.register_user(
        &ctx.admin,
        &unverified_provider,
        &Role::Optometrist,
        &String::from_str(&ctx.env, "Unverified Provider"),
    );

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &unverified_provider,
        &String::from_str(&ctx.env, "Unverified Provider"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    // Try to grant emergency access (should fail - not verified)
    let attestation = String::from_str(&ctx.env, "Emergency");
    let contacts = Vec::new(&ctx.env);

    let result = ctx.client.try_grant_emergency_access(
        &unverified_provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &3600u64,
        &contacts,
    );

    assert!(result.is_err());
}
