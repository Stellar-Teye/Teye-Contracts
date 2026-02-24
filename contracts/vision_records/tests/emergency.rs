mod common;

use common::setup_test_env;
use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, String, Vec};
use vision_records::{
    Certification, EmergencyCondition, EmergencyStatus, License, Location, VerificationStatus,
};

type TestContext = common::TestContext;

fn create_test_provider(ctx: &TestContext) -> Address {
    let provider = Address::generate(&ctx.env);
    let name = String::from_str(&ctx.env, "Emergency Provider");

    let mut licenses = Vec::new(&ctx.env);
    licenses.push_back(License {
        number: String::from_str(&ctx.env, "LIC123456"),
        issuing_authority: String::from_str(&ctx.env, "State Board"),
        issued_date: 1000,
        expiry_date: 2000,
        license_type: String::from_str(&ctx.env, "Ophthalmology"),
    });

    let mut specialties = Vec::new(&ctx.env);
    specialties.push_back(String::from_str(&ctx.env, "Emergency Care"));

    let mut certifications = Vec::new(&ctx.env);
    certifications.push_back(Certification {
        name: String::from_str(&ctx.env, "Board Certified"),
        issuer: String::from_str(&ctx.env, "Certification Board"),
        issued_date: 1000,
        expiry_date: 2000,
        credential_id: String::from_str(&ctx.env, "CERT123"),
    });

    let mut locations = Vec::new(&ctx.env);
    locations.push_back(Location {
        name: String::from_str(&ctx.env, "ER"),
        address: String::from_str(&ctx.env, "123 Main St"),
        city: String::from_str(&ctx.env, "City"),
        state: String::from_str(&ctx.env, "State"),
        zip: String::from_str(&ctx.env, "12345"),
        country: String::from_str(&ctx.env, "USA"),
    });

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &name,
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    // Verify the provider
    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Verified);

    provider
}

#[test]
fn test_grant_emergency_access() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(
        &ctx.env,
        "Patient unconscious, requires immediate vision assessment",
    );
    let mut contacts = Vec::new(&ctx.env);
    let contact = Address::generate(&ctx.env);
    contacts.push_back(contact.clone());

    let duration = 3600u64;
    let access_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::Unconscious,
        &attestation,
        &duration,
        &contacts,
    );

    assert!(access_id > 0);

    // Verify the emergency access was created
    let emergency_access = ctx.client.get_emergency_access(&access_id);
    assert_eq!(emergency_access.patient, patient);
    assert_eq!(emergency_access.requester, provider);
    assert_eq!(emergency_access.condition, EmergencyCondition::Unconscious);
    assert_eq!(emergency_access.status, EmergencyStatus::Active);
    assert_eq!(emergency_access.notified_contacts.len(), 1);
    assert_eq!(emergency_access.notified_contacts.get(0).unwrap(), contact);
}

#[test]
fn test_grant_emergency_access_requires_verified_provider() {
    let ctx = setup_test_env();
    let unverified_provider = Address::generate(&ctx.env);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);
    let duration = 3600u64;

    let result = ctx.client.try_grant_emergency_access(
        &unverified_provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration,
        &contacts,
    );

    assert!(result.is_err());
}

#[test]
fn test_grant_emergency_access_requires_attestation() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let empty_attestation = String::from_str(&ctx.env, "");
    let contacts = Vec::new(&ctx.env);
    let duration = 3600u64;

    let result = ctx.client.try_grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &empty_attestation,
        &duration,
        &contacts,
    );

    assert!(result.is_err());
}

#[test]
fn test_grant_emergency_access_max_duration() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);

    // Try to grant with duration > 24 hours
    let duration_too_long = 86401u64;
    let result = ctx.client.try_grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration_too_long,
        &contacts,
    );

    assert!(result.is_err());

    // Try with 0 duration
    let duration_zero = 0u64;
    let result = ctx.client.try_grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration_zero,
        &contacts,
    );

    assert!(result.is_err());
}

#[test]
fn test_check_emergency_access() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);
    let duration = 3600u64;

    let access_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration,
        &contacts,
    );

    // Check that emergency access is active
    let emergency_access = ctx.client.check_emergency_access(&patient, &provider);
    assert!(emergency_access.is_some());
    let access = emergency_access.unwrap();
    assert_eq!(access.id, access_id);
    assert_eq!(access.status, EmergencyStatus::Active);
}

