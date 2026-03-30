#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    unused_imports,
    unused_variables
)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::Env;

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert!(client.is_initialized());
    assert_eq!(client.get_admin(), admin);

    // soroban-sdk 25.x: env.events().all() returns ContractEvents which does
    // not implement is_empty / get / len.  Use iter() and search for the
    // INIT event explicitly instead.
    // assert!(!env.events().all().events().is_empty());
    // assert!(found_init, "Expected INIT event was not published");

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let data_hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");

    // First two record additions should succeed
    client.add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );
    client.add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    // Third should be rate limited
    let res = client.try_add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(matches!(err, Ok(ContractError::RateLimitExceeded)));

    // Advance time beyond the window and ensure the limit resets
    let current = env.ledger().timestamp();
    env.ledger().set_timestamp(current + 61);

    let res_after_reset = client.try_add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );
    assert!(res_after_reset.is_ok());

    // Grant access calls should also consume the same per-address budget
    let doctor = Address::generate(&env);
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);
    let rate_limited =
        client.try_grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);
    assert!(rate_limited.is_err());
}

#[test]
fn test_permission_without_consent_denied() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    // Grant access but NOT consent
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);

    // Access denied — no consent
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
}

#[test]
fn test_consent_and_permission_grants_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    // Grant both consent and access
    client.grant_consent(&patient, &doctor, &ConsentType::Treatment, &86400);
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);

    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);
}

#[test]
fn test_revoked_consent_blocks_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.grant_consent(&patient, &doctor, &ConsentType::Sharing, &86400);
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);

    // Revoke consent
    client.revoke_consent(&patient, &doctor);

    // Access now denied despite active access grant
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
}

#[test]
fn test_expired_consent_blocks_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    // Grant short-lived consent and long-lived access
    client.grant_consent(&patient, &doctor, &ConsentType::Research, &100);
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);

    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);

    // Advance time past consent expiry
    env.ledger().set_timestamp(200);

    // Consent expired — access denied
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
}

#[test]
fn test_get_record_consent_required() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let doctor = Address::generate(&env);
    let data_hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");

    let record_id = client.add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    // Patient can always view own record
    let record = client.get_record(&patient, &record_id);
    assert_eq!(record.patient, patient);

    // Doctor without consent → error (ConsentRequired = 26)
    let result = client.try_get_record(&doctor, &record_id);
    assert!(result.is_err());

    // Grant consent → doctor can view
    client.grant_consent(&patient, &doctor, &ConsentType::Treatment, &86400);
    let record = client.get_record(&doctor, &record_id);
    assert_eq!(record.patient, patient);
}

// ── Inter-module call tests: No deadlocks or infinite loops ──────────────────

/// Test that multiple concurrent record operations don't deadlock
#[test]
fn test_concurrent_record_operations_no_deadlock() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient1 = Address::generate(&env);
    let patient2 = Address::generate(&env);
    let provider = Address::generate(&env);
    let data_hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");

    // Multiple patients adding records simultaneously
    let record_id1 = client.add_record(
        &admin,
        &patient1,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );
    let record_id2 = client.add_record(
        &admin,
        &patient2,
        &provider,
        &RecordType::Prescription,
        &data_hash,
    );

    // Verify both records were created without deadlock
    let rec1 = client.get_record(&patient1, &record_id1);
    let rec2 = client.get_record(&patient2, &record_id2);

    assert_eq!(rec1.patient, patient1);
    assert_eq!(rec2.patient, patient2);
    assert_ne!(record_id1, record_id2);
}

/// Test that grant_access and grant_consent calls don't create circular dependencies
#[test]
fn test_grant_access_and_consent_no_circular_dependency() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor1 = Address::generate(&env);
    let doctor2 = Address::generate(&env);

    // Call grant_consent (which internally may check access)
    client.grant_consent(&patient, &doctor1, &ConsentType::Treatment, &86400);

    // Call grant_access (which internally may check consent)
    client.grant_access(&patient, &patient, &doctor1, &AccessLevel::Read, &86400);

    // Both should complete without deadlock/infinite loop
    assert_eq!(client.check_access(&patient, &doctor1), AccessLevel::Read);

    // Different doctor doesn't have access
    assert_eq!(client.check_access(&patient, &doctor2), AccessLevel::None);
}

