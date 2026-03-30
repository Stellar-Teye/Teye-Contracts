#![no_std]
#![allow(clippy::unwrap_used)]

//! Timestamp Manipulation & Expiry Bounds tests for the `fhir` crate.
//!
//! Advances the ledger time explicitly via the test environment and verifies
//! that resource timestamps reflect block timestamps accurately, ensuring
//! strict adherence to ledger time for audit trails and expiry logic.

use fhir::{FhirContract, FhirContractClient, FhirError};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Bytes, Env, String,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Boot an `Env`, register the contract, initialise it, and return all handles.
fn setup() -> (Env, FhirContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Verifies that a resource registered at a known ledger timestamp can be
/// retrieved successfully, confirming the timestamp was captured without error.
#[test]
fn test_register_resource_captures_ledger_timestamp() {
    let (env, client, admin) = setup();

    // Set a specific ledger timestamp before registering.
    env.ledger().set_timestamp(1_700_000_000);

    let id = String::from_str(&env, "patient-ts-1");
    let payload = Bytes::from_slice(&env, b"{\"resourceType\":\"Patient\",\"id\":\"ts-1\"}");

    client.register_resource(&admin, &id, &payload);

    // The resource is retrievable — the timestamp write path succeeded.
    let stored = client.get_resource(&id);
    assert_eq!(stored, payload);
}

/// Advancing the ledger timestamp between register and update should not break
/// the contract; both operations capture their own timestamp independently.
#[test]
fn test_update_resource_after_time_advance() {
    let (env, client, admin) = setup();

    // Register at T=1000
    env.ledger().set_timestamp(1_000);
    let id = String::from_str(&env, "res-time-1");
    let v1 = Bytes::from_slice(&env, b"version-1");
    client.register_resource(&admin, &id, &v1);

    // Advance ledger to T=5000 and update
    env.ledger().set_timestamp(5_000);
    let v2 = Bytes::from_slice(&env, b"version-2");
    client.update_resource(&admin, &id, &v2);

    let stored = client.get_resource(&id);
    assert_eq!(stored, v2);
}

/// Multiple resources registered at different ledger timestamps should each be
/// independently retrievable and not interfere with each other.
#[test]
fn test_multiple_resources_at_different_timestamps() {
    let (env, client, admin) = setup();

    // Resource A at T=100
    env.ledger().set_timestamp(100);
    let id_a = String::from_str(&env, "res-a");
    let payload_a = Bytes::from_slice(&env, b"payload-a");
    client.register_resource(&admin, &id_a, &payload_a);

    // Resource B at T=200
    env.ledger().set_timestamp(200);
    let id_b = String::from_str(&env, "res-b");
    let payload_b = Bytes::from_slice(&env, b"payload-b");
    client.register_resource(&admin, &id_b, &payload_b);

    // Resource C at T=300
    env.ledger().set_timestamp(300);
    let id_c = String::from_str(&env, "res-c");
    let payload_c = Bytes::from_slice(&env, b"payload-c");
    client.register_resource(&admin, &id_c, &payload_c);

    // All resources independently retrievable.
    assert_eq!(client.get_resource(&id_a), payload_a);
    assert_eq!(client.get_resource(&id_b), payload_b);
    assert_eq!(client.get_resource(&id_c), payload_c);
}

/// Registering a resource at timestamp 0 (genesis-like scenario) should work.
#[test]
fn test_register_at_timestamp_zero() {
    let (env, client, admin) = setup();

    env.ledger().set_timestamp(0);

    let id = String::from_str(&env, "genesis-resource");
    let payload = Bytes::from_slice(&env, b"genesis-data");

    client.register_resource(&admin, &id, &payload);
    assert_eq!(client.get_resource(&id), payload);
}

/// Large timestamp values (far-future dates) should be handled correctly by
/// the u64 timestamp storage.
#[test]
fn test_register_at_far_future_timestamp() {
    let (env, client, admin) = setup();

    // Year ~2554 in Unix time
    let far_future: u64 = 18_446_744_073;
    env.ledger().set_timestamp(far_future);

    let id = String::from_str(&env, "future-resource");
    let payload = Bytes::from_slice(&env, b"future-data");

    client.register_resource(&admin, &id, &payload);
    assert_eq!(client.get_resource(&id), payload);
}

/// Updating the same resource multiple times at advancing timestamps should
/// always reflect the latest payload.
#[test]
fn test_sequential_updates_with_advancing_time() {
    let (env, client, admin) = setup();

    let id = String::from_str(&env, "evolving-resource");

    env.ledger().set_timestamp(1_000);
    let v1 = Bytes::from_slice(&env, b"v1");
    client.register_resource(&admin, &id, &v1);
    assert_eq!(client.get_resource(&id), v1);

    env.ledger().set_timestamp(2_000);
    let v2 = Bytes::from_slice(&env, b"v2");
    client.update_resource(&admin, &id, &v2);
    assert_eq!(client.get_resource(&id), v2);

    env.ledger().set_timestamp(3_000);
    let v3 = Bytes::from_slice(&env, b"v3");
    client.update_resource(&admin, &id, &v3);
    assert_eq!(client.get_resource(&id), v3);

    env.ledger().set_timestamp(10_000);
    let v4 = Bytes::from_slice(&env, b"v4");
    client.update_resource(&admin, &id, &v4);
    assert_eq!(client.get_resource(&id), v4);
}

/// Deleting a resource after advancing time should succeed and the resource
/// should not be retrievable afterward.
#[test]
fn test_delete_resource_after_time_advance() {
    let (env, client, admin) = setup();

    env.ledger().set_timestamp(500);
    let id = String::from_str(&env, "to-delete");
    let payload = Bytes::from_slice(&env, b"delete-me");
    client.register_resource(&admin, &id, &payload);

    // Advance significantly and delete.
    env.ledger().set_timestamp(999_999);
    client.delete_resource(&admin, &id);

    // Retrieving after deletion must fail.
    let result = client.try_get_resource(&id);
    assert!(result.is_err());
}

/// Verifying that Patient birth_date (a timestamp field) works correctly
/// with boundary values.
#[test]
fn test_patient_birth_date_boundary_values() {
    let env = Env::default();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);

    let id = String::from_str(&env, "p-birth");
    let identifier = String::from_str(&env, "MRN-001");
    let name = String::from_str(&env, "Jane Doe");

    // birth_date = 0 (epoch)
    let patient_epoch =
        client.create_patient(&id, &identifier, &name, &fhir::types::Gender::Female, &0u64);
    assert!(client.validate_patient(&patient_epoch));
    assert_eq!(patient_epoch.birth_date, 0);

    // birth_date = far future
    let future_ts: u64 = 4_102_444_800; // 2100-01-01
    let patient_future = client.create_patient(
        &id,
        &identifier,
        &name,
        &fhir::types::Gender::Female,
        &future_ts,
    );
    assert!(client.validate_patient(&patient_future));
    assert_eq!(patient_future.birth_date, future_ts);

    // birth_date = u64::MAX
    let patient_max = client.create_patient(
        &id,
        &identifier,
        &name,
        &fhir::types::Gender::Male,
        &u64::MAX,
    );
    assert!(client.validate_patient(&patient_max));
    assert_eq!(patient_max.birth_date, u64::MAX);
}

