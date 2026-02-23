#![allow(clippy::unwrap_used, clippy::expect_used, clippy::arithmetic_side_effects)]
mod common;

use common::{create_test_record, create_test_user, setup_test_env};
use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address};
use soroban_sdk::{Env, String};
use vision_records::{
    AccessLevel, RecordType, Role, VisionRecordsContract, VisionRecordsContractClient,
};

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    // Explicitly test state before initialization
    assert!(!client.is_initialized());

    let ctx = setup_test_env();
    assert!(ctx.client.is_initialized());
    assert_eq!(ctx.client.get_admin(), ctx.admin);
}

#[test]
fn test_register_user() {
    let ctx = setup_test_env();
    let user = create_test_user(&ctx, Role::Optometrist, "Dr. Smith");
    let user_data = ctx.client.get_user(&user);

    assert_eq!(user_data.role, Role::Optometrist);
    assert!(user_data.is_active);
}

#[test]
fn test_add_and_get_record() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");

    let record_id = create_test_record(
        &ctx,
        &provider,
        &patient,
        &provider,
        RecordType::Examination,
        "e3b0c44298fc1c149afbf4c8996fb924",
    );

    assert_eq!(record_id, 1);
    let record = ctx.client.get_record(&record_id);
    assert_eq!(record.patient, patient);
    assert_eq!(record.provider, provider);
}

#[test]
fn test_access_control() {
    let ctx = setup_test_env();
    let patient = Address::generate(&ctx.env);
    let doctor = Address::generate(&ctx.env);

    assert_eq!(
        ctx.client.check_access(&patient, &doctor),
        AccessLevel::None
    );

    let current_time = 1000;
    ctx.env.ledger().set_timestamp(current_time);

    ctx.client
        .grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);
    assert_eq!(
        ctx.client.check_access(&patient, &doctor),
        AccessLevel::Read
    );

    // EXACT boundary check: at the exact second of expiration
    ctx.env.ledger().set_timestamp(current_time + 86400);
    assert_eq!(
        ctx.client.check_access(&patient, &doctor),
        AccessLevel::None
    );

    ctx.env.ledger().set_timestamp(current_time + 86401);
    assert_eq!(
        ctx.client.check_access(&patient, &doctor),
        AccessLevel::None
    );

    ctx.env.ledger().set_timestamp(current_time);
    ctx.client.revoke_access(&patient, &doctor);
    assert_eq!(
        ctx.client.check_access(&patient, &doctor),
        AccessLevel::None
    );
}

#[test]
fn test_get_record_count_and_patient_records() {
    let ctx = setup_test_env();
    assert_eq!(ctx.client.get_record_count(), 0);

    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");

    create_test_record(
        &ctx,
        &provider,
        &patient,
        &provider,
        RecordType::Examination,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );
    assert_eq!(ctx.client.get_record_count(), 1);

    create_test_record(
        &ctx,
        &provider,
        &patient,
        &provider,
        RecordType::Diagnosis,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    assert_eq!(ctx.client.get_record_count(), 2);

    let patient_records = ctx.client.get_patient_records(&patient);
    assert_eq!(patient_records.len(), 2);
    // Validate returned record IDs
    assert_eq!(patient_records.get(0).unwrap_or(0), 1);
    assert_eq!(patient_records.get(1).unwrap_or(0), 2);
}

#[test]
fn test_add_record_unauthorized_and_admin() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let random_user = create_test_user(&ctx, Role::Patient, "Random");
    let hash = String::from_str(&ctx.env, "cccccccccccccccccccccccccccccccc");
    // Random user cannot add typical record
    let res = ctx.client.try_add_record(
        &random_user,
        &patient,
        &random_user,
        &RecordType::Examination,
        &hash,
    );
    assert!(res.is_err());

    // SystemAdmin (ctx.admin) should be able to add a record even if they are not the provider.
    // If the `!` in `!rbac::has_permission(..., SystemAdmin)` is deleted, this will incorrectly fail.
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let admin_res = ctx.client.try_add_record(
        &ctx.admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &hash,
    );
    assert!(admin_res.is_ok());
}

#[test]
fn test_events_and_version() {
    use soroban_sdk::testutils::Events;
    let ctx = setup_test_env();

    // Test base version mutant `result -> 0`
    assert_eq!(ctx.client.version(), 1);

    assert_eq!(ctx.client.version(), 1);

    // Test initialization event by creating a fresh contract instance
    let contract_id2 = ctx.env.register(vision_records::VisionRecordsContract, ());
    let client2 = vision_records::VisionRecordsContractClient::new(&ctx.env, &contract_id2);
    client2.initialize(&ctx.admin);
    assert_eq!(ctx.env.events().all().len(), 1); // Kills publish_initialized missed mutant

    // Test register user event
    let user = Address::generate(&ctx.env);
    ctx.client.register_user(
        &ctx.admin,
        &user,
        &Role::Patient,
        &String::from_str(&ctx.env, "Patient Profile"),
    );
    assert_eq!(ctx.env.events().all().len(), 1); // Kills publish_user_registered mutant

    // Test add record event
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let hash = String::from_str(&ctx.env, "dddddddddddddddddddddddddddddddd");
    ctx.client
        .add_record(&provider, &user, &provider, &RecordType::Examination, &hash);
    assert_eq!(ctx.env.events().all().len(), 1); // Kills publish_record_added mutant

    // Test access grant/revoke event
    ctx.client
        .grant_access(&user, &user, &provider, &AccessLevel::Read, &86400);
    assert_eq!(ctx.env.events().all().len(), 1); // Kills publish_access_granted mutant

    ctx.client.revoke_access(&user, &provider);
    assert_eq!(ctx.env.events().all().len(), 1); // Kills publish_access_revoked mutant
}