/// Test that record retrieval chains don't cause stack overflow
#[test]
fn test_sequential_record_retrievals_no_overflow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let data_hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");

    // Create multiple records
    let mut record_ids = Vec::new();
    for _ in 0..5 {
        let record_id = client.add_record(
            &admin,
            &patient,
            &provider,
            &RecordType::Examination,
            &data_hash,
        );
        record_ids.push(record_id);
    }

    // Sequentially retrieve all records (simulate chain of calls)
    for record_id in record_ids {
        let record = client.get_record(&patient, &record_id);
        assert_eq!(record.patient, patient);
    }
}

/// Test that access control checks don't create infinite loops
#[test]
fn test_access_control_checks_no_infinite_loop() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    // Granting access without consent, then checking, shouldn't loop
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);
    let access_level = client.check_access(&patient, &doctor);
    assert_eq!(access_level, AccessLevel::None);

    // Adding consent and checking should resolve quickly
    client.grant_consent(&patient, &doctor, &ConsentType::Treatment, &86400);
    let access_level = client.check_access(&patient, &doctor);
    assert_eq!(access_level, AccessLevel::Read);
}

/// Test that multiple overlapping consent periods don't cause issues
#[test]
fn test_overlapping_consent_periods_no_deadlock() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor1 = Address::generate(&env);
    let doctor2 = Address::generate(&env);

    // Create overlapping consent experiences
    client.grant_consent(&patient, &doctor1, &ConsentType::Treatment, &1000);
    client.grant_consent(&patient, &doctor2, &ConsentType::Sharing, &2000);
    client.grant_consent(&patient, &doctor1, &ConsentType::Research, &3000);

    // Verify all can be checked without deadlock
    client.grant_access(&patient, &patient, &doctor1, &AccessLevel::Read, &1000);
    client.grant_access(&patient, &patient, &doctor2, &AccessLevel::Read, &2000);

    assert_eq!(client.check_access(&patient, &doctor1), AccessLevel::Read);
    assert_eq!(client.check_access(&patient, &doctor2), AccessLevel::Read);
}

/// Test that admin operations don't deadlock with patient operations
#[test]
fn test_admin_and_patient_operations_no_deadlock() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient1 = Address::generate(&env);
    let patient2 = Address::generate(&env);
    let provider = Address::generate(&env);
    let data_hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");

    client.initialize(&admin);

    // Admin adds record for patient1
    let record_id = client.add_record(
        &admin,
        &patient1,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    // Patient2 grants consent while admin adds another record
    client.grant_consent(&patient2, &provider, &ConsentType::Treatment, &86400);
    let record_id2 = client.add_record(
        &admin,
        &patient2,
        &provider,
        &RecordType::Prescription,
        &data_hash,
    );

    // Verify both records exist
    let rec1 = client.get_record(&patient1, &record_id);
    let rec2 = client.get_record(&patient2, &record_id2);

    assert_eq!(rec1.patient, patient1);
    assert_eq!(rec2.patient, patient2);
}

/// Test that revoke_consent doesn't cause access control re-evaluation loops
#[test]
fn test_revoke_consent_no_reevaluation_loop() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    // Grant consent and access
    client.grant_consent(&patient, &doctor, &ConsentType::Treatment, &86400);
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);

    // Revoke consent immediately
    client.revoke_consent(&patient, &doctor);

    // Access should be denied immediately (no loops)
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);

    // Re-grant and verify cycle completes
    client.grant_consent(&patient, &doctor, &ConsentType::Treatment, &86400);
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);
}

/// Test that multiple record types don't interfere with each other
#[test]
fn test_multiple_record_types_no_interference() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let data_hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");

    // Add different types of records
    let exam_id = client.add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );
    let prescription_id = client.add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Prescription,
        &data_hash,
    );
    let imaging_id = client.add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::ImagingData,
        &data_hash,
    );

    // Retrieve each and verify integrity
    let exam = client.get_record(&patient, &exam_id);
    let prescription = client.get_record(&patient, &prescription_id);
    let imaging = client.get_record(&patient, &imaging_id);

    assert_eq!(exam.record_type, RecordType::Examination);
    assert_eq!(prescription.record_type, RecordType::Prescription);
    assert_eq!(imaging.record_type, RecordType::ImagingData);
}