#[test]
fn test_access_record_via_emergency() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);
    let duration = 3600u64;

    let access_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration,
        &contacts,
    );

    // Access records via emergency access
    ctx.client
        .access_record_via_emergency(&provider, &patient, &None);

    // Verify audit trail
    let audit_trail = ctx.client.get_emergency_audit_trail(&access_id);
    assert!(audit_trail.len() >= 2); // GRANTED and ACCESSED
}

#[test]
fn test_access_record_via_emergency_denied_without_access() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    // Try to access without granting emergency access
    let result = ctx
        .client
        .try_access_record_via_emergency(&provider, &patient, &None);

    assert!(result.is_err());
}

#[test]
fn test_revoke_emergency_access_by_patient() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);
    let duration = 3600u64;

    let access_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration,
        &contacts,
    );

    // Patient revokes the access
    ctx.client.revoke_emergency_access(&patient, &access_id);

    // Verify access is revoked
    let emergency_access = ctx.client.get_emergency_access(&access_id);
    assert_eq!(emergency_access.status, EmergencyStatus::Revoked);

    // Verify cannot access records anymore
    let result = ctx
        .client
        .try_access_record_via_emergency(&provider, &patient, &None);
    assert!(result.is_err());
}

#[test]
fn test_revoke_emergency_access_by_requester() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);
    let duration = 3600u64;

    let access_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration,
        &contacts,
    );

    // Requester revokes the access
    ctx.client.revoke_emergency_access(&provider, &access_id);

    // Verify access is revoked
    let emergency_access = ctx.client.get_emergency_access(&access_id);
    assert_eq!(emergency_access.status, EmergencyStatus::Revoked);
}

#[test]
fn test_revoke_emergency_access_by_admin() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);
    let duration = 3600u64;

    let access_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration,
        &contacts,
    );

    // Admin revokes the access
    ctx.client.revoke_emergency_access(&ctx.admin, &access_id);

    // Verify access is revoked
    let emergency_access = ctx.client.get_emergency_access(&access_id);
    assert_eq!(emergency_access.status, EmergencyStatus::Revoked);
}

#[test]
fn test_revoke_emergency_access_unauthorized() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);
    let unauthorized = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);
    let duration = 3600u64;

    let access_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration,
        &contacts,
    );

    // Unauthorized user tries to revoke
    let result = ctx
        .client
        .try_revoke_emergency_access(&unauthorized, &access_id);

    assert!(result.is_err());
}

#[test]
fn test_get_patient_emergency_accesses() {
    let ctx = setup_test_env();
    let provider1 = create_test_provider(&ctx);
    let provider2 = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);
    let duration1 = 3600u64;
    let duration2 = 7200u64;

    let access_id1 = ctx.client.grant_emergency_access(
        &provider1,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration1,
        &contacts,
    );

    let access_id2 = ctx.client.grant_emergency_access(
        &provider2,
        &patient,
        &EmergencyCondition::Unconscious,
        &attestation,
        &duration2,
        &contacts,
    );

    // Get all emergency accesses for patient
    let accesses = ctx.client.get_patient_emergency_accesses(&patient);
    assert_eq!(accesses.len(), 2);

    // Verify both accesses are present by checking IDs manually
    let mut found_id1 = false;
    let mut found_id2 = false;
    for i in 0..accesses.len() {
        let access = accesses.get(i).unwrap();
        if access.id == access_id1 {
            found_id1 = true;
        }
        if access.id == access_id2 {
            found_id2 = true;
        }
    }
    assert!(found_id1);
    assert!(found_id2);
}

#[test]
fn test_emergency_access_audit_trail() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);
    let duration = 3600u64;

    let access_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration,
        &contacts,
    );

    // Access records
    ctx.client
        .access_record_via_emergency(&provider, &patient, &None);

    // Revoke access
    ctx.client.revoke_emergency_access(&patient, &access_id);

    // Get audit trail
    let audit_trail = ctx.client.get_emergency_audit_trail(&access_id);
    assert!(audit_trail.len() >= 3); // GRANTED, ACCESSED, REVOKED

    // Verify audit entries by checking actions manually
    let granted_str = String::from_str(&ctx.env, "GRANTED");
    let accessed_str = String::from_str(&ctx.env, "ACCESSED");
    let revoked_str = String::from_str(&ctx.env, "REVOKED");

    let mut found_granted = false;
    let mut found_accessed = false;
    let mut found_revoked = false;

    for i in 0..audit_trail.len() {
        let entry = audit_trail.get(i).unwrap();
        if entry.action == granted_str {
            found_granted = true;
        }
        if entry.action == accessed_str {
            found_accessed = true;
        }
        if entry.action == revoked_str {
            found_revoked = true;
        }
    }

    assert!(found_granted);
    assert!(found_accessed);
    assert!(found_revoked);
}

