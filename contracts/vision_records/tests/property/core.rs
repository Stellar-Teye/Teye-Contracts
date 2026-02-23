#![allow(clippy::unwrap_used, clippy::expect_used, clippy::arithmetic_side_effects)]
//! Property-based tests for the core contract functions.
//!
//! Invariants tested:
//! - Record IDs are always monotonically increasing (1, 2, 3…)
//! - `get_record` always returns exactly what was stored via `add_record`
//! - `get_patient_records` always includes every record ID for that patient
//! - `get_record_count` always equals the number of successful `add_record` calls

use proptest::prelude::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, String};
use vision_records::{RecordType, VisionRecordsContract, VisionRecordsContractClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, VisionRecordsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

/// Map a u8 to one of the six `RecordType` variants so proptest can generate them.
fn record_type_from_u8(n: u8) -> RecordType {
    match n % 6 {
        0 => RecordType::Examination,
        1 => RecordType::Prescription,
        2 => RecordType::Diagnosis,
        3 => RecordType::Treatment,
        4 => RecordType::Surgery,
        _ => RecordType::LabResult,
    }
}

// ── proptest! blocks ──────────────────────────────────────────────────────────

proptest! {
    /// For any number of records added (1–10), the returned IDs must be 1, 2, …, N.
    #[test]
    fn prop_record_id_monotonic(n_records in 1usize..=10usize) {
        let (env, client, _admin) = setup();
        let patient = Address::generate(&env);
        let provider = Address::generate(&env);

        for expected_id in 1..=(n_records as u64) {
            let hash = String::from_str(
                &env,
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            );
            let id = client.add_record(
                &_admin,
                &patient,
                &provider,
                &RecordType::Examination,
                &hash,
            );
            prop_assert_eq!(id, expected_id);
        }
    }

    /// `get_record` must return exactly the patient, provider, and record_type that was stored.
    #[test]
    fn prop_get_record_matches_store(record_type_seed in 0u8..=255u8) {
        let (env, client, _admin) = setup();

        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        let rtype = record_type_from_u8(record_type_seed);
        let hash = String::from_str(
            &env,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",        );

        let id = client.add_record(&_admin, &patient, &provider, &rtype, &hash);
        let record = client.get_record(&id);

        prop_assert_eq!(record.patient, patient);
        prop_assert_eq!(record.provider, provider);
        prop_assert_eq!(record.record_type, rtype);
        prop_assert_eq!(record.data_hash, hash);
        prop_assert_eq!(record.id, id);
    }

    /// After `add_record`, `get_patient_records` must always contain the new record ID.
    #[test]
    fn prop_patient_records_always_includes_new(n_records in 1usize..=8usize) {
        let (env, client, _admin) = setup();
        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        let hash = String::from_str(
            &env,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",        );

        let mut added_ids: Vec<u64> = Vec::new();

        for _ in 0..n_records {
            let id = client.add_record(
                &_admin,
                &patient,
                &provider,
                &RecordType::Prescription,
                &hash,
            );
            added_ids.push(id);

            // Every time we add a record, check that the full list contains all added IDs so far.
            let stored = client.get_patient_records(&patient);
            for prev_id in &added_ids {
                prop_assert!(
                    stored.contains(prev_id),
                    "Record ID {} missing from patient records list",
                    prev_id
                );
            }
        }
    }

    /// `get_record_count` must always equal the number of successfully added records.
    #[test]
    fn prop_record_count_increments(n_records in 0usize..=12usize) {
        let (env, client, _admin) = setup();
        let patient = Address::generate(&env);
        let provider = Address::generate(&env);

        prop_assert_eq!(client.get_record_count(), 0u64);

        for i in 0..n_records {
            let hash = String::from_str(
                &env,
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",            );
            client.add_record(&_admin, &patient, &provider, &RecordType::Diagnosis, &hash);
            prop_assert_eq!(client.get_record_count(), i.saturating_add(1) as u64);
        }
    }

    /// Records for different patients must never appear in each other's record lists.
    #[test]
    fn prop_patient_records_isolated(n_a in 1usize..=5usize, n_b in 1usize..=5usize) {
        let (env, client, _admin) = setup();
        let patient_a = Address::generate(&env);
        let patient_b = Address::generate(&env);
        let provider = Address::generate(&env);
        let hash = String::from_str(
            &env,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",        );

        let mut ids_a: Vec<u64> = Vec::new();
        let mut ids_b: Vec<u64> = Vec::new();

        for _ in 0..n_a {
            let id = client.add_record(&_admin, &patient_a, &provider, &RecordType::Examination, &hash);
            ids_a.push(id);
        }
        for _ in 0..n_b {
            let id = client.add_record(&_admin, &patient_b, &provider, &RecordType::Examination, &hash);
            ids_b.push(id);
        }

        let records_a = client.get_patient_records(&patient_a);
        let records_b = client.get_patient_records(&patient_b);

        // No A id should appear in B's list
        for id in &ids_a {
            prop_assert!(
                !records_b.contains(id),
                "ID {} from patient_a found in patient_b's records",
                id
            );
        }
        // No B id should appear in A's list
        for id in &ids_b {
            prop_assert!(
                !records_a.contains(id),
                "ID {} from patient_b found in patient_a's records",
                id
            );
        }
    }
}
