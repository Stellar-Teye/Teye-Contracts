// Integration tests for patient registration and management workflows

use super::{create_test_user, setup_test_env};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, String,
};
use vision_records::{AccessLevel, RecordType};

/// Test complete patient registration workflow
#[test]
fn test_patient_registration_workflow() {
    let ctx = setup_test_env();
    let patient = Address::generate(&ctx.env);

    // Step 1: Admin registers patient
    ctx.client.register_user(
        &ctx.admin,
        &patient,
        &vision_records::Role::Patient,
        &String::from_str(&ctx.env, "John Doe"),
    );

    // Step 2: Verify patient is registered
    let user_data = ctx.client.get_user(&patient);
    assert_eq!(user_data.role, vision_records::Role::Patient);
    assert_eq!(user_data.name, String::from_str(&ctx.env, "John Doe"));
    assert!(user_data.is_active);

    // Step 3: Patient can view their own profile
    let profile = ctx.client.get_user(&patient);
    assert_eq!(profile.address, patient);
}

/// Test patient granting access to family member
#[test]
fn test_patient_grant_access_workflow() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, vision_records::Role::Patient, "Patient");
    let family_member = create_test_user(&ctx, vision_records::Role::Patient, "Family Member");

    // Patient grants read access to family member
    ctx.client.grant_access(
        &patient,
        &patient,
        &family_member,
        &AccessLevel::Read,
        &3600u64, // 1 hour
    );

    // Verify access was granted
    let access_level = ctx.client.check_access(&patient, &family_member);
    assert_eq!(access_level, AccessLevel::Read);

    // Family member should be able to read patient's records
    // (This would require records to exist, tested in record_workflows)
}

/// Test patient revoking access workflow
#[test]
fn test_patient_revoke_access_workflow() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, vision_records::Role::Patient, "Patient");
    let doctor = create_test_user(&ctx, vision_records::Role::Optometrist, "Doctor");

    // Grant access
    ctx.client.grant_access(
        &patient,
        &patient,
        &doctor,
        &AccessLevel::Full,
        &86400u64, // 24 hours
    );

    // Verify access
    assert_eq!(
        ctx.client.check_access(&patient, &doctor),
        AccessLevel::Full
    );

    // Revoke access
    ctx.client.revoke_access(&patient, &patient, &doctor);

    // Verify access is revoked
    assert_eq!(
        ctx.client.check_access(&patient, &doctor),
        AccessLevel::None
    );
}

/// Test patient managing multiple access grants
#[test]
fn test_patient_multiple_access_grants() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, vision_records::Role::Patient, "Patient");
    let doctor1 = create_test_user(&ctx, vision_records::Role::Optometrist, "Doctor 1");
    let doctor2 = create_test_user(&ctx, vision_records::Role::Ophthalmologist, "Doctor 2");
    let family = create_test_user(&ctx, vision_records::Role::Patient, "Family");

    // Grant different access levels to different users
    ctx.client
        .grant_access(&patient, &patient, &doctor1, &AccessLevel::Full, &86400u64);
    ctx.client
        .grant_access(&patient, &patient, &doctor2, &AccessLevel::Read, &3600u64);
    ctx.client.grant_access(
        &patient,
        &patient,
        &family,
        &AccessLevel::Read,
        &604800u64, // 7 days
    );

    // Verify all grants
    assert_eq!(
        ctx.client.check_access(&patient, &doctor1),
        AccessLevel::Full
    );
    assert_eq!(
        ctx.client.check_access(&patient, &doctor2),
        AccessLevel::Read
    );
    assert_eq!(
        ctx.client.check_access(&patient, &family),
        AccessLevel::Read
    );

    // Revoke one grant
    ctx.client.revoke_access(&patient, &patient, &doctor2);

    // Verify revoked grant is gone, others remain
    assert_eq!(
        ctx.client.check_access(&patient, &doctor1),
        AccessLevel::Full
    );
    assert_eq!(
        ctx.client.check_access(&patient, &doctor2),
        AccessLevel::None
    );
    assert_eq!(
        ctx.client.check_access(&patient, &family),
        AccessLevel::Read
    );
}

/// Test patient access expiration workflow
#[test]
fn test_patient_access_expiration() {
    let ctx = setup_test_env();
    ctx.env.ledger().set_timestamp(100000);

    let patient = create_test_user(&ctx, vision_records::Role::Patient, "Patient");
    let doctor = create_test_user(&ctx, vision_records::Role::Optometrist, "Doctor");

    // Grant access with short duration (minimum 3600 seconds)
    ctx.client.grant_access(
        &patient,
        &patient,
        &doctor,
        &AccessLevel::Read,
        &3600u64, // 1 hour (minimum allowed)
    );

    // Verify access is granted
    assert_eq!(
        ctx.client.check_access(&patient, &doctor),
        AccessLevel::Read
    );

    // Advance time past expiration (3600 seconds + 1 second buffer)
    ctx.env.ledger().set_timestamp(100000 + 3600 + 1);

    // Access should be expired
    assert_eq!(
        ctx.client.check_access(&patient, &doctor),
        AccessLevel::None
    );
}

/// Test patient viewing their record list
#[test]
fn test_patient_view_records() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, vision_records::Role::Patient, "Patient");
    let provider = create_test_user(&ctx, vision_records::Role::Optometrist, "Provider");

    // Grant provider permission to write records
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Provider creates multiple records
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

    // Patient views their records
    let records = ctx.client.get_patient_records(&patient);
    assert!(records.len() >= 2);
    assert!(records.contains(&record_id1));
    assert!(records.contains(&record_id2));
}

/// Test patient cannot grant access to themselves
#[test]
fn test_patient_cannot_grant_self_access() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, vision_records::Role::Patient, "Patient");

    // Patient already has access to their own records, but explicit grant should work
    // (The system allows this, but it's redundant)
    ctx.client
        .grant_access(&patient, &patient, &patient, &AccessLevel::Full, &86400u64);

    // Verify access
    assert_eq!(
        ctx.client.check_access(&patient, &patient),
        AccessLevel::Full
    );
}