#[test]
fn test_emergency_access_expires() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);
    let duration = 1u64; // 1 second duration

    let access_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration,
        &contacts,
    );

    // Fast forward time
    ctx.env.ledger().set_timestamp(1002);

    // Expire emergency accesses
    let expired_count = ctx.client.expire_emergency_accesses();
    assert!(expired_count >= 1);

    // Verify access is expired
    let emergency_access = ctx.client.get_emergency_access(&access_id);
    assert_eq!(emergency_access.status, EmergencyStatus::Expired);

    // Verify cannot access records anymore
    let result = ctx
        .client
        .try_access_record_via_emergency(&provider, &patient, &None);
    assert!(result.is_err());
}

#[test]
fn test_emergency_access_events() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);
    let contact = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let mut contacts = Vec::new(&ctx.env);
    contacts.push_back(contact.clone());
    let duration = 3600u64;

    // Grant emergency access - this should publish events
    let access_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration,
        &contacts,
    );

    // Verify the access was created successfully
    let emergency_access = ctx.client.get_emergency_access(&access_id);
    assert_eq!(emergency_access.status, EmergencyStatus::Active);
    assert_eq!(emergency_access.notified_contacts.len(), 1);

    // Access records via emergency - this should publish an event
    ctx.client
        .access_record_via_emergency(&provider, &patient, &None);

    // Verify audit trail shows the access was used
    let audit_trail = ctx.client.get_emergency_audit_trail(&access_id);
    assert!(audit_trail.len() >= 2); // GRANTED and ACCESSED

    // Revoke - this should publish an event
    ctx.client.revoke_emergency_access(&patient, &access_id);

    // Verify the access was revoked
    let revoked_access = ctx.client.get_emergency_access(&access_id);
    assert_eq!(revoked_access.status, EmergencyStatus::Revoked);

    // Verify audit trail shows all actions
    let final_audit_trail = ctx.client.get_emergency_audit_trail(&access_id);
    assert!(final_audit_trail.len() >= 3); // GRANTED, ACCESSED, REVOKED
}

#[test]
fn test_emergency_access_different_conditions() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let contacts = Vec::new(&ctx.env);
    let duration = 3600u64;

    // Test all emergency conditions
    let conditions = [
        EmergencyCondition::LifeThreatening,
        EmergencyCondition::Unconscious,
        EmergencyCondition::SurgicalEmergency,
        EmergencyCondition::Masscasualties,
    ];

    for condition in conditions.iter() {
        let access_id = ctx.client.grant_emergency_access(
            &provider,
            &patient,
            condition,
            &attestation,
            &duration,
            &contacts,
        );

        let emergency_access = ctx.client.get_emergency_access(&access_id);
        assert_eq!(emergency_access.condition, *condition);

        // Clean up
        ctx.client.revoke_emergency_access(&patient, &access_id);
    }
}

#[test]
fn test_emergency_access_multiple_contacts() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let attestation = String::from_str(&ctx.env, "Emergency situation");
    let mut contacts = Vec::new(&ctx.env);
    let contact1 = Address::generate(&ctx.env);
    let contact2 = Address::generate(&ctx.env);
    let contact3 = Address::generate(&ctx.env);
    contacts.push_back(contact1.clone());
    contacts.push_back(contact2.clone());
    contacts.push_back(contact3.clone());
    let duration = 3600u64;

    let access_id = ctx.client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::LifeThreatening,
        &attestation,
        &duration,
        &contacts,
    );

    let emergency_access = ctx.client.get_emergency_access(&access_id);
    assert_eq!(emergency_access.notified_contacts.len(), 3);

    // Verify all three contacts are in the notified list
    let mut found_contact1 = false;
    let mut found_contact2 = false;
    let mut found_contact3 = false;

    for i in 0..emergency_access.notified_contacts.len() {
        let contact = emergency_access.notified_contacts.get(i).unwrap();
        if contact == contact1 {
            found_contact1 = true;
        }
        if contact == contact2 {
            found_contact2 = true;
        }
        if contact == contact3 {
            found_contact3 = true;
        }
    }

    assert!(found_contact1, "Contact1 should be in notified contacts");
    assert!(found_contact2, "Contact2 should be in notified contacts");
    assert!(found_contact3, "Contact3 should be in notified contacts");
}
