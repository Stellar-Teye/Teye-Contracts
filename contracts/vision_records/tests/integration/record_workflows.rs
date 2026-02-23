// Integration tests for record creation and access workflows

use super::{create_test_user, setup_test_env};
use soroban_sdk::{testutils::Ledger, String};
use vision_records::{AccessLevel, RecordType, Role};

/// Test complete record creation workflow
#[test]
fn test_record_creation_workflow() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");

    // Grant provider permission
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Provider creates record
    let data_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let record_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    assert_eq!(record_id, 1);

    // Verify record exists
    let record = ctx.client.get_record(&provider, &record_id);
    assert_eq!(record.patient, patient);
    assert_eq!(record.provider, provider);
    assert_eq!(record.record_type, RecordType::Examination);
    assert_eq!(record.data_hash, data_hash);
}

/// Test record access by different users
#[test]
fn test_record_access_workflow() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let doctor = create_test_user(&ctx, Role::Ophthalmologist, "Doctor");
    let family = create_test_user(&ctx, Role::Patient, "Family");

    // Revoke ReadAnyRecord from doctor to test access grants
    ctx.client.revoke_custom_permission(
        &ctx.admin,
        &doctor,
        &vision_records::Permission::ReadAnyRecord,
    );

    // Grant provider permission
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Create record
    let data_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let record_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    // Patient can read their own record
    let record = ctx.client.get_record(&patient, &record_id);
    assert_eq!(record.id, record_id);

    // Provider can read their own record
    let record = ctx.client.get_record(&provider, &record_id);
    assert_eq!(record.id, record_id);

    // Doctor cannot read without access (ReadAnyRecord revoked)
    let result = ctx.client.try_get_record(&doctor, &record_id);
    assert!(result.is_err());

    // Patient grants read access to doctor
    ctx.client
        .grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400u64);

    // Now doctor can read
    let record = ctx.client.get_record(&doctor, &record_id);
    assert_eq!(record.id, record_id);

    // Family cannot read without access
    let result = ctx.client.try_get_record(&family, &record_id);
    assert!(result.is_err());

    // Patient grants read access to family
    ctx.client
        .grant_access(&patient, &patient, &family, &AccessLevel::Read, &604800u64);

    // Now family can read
    let record = ctx.client.get_record(&family, &record_id);
    assert_eq!(record.id, record_id);
}

/// Test multiple record types workflow
#[test]
fn test_multiple_record_types_workflow() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");

    // Grant provider permission
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Create different record types
    let exam_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let exam_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &exam_hash,
    );

    let presc_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let presc_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Prescription,
        &presc_hash,
    );

    let diag_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdI");
    let diag_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Diagnosis,
        &diag_hash,
    );

    let treat_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdJ");
    let treat_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Treatment,
        &treat_hash,
    );

    // Verify all records exist
    assert_eq!(exam_id, 1);
    assert_eq!(presc_id, 2);
    assert_eq!(diag_id, 3);
    assert_eq!(treat_id, 4);

    // Verify record types
    let exam_record = ctx.client.get_record(&patient, &exam_id);
    assert_eq!(exam_record.record_type, RecordType::Examination);

    let presc_record = ctx.client.get_record(&patient, &presc_id);
    assert_eq!(presc_record.record_type, RecordType::Prescription);

    let diag_record = ctx.client.get_record(&patient, &diag_id);
    assert_eq!(diag_record.record_type, RecordType::Diagnosis);

    let treat_record = ctx.client.get_record(&patient, &treat_id);
    assert_eq!(treat_record.record_type, RecordType::Treatment);

    // Patient can see all their records
    let records = ctx.client.get_patient_records(&patient);
    assert_eq!(records.len(), 4);
    assert!(records.contains(&exam_id));
    assert!(records.contains(&presc_id));
    assert!(records.contains(&diag_id));
    assert!(records.contains(&treat_id));
}

/// Test record access with different access levels
#[test]
fn test_record_access_levels_workflow() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let reader = create_test_user(&ctx, Role::Optometrist, "Reader");
    let writer = create_test_user(&ctx, Role::Optometrist, "Writer");
    let full_access = create_test_user(&ctx, Role::Optometrist, "Full Access");

    // Grant provider permission
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Create record
    let data_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let record_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    // Grant different access levels
    ctx.client
        .grant_access(&patient, &patient, &reader, &AccessLevel::Read, &86400u64);
    ctx.client
        .grant_access(&patient, &patient, &writer, &AccessLevel::Write, &86400u64);
    ctx.client.grant_access(
        &patient,
        &patient,
        &full_access,
        &AccessLevel::Full,
        &86400u64,
    );

    // All can read
    let _record1 = ctx.client.get_record(&reader, &record_id);
    let _record2 = ctx.client.get_record(&writer, &record_id);
    let _record3 = ctx.client.get_record(&full_access, &record_id);

    // Verify access levels
    assert_eq!(
        ctx.client.check_access(&patient, &reader),
        AccessLevel::Read
    );
    assert_eq!(
        ctx.client.check_access(&patient, &writer),
        AccessLevel::Write
    );
    assert_eq!(
        ctx.client.check_access(&patient, &full_access),
        AccessLevel::Full
    );
}

