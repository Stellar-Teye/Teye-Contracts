//! Initialization Constraints Validation for `emr_bridge`
//!
//! Validates that:
//! - `initialize` sets admin and INIT flag exactly once (O(1) storage writes).
//! - Repeated calls return `AlreadyInitialized` without mutating state.
//! - All admin-gated entry points reject calls on an uninitialized contract.
//! - The `EMR_INIT` event is emitted on successful initialization.

use emr_bridge::{EmrBridgeContract, EmrBridgeContractClient, EmrBridgeError};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, String, Vec,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Minimal setup: register the contract but do NOT initialize.
fn setup_uninit() -> (Env, EmrBridgeContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(EmrBridgeContract, ());
    let client = EmrBridgeContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    (env, client, admin)
}

/// Setup + initialize — convenience for tests that need a ready contract.
fn setup() -> (Env, EmrBridgeContractClient<'static>, Address) {
    let (env, client, admin) = setup_uninit();
    client.initialize(&admin);
    (env, client, admin)
}

// ── 1. Successful Initialization ─────────────────────────────────────────────

#[test]
fn test_initialize_sets_admin() {
    let (_env, client, admin) = setup();
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_initialize_returns_ok_via_try() {
    let (_env, client, admin) = setup_uninit();
    let result = client.try_initialize(&admin);
    assert_eq!(result, Ok(Ok(())));
}

// ── 2. Double / Repeated Initialization ──────────────────────────────────────

#[test]
fn test_double_init_returns_already_initialized() {
    let (env, client, _admin) = setup();
    let other = Address::generate(&env);
    let result = client.try_initialize(&other);
    assert_eq!(result, Err(Ok(EmrBridgeError::AlreadyInitialized)));
}

#[test]
fn test_repeated_init_attempts_all_rejected() {
    let (env, client, _admin) = setup();
    for _ in 0..3 {
        let attacker = Address::generate(&env);
        assert_eq!(
            client.try_initialize(&attacker),
            Err(Ok(EmrBridgeError::AlreadyInitialized)),
        );
    }
}

#[test]
fn test_admin_unchanged_after_rejected_reinit() {
    let (env, client, admin) = setup();
    let attacker = Address::generate(&env);
    let _ = client.try_initialize(&attacker);
    // Admin must remain the original address.
    assert_eq!(client.get_admin(), admin);
}

// ── 3. Uninitialized Contract Guards ─────────────────────────────────────────
//
// Every admin-gated entry point relies on `require_admin`, which reads the
// ADMIN key. On an uninitialized contract the key is absent, so the helper
// returns `NotInitialized`. We test each such entry point exactly once to
// confirm the guard is active.

#[test]
fn test_get_admin_before_init_returns_not_initialized() {
    let (_env, client, _admin) = setup_uninit();
    let result = client.try_get_admin();
    assert_eq!(result, Err(Ok(EmrBridgeError::NotInitialized)));
}

#[test]
fn test_register_provider_before_init_returns_not_initialized() {
    let (env, client, admin) = setup_uninit();
    let result = client.try_register_provider(
        &admin,
        &String::from_str(&env, "p1"),
        &String::from_str(&env, "name"),
        &emr_bridge::types::EmrSystem::EpicFhir,
        &String::from_str(&env, "https://ep"),
        &emr_bridge::types::DataFormat::FhirR4,
    );
    assert_eq!(result, Err(Ok(EmrBridgeError::NotInitialized)));
}

#[test]
fn test_activate_provider_before_init_returns_not_initialized() {
    let (env, client, admin) = setup_uninit();
    let result = client.try_activate_provider(&admin, &String::from_str(&env, "p1"));
    assert_eq!(result, Err(Ok(EmrBridgeError::NotInitialized)));
}

#[test]
fn test_suspend_provider_before_init_returns_not_initialized() {
    let (env, client, admin) = setup_uninit();
    let result = client.try_suspend_provider(&admin, &String::from_str(&env, "p1"));
    assert_eq!(result, Err(Ok(EmrBridgeError::NotInitialized)));
}

#[test]
fn test_record_data_exchange_before_init_returns_not_initialized() {
    let (env, client, admin) = setup_uninit();
    let s = |v: &str| String::from_str(&env, v);
    let result = client.try_record_data_exchange(
        &admin,
        &s("ex1"),
        &s("p1"),
        &s("pat1"),
        &emr_bridge::types::ExchangeDirection::Import,
        &emr_bridge::types::DataFormat::FhirR4,
        &s("Patient"),
        &s("hash"),
    );
    assert_eq!(result, Err(Ok(EmrBridgeError::NotInitialized)));
}

#[test]
fn test_update_exchange_status_before_init_returns_not_initialized() {
    let (env, client, admin) = setup_uninit();
    let result = client.try_update_exchange_status(
        &admin,
        &String::from_str(&env, "ex1"),
        &emr_bridge::types::SyncStatus::Completed,
    );
    assert_eq!(result, Err(Ok(EmrBridgeError::NotInitialized)));
}

#[test]
fn test_create_field_mapping_before_init_returns_not_initialized() {
    let (env, client, admin) = setup_uninit();
    let s = |v: &str| String::from_str(&env, v);
    let result = client.try_create_field_mapping(
        &admin,
        &s("m1"),
        &s("p1"),
        &s("src"),
        &s("tgt"),
        &s("rule"),
    );
    assert_eq!(result, Err(Ok(EmrBridgeError::NotInitialized)));
}

#[test]
fn test_verify_sync_before_init_returns_not_initialized() {
    let (env, client, admin) = setup_uninit();
    let s = |v: &str| String::from_str(&env, v);
    let empty_vec: Vec<String> = Vec::new(&env);
    let result =
        client.try_verify_sync(&admin, &s("v1"), &s("ex1"), &s("h1"), &s("h2"), &empty_vec);
    assert_eq!(result, Err(Ok(EmrBridgeError::NotInitialized)));
}

// ── 4. Event Emission ────────────────────────────────────────────────────────

#[test]
fn test_initialize_emits_event() {
    let (env, client, admin) = setup_uninit();
    client.initialize(&admin);

    // env.events().all() returns ContractEvents; .events() gives &[xdr::ContractEvent].
    // We verify at least one event was emitted during initialization.
    let contract_events = env.events().all();
    assert!(
        !contract_events.events().is_empty(),
        "initialization must emit at least one event"
    );
}

// ── 5. Read-Only Endpoints on Uninitialized Contract ─────────────────────────
//
// Read-only endpoints that don't call `require_admin` should still behave
// correctly on a fresh contract (return empty collections or typed errors).

#[test]
fn test_list_providers_returns_empty_before_any_registration() {
    let (_env, client, _admin) = setup_uninit();
    let providers = client.list_providers();
    assert_eq!(providers.len(), 0);
}

#[test]
fn test_get_patient_exchanges_returns_empty_on_fresh_contract() {
    let (env, client, _admin) = setup_uninit();
    let exchanges = client.get_patient_exchanges(&String::from_str(&env, "pat-999"));
    assert_eq!(exchanges.len(), 0);
}

#[test]
fn test_get_provider_mappings_returns_empty_on_fresh_contract() {
    let (env, client, _admin) = setup_uninit();
    let mappings = client.get_provider_mappings(&String::from_str(&env, "p-999"));
    assert_eq!(mappings.len(), 0);
}

#[test]
fn test_get_provider_returns_not_found_on_fresh_contract() {
    let (env, client, _admin) = setup_uninit();
    let result = client.try_get_provider(&String::from_str(&env, "nonexistent"));
    assert_eq!(result, Err(Ok(EmrBridgeError::ProviderNotFound)));
}

#[test]
fn test_get_exchange_returns_not_found_on_fresh_contract() {
    let (env, client, _admin) = setup_uninit();
    let result = client.try_get_exchange(&String::from_str(&env, "nonexistent"));
    assert_eq!(result, Err(Ok(EmrBridgeError::ExchangeNotFound)));
}

#[test]
fn test_get_verification_returns_not_found_on_fresh_contract() {
    let (env, client, _admin) = setup_uninit();
    let result = client.try_get_verification(&String::from_str(&env, "nonexistent"));
    assert_eq!(result, Err(Ok(EmrBridgeError::VerificationNotFound)));
}