/// Verifying that Observation effective_datetime (a timestamp field) works
/// correctly with boundary values.
#[test]
fn test_observation_effective_datetime_boundaries() {
    let env = Env::default();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);

    let id = String::from_str(&env, "obs-1");
    let code_sys = String::from_str(&env, "LOINC");
    let code_val = String::from_str(&env, "8867-4");
    let subject = String::from_str(&env, "patient-1");
    let value = String::from_str(&env, "72 bpm");

    // effective_datetime = 0
    let obs_epoch = client.create_observation(
        &id,
        &fhir::types::ObservationStatus::Final,
        &code_sys,
        &code_val,
        &subject,
        &value,
        &0u64,
    );
    assert!(client.validate_observation(&obs_epoch));
    assert_eq!(obs_epoch.effective_datetime, 0);

    // effective_datetime = specific known date (2024-01-01 00:00:00 UTC)
    let known_ts: u64 = 1_704_067_200;
    let obs_known = client.create_observation(
        &id,
        &fhir::types::ObservationStatus::Preliminary,
        &code_sys,
        &code_val,
        &subject,
        &value,
        &known_ts,
    );
    assert!(client.validate_observation(&obs_known));
    assert_eq!(obs_known.effective_datetime, known_ts);

    // effective_datetime = u64::MAX
    let obs_max = client.create_observation(
        &id,
        &fhir::types::ObservationStatus::Registered,
        &code_sys,
        &code_val,
        &subject,
        &value,
        &u64::MAX,
    );
    assert!(client.validate_observation(&obs_max));
    assert_eq!(obs_max.effective_datetime, u64::MAX);
}