/// Test record access expiration workflow
#[test]
fn test_record_access_expiration_workflow() {
    let ctx = setup_test_env();
    ctx.env.ledger().set_timestamp(100000);

    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let doctor = create_test_user(&ctx, Role::Ophthalmologist, "Doctor");

    // Revoke ReadAnyRecord from doctor to test access expiration
    ctx.client.revoke_custom_permission(
        &ctx.admin,
        &doctor,
        &vision_records::Permission::ReadAnyRecord,
    );

    // Grant provider permission
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Create record
    let data_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdI");
    let record_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    // Grant access with short duration (minimum 3600 seconds)
    ctx.client.grant_access(
        &patient,
        &patient,
        &doctor,
        &AccessLevel::Read,
        &3600u64, // 1 hour (minimum allowed)
    );

    // Doctor can read
    let record = ctx.client.get_record(&doctor, &record_id);
    assert_eq!(record.id, record_id);

    // Advance time past expiration (3600 seconds + 1 second buffer)
    ctx.env.ledger().set_timestamp(100000 + 3600 + 1);

    // Doctor can no longer read (access expired)
    let result = ctx.client.try_get_record(&doctor, &record_id);
    assert!(result.is_err());
}

/// Test record access revocation workflow
#[test]
fn test_record_access_revocation_workflow() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let doctor = create_test_user(&ctx, Role::Ophthalmologist, "Doctor");

    // Revoke ReadAnyRecord from doctor to test access revocation
    ctx.client.revoke_custom_permission(
        &ctx.admin,
        &doctor,
        &vision_records::Permission::ReadAnyRecord,
    );

    // Grant provider permission
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Create record
    let data_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdJ");
    let record_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    // Grant access
    ctx.client
        .grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400u64);

    // Doctor can read
    let record = ctx.client.get_record(&doctor, &record_id);
    assert_eq!(record.id, record_id);

    // Patient revokes access
    ctx.client.revoke_access(&patient, &patient, &doctor);

    // Doctor can no longer read (access revoked, no ReadAnyRecord)
    let result = ctx.client.try_get_record(&doctor, &record_id);
    assert!(result.is_err());
}

/// Test multiple providers creating records for same patient
#[test]
fn test_multiple_providers_workflow() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider1 = create_test_user(&ctx, Role::Optometrist, "Provider 1");
    let provider2 = create_test_user(&ctx, Role::Ophthalmologist, "Provider 2");

    // Grant both providers permission
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider1,
        &vision_records::Permission::WriteRecord,
    );
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider2,
        &vision_records::Permission::WriteRecord,
    );

    // Provider 1 creates record
    let hash1 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let record_id1 = ctx.client.add_record(
        &provider1,
        &patient,
        &provider1,
        &RecordType::Examination,
        &hash1,
    );

    // Provider 2 creates record
    let hash2 = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let record_id2 = ctx.client.add_record(
        &provider2,
        &patient,
        &provider2,
        &RecordType::Surgery,
        &hash2,
    );

    // Patient can see both records
    let records = ctx.client.get_patient_records(&patient);
    assert_eq!(records.len(), 2);
    assert!(records.contains(&record_id1));
    assert!(records.contains(&record_id2));

    // Both providers can read their own records
    let record1 = ctx.client.get_record(&provider1, &record_id1);
    assert_eq!(record1.provider, provider1);

    let record2 = ctx.client.get_record(&provider2, &record_id2);
    assert_eq!(record2.provider, provider2);

    // Revoke ReadAnyRecord from provider1 to test access control
    ctx.client.revoke_custom_permission(
        &ctx.admin,
        &provider1,
        &vision_records::Permission::ReadAnyRecord,
    );

    // Provider 1 cannot read provider 2's record without access
    let result = ctx.client.try_get_record(&provider1, &record_id2);
    assert!(result.is_err());
}

/// Test record audit logging workflow
#[test]
fn test_record_audit_workflow() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let doctor = create_test_user(&ctx, Role::Ophthalmologist, "Doctor");

    // Grant provider permission
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Create record
    let data_hash = String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdK");
    let record_id = ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    // Get audit log for record
    let audit_log = ctx.client.get_record_audit_log(&record_id);
    assert!(!audit_log.is_empty());

    // Grant access and read record
    ctx.client
        .grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400u64);

    let _record = ctx.client.get_record(&doctor, &record_id);

    // Check audit log includes access
    let audit_log_after = ctx.client.get_record_audit_log(&record_id);
    assert!(audit_log_after.len() > audit_log.len());
}
