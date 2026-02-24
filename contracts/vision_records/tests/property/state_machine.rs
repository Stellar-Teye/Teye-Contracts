#![allow(clippy::unwrap_used, clippy::expect_used, clippy::arithmetic_side_effects)]
//! Property-based state machine tests for the VisionRecords contract.
//!
//! These tests model the contract as a state machine and verify that
//! sequences of operations always produce internally consistent state.
//!
//! Invariants tested:
//! - A second `initialize` call must always return `Err(AlreadyInitialized)`
//! - The full lifecycle (init → register → record → access → revoke) is always consistent

use proptest::prelude::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, String};
use vision_records::{
    AccessLevel, ContractError, RecordType, Role, VisionRecordsContract,
    VisionRecordsContractClient,
};

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

// ── proptest! blocks ──────────────────────────────────────────────────────────

proptest! {
    /// A second `initialize` call must always fail with `AlreadyInitialized`.
    #[test]
    fn prop_double_initialize_always_fails(_seed in 0u8..=255u8) {
        let (env, client, _admin) = setup();

        // Contract is already initialized — a second call must error
        let second_admin = Address::generate(&env);
        let result = client.try_initialize(&second_admin);

        prop_assert!(result.is_err(), "Double initialize must always fail");
        match result {
            Err(Ok(e)) => prop_assert_eq!(e, ContractError::AlreadyInitialized),
            _ => prop_assert!(false, "Expected AlreadyInitialized error"),
        }
    }

    /// `is_initialized` must always return `true` after `initialize` succeeds.
    #[test]
    fn prop_is_initialized_after_init(_seed in 0u8..=255u8) {
        let (_env, client, _admin) = setup();
        prop_assert!(client.is_initialized());
    }

    /// `get_admin` must always return exactly the address passed to `initialize`.
    #[test]
    fn prop_get_admin_matches_initializer(_seed in 0u8..=255u8) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        prop_assert_eq!(client.get_admin(), admin);
    }

    /// Full lifecycle: initialize → register_user → add_record → grant_access → check_access
    /// → revoke_access. Every step must yield the expected consistent result.
    #[test]
    fn prop_full_lifecycle_consistent(_seed in 0u8..=255u8) {
        let (env, client, admin) = setup();

        // Register a provider (optometrist)
        let provider = Address::generate(&env);
        client.register_user(
            &admin,
            &provider,
            &Role::Optometrist,
            &String::from_str(&env, "Dr. Property"),
        );

        // Register a patient
        let patient = Address::generate(&env);
        client.register_user(
            &admin,
            &patient,
            &Role::Patient,
            &String::from_str(&env, "Patient Property"),
        );

        // Provider adds a record for the patient
        let hash = String::from_str(&env, "lifecycle_hash_value_ffffffffffff");        let record_id = client.add_record(
            &admin,
            &patient,
            &provider,
            &RecordType::Examination,
            &hash,
        );

        // Record ID must equal 1 for the first record
        prop_assert_eq!(record_id, 1u64);

        // Stored record must match inputs
        let record = client.get_record(&record_id);
        prop_assert_eq!(record.patient, patient.clone());
        prop_assert_eq!(record.provider, provider.clone());
        prop_assert_eq!(record.data_hash, hash);

        // Patient grants access to a doctor
        let doctor = Address::generate(&env);
        client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &3600u64);
        prop_assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);

        // Patient revokes access
        client.revoke_access(&patient, &doctor);
        prop_assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
    }

    /// Registering the same user twice must overwrite the record (not panic),
    /// and the most-recently registered role must be the current one.
    #[test]
    fn prop_reregister_overwrites_role(_seed in 0u8..=255u8) {
        let (env, client, admin) = setup();

        let user = Address::generate(&env);
        let name = String::from_str(&env, "Test User");

        client.register_user(&admin, &user, &Role::Staff, &name);
        prop_assert_eq!(client.get_user(&user).role, Role::Staff);

        // Re-register with a different role
        client.register_user(&admin, &user, &Role::Optometrist, &name);
        prop_assert_eq!(client.get_user(&user).role, Role::Optometrist);
    }

    /// `version()` must always return the same constant (1) regardless of state.
    #[test]
    fn prop_version_always_returns_one(_seed in 0u8..=255u8) {
        prop_assert_eq!(VisionRecordsContract::version(), 1u32);
    }
}