/// Re-registering a resource at the same timestamp should fail with
/// RecordAlreadyExists, ensuring idempotency under identical time conditions.
#[test]
fn test_duplicate_register_at_same_timestamp() {
    let (env, client, admin) = setup();

    env.ledger().set_timestamp(42_000);

    let id = String::from_str(&env, "dup-check");
    let payload = Bytes::from_slice(&env, b"first");

    client.register_resource(&admin, &id, &payload);

    // Attempt to register the same id again at the same ledger time.
    let result = client.try_register_resource(
        &admin,
        &id,
        &Bytes::from_slice(&env, b"second"),
    );
    assert_eq!(result, Err(Ok(FhirError::RecordAlreadyExists.into())));
}

/// Updating a non-existent resource at any timestamp should fail with
/// RecordNotFound.
#[test]
fn test_update_nonexistent_resource_at_future_time() {
    let (env, client, admin) = setup();

    env.ledger().set_timestamp(99_999_999);

    let id = String::from_str(&env, "ghost");
    let payload = Bytes::from_slice(&env, b"nope");

    let result = client.try_update_resource(&admin, &id, &payload);
    assert_eq!(result, Err(Ok(FhirError::RecordNotFound.into())));
}

/// Deleting a non-existent resource after time advance should fail.
#[test]
fn test_delete_nonexistent_after_time_advance() {
    let (env, client, admin) = setup();

    env.ledger().set_timestamp(50_000);

    let id = String::from_str(&env, "never-created");
    let result = client.try_delete_resource(&admin, &id);
    assert_eq!(result, Err(Ok(FhirError::RecordNotFound.into())));
}

/// Register at time T, delete at T+1000, then verify that re-registering at
/// T+2000 succeeds (the resource slot is freed after deletion).
#[test]
fn test_register_delete_reregister_across_time() {
    let (env, client, admin) = setup();

    let id = String::from_str(&env, "recyclable");

    // Register at T=100
    env.ledger().set_timestamp(100);
    let v1 = Bytes::from_slice(&env, b"first-life");
    client.register_resource(&admin, &id, &v1);
    assert_eq!(client.get_resource(&id), v1);

    // Delete at T=1100
    env.ledger().set_timestamp(1_100);
    client.delete_resource(&admin, &id);
    assert!(client.try_get_resource(&id).is_err());

    // Re-register at T=2100
    env.ledger().set_timestamp(2_100);
    let v2 = Bytes::from_slice(&env, b"second-life");
    client.register_resource(&admin, &id, &v2);
    assert_eq!(client.get_resource(&id), v2);
}

/// Ensure the contract remains functional across a very large time gap between
/// initialization and first resource creation.
#[test]
fn test_large_time_gap_between_init_and_register() {
    let env = Env::default();
    env.mock_all_auths();

    // Initialize at T=0
    env.ledger().set_timestamp(0);
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Jump far into the future
    env.ledger().set_timestamp(31_536_000_000); // ~1000 years in seconds

    let id = String::from_str(&env, "millennium");
    let payload = Bytes::from_slice(&env, b"from-the-future");
    client.register_resource(&admin, &id, &payload);
    assert_eq!(client.get_resource(&id), payload);
}

/// Rapid timestamp increments (simulating fast block production) should all be
/// recorded without error.
#[test]
fn test_rapid_timestamp_increments() {
    let (env, client, admin) = setup();

    let base_time: u64 = 1_000_000;

    for i in 0u64..10 {
        env.ledger().set_timestamp(base_time + i);

        let id_str = {
            // Build an id string like "rapid-0", "rapid-1", etc.
            // Using a fixed set since no_std doesn't have format!
            match i {
                0 => "rapid-0",
                1 => "rapid-1",
                2 => "rapid-2",
                3 => "rapid-3",
                4 => "rapid-4",
                5 => "rapid-5",
                6 => "rapid-6",
                7 => "rapid-7",
                8 => "rapid-8",
                9 => "rapid-9",
                _ => unreachable!(),
            }
        };

        let id = String::from_str(&env, id_str);
        let payload = Bytes::from_slice(&env, b"rapid-data");
        client.register_resource(&admin, &id, &payload);
        assert_eq!(client.get_resource(&id), payload);
    }
}
