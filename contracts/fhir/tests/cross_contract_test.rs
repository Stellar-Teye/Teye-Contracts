//! cross_contract_test.rs
//!
//! Tests for Cross-Contract Calling Invariants in the `fhir` smart contract.
//!
//! Goal: Mock a secondary contract interface and verify that:
//!   1. The primary `fhir` contract correctly parses well-formed responses
//!      from external contracts.
//!   2. The primary contract handles external failures (panics / bad data)
//!      gracefully and propagates the correct error types.

#![cfg(test)]

extern crate std;

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, testutils::Address as _, vec, Address,
    Bytes, Env, String,
};

use fhir::{FhirContract, FhirContractClient, FhirError};

// ══════════════════════════════════════════════════════════════════════════════
// Mock "Registry" contract – simulates the external dependency that the FHIR
// contract calls when it needs to verify patient or record identifiers.
// ══════════════════════════════════════════════════════════════════════════════

/// Errors the mock registry can surface.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MockRegistryError {
    NotFound = 1,
    ServiceUnavailable = 2,
}

/// A minimal mock registry contract with two controllable behaviours:
///   - `get_record` → returns a valid record identifier
///   - `get_record_fail` → always panics / reverts (simulates external outage)
#[contract]
pub struct MockRegistryContract;

#[contractimpl]
impl MockRegistryContract {
    /// Happy-path: returns a deterministic record bytes payload.
    pub fn get_record(env: Env, record_id: u64) -> Bytes {
        // A trivially recognisable payload for assertion checks.
        let mut data_arr = [0u8; 1];
        data_arr[0] = record_id as u8;
        Bytes::from_array(&env, &data_arr)
    }

    /// Failure-path: always reverts, simulating a downstream outage.
    pub fn get_record_fail(_env: Env, _record_id: u64) -> Bytes {
        panic_with_error!(_env, MockRegistryError::ServiceUnavailable)
    }

    /// Returns an empty payload (edge case: registry exists but record is empty).
    pub fn get_record_empty(env: Env, _record_id: u64) -> Bytes {
        Bytes::new(&env)
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════════════════

struct TestFixture {
    env: Env,
    fhir_client: FhirContractClient<'static>,
    registry_id: Address,
    admin: Address,
}

fn setup() -> TestFixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    // Deploy the mock registry.
    let registry_id = env.register_contract(None, MockRegistryContract);

    // Deploy and initialise the FHIR contract, pointing it at the registry.
    let fhir_id = env.register_contract(None, FhirContract);
    let fhir_client = FhirContractClient::new(&env, &fhir_id);
    fhir_client.initialize(&admin, &registry_id);

    TestFixture {
        env,
        fhir_client,
        registry_id,
        admin,
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests – happy paths
// ══════════════════════════════════════════════════════════════════════════════

/// The FHIR contract correctly retrieves and stores a record that comes back
/// from the external registry in a well-formed state.
#[test]
fn test_cross_contract_fetch_record_success() {
    let TestFixture {
        env, fhir_client, ..
    } = setup();

    let record_id: u64 = 42;
    let result = fhir_client.try_fetch_and_store_record(&record_id);

    assert!(
        result.is_ok(),
        "Fetching a valid record from the registry must succeed"
    );

    // The record should now be readable through the FHIR contract itself.
    let stored = fhir_client.get_record(&record_id);
    assert!(
        !stored.is_empty(),
        "Stored record must be non-empty after a successful fetch"
    );
}

/// When the registry returns a record, the FHIR contract forwards the exact
/// same bytes without mutation (integrity check).
#[test]
fn test_cross_contract_record_integrity() {
    let TestFixture {
        env,
        fhir_client,
        registry_id,
        ..
    } = setup();

    let record_id: u64 = 7;

    // Pre-compute what the mock registry will return for this id.
    let registry_client = MockRegistryContractClient::new(&env, &registry_id);
    let expected_payload = registry_client.get_record(&record_id);

    fhir_client.fetch_and_store_record(&record_id);

    let stored = fhir_client.get_record(&record_id);
    assert_eq!(
        stored, expected_payload,
        "FHIR contract must store the exact bytes returned by the registry"
    );
}

/// Fetching multiple distinct records in sequence should all succeed and be
/// stored independently (no cross-contamination between record slots).
#[test]
fn test_cross_contract_multiple_records_independent() {
    let TestFixture {
        env,
        fhir_client,
        registry_id,
        ..
    } = setup();

    let ids: [u64; 3] = [1, 2, 3];
    for &id in &ids {
        fhir_client.fetch_and_store_record(&id);
    }

    let registry_client = MockRegistryContractClient::new(&env, &registry_id);
    for &id in &ids {
        let expected = registry_client.get_record(&id);
        let stored = fhir_client.get_record(&id);
        assert_eq!(
            stored, expected,
            "Record {} must match the registry payload",
            id
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests – external failure handling
// ══════════════════════════════════════════════════════════════════════════════

/// When the external registry reverts, the FHIR contract must propagate a
/// structured `ExternalCallFailed` error rather than panicking uncontrolled.
#[test]
fn test_cross_contract_external_failure_propagated() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    // Register a *failing* version of the registry.
    let failing_registry_id = env.register_contract(None, MockRegistryContract);

    let fhir_id = env.register_contract(None, FhirContract);
    let fhir_client = FhirContractClient::new(&env, &fhir_id);
    // Point the FHIR contract at the failing registry variant.
    fhir_client.initialize_with_failing_registry(&admin, &failing_registry_id);

    let result = fhir_client.try_fetch_and_store_record(&99_u64);
    assert_eq!(
        result,
        Err(Ok(FhirError::ExternalCallFailed.into())),
        "FHIR contract must surface ExternalCallFailed when the registry reverts"
    );
}

/// No partial state must be written if the external call fails mid-transaction.
#[test]
fn test_cross_contract_no_partial_state_on_failure() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let failing_registry_id = env.register_contract(None, MockRegistryContract);

    let fhir_id = env.register_contract(None, FhirContract);
    let fhir_client = FhirContractClient::new(&env, &fhir_id);
    fhir_client.initialize_with_failing_registry(&admin, &failing_registry_id);

    let record_id: u64 = 55;

    // Attempt (expected to fail).
    let _ = fhir_client.try_fetch_and_store_record(&record_id);

    // The record must NOT appear in storage.
    let result = fhir_client.try_get_record(&record_id);
    assert_eq!(
        result,
        Err(Ok(FhirError::RecordNotFound.into())),
        "No record must be persisted after a failed cross-contract call"
    );
}

/// When the registry returns an *empty* payload the FHIR contract must reject
/// it as invalid rather than silently storing an empty record.
#[test]
fn test_cross_contract_empty_payload_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    // Register the empty-payload variant of the registry.
    let empty_registry_id = env.register_contract(None, MockRegistryContract);

    let fhir_id = env.register_contract(None, FhirContract);
    let fhir_client = FhirContractClient::new(&env, &fhir_id);
    fhir_client.initialize_with_empty_registry(&admin, &empty_registry_id);

    let result = fhir_client.try_fetch_and_store_record(&1_u64);
    assert_eq!(
        result,
        Err(Ok(FhirError::InvalidRecordData.into())),
        "FHIR contract must reject empty payloads from the registry"
    );
}

/// The FHIR contract must reject the call entirely if the configured registry
/// address is the zero / unset sentinel (defensive guard at the call site).
#[test]
fn test_cross_contract_unset_registry_address_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy FHIR without calling initialize (registry address unset).
    let fhir_id = env.register_contract(None, FhirContract);
    let fhir_client = FhirContractClient::new(&env, &fhir_id);

    let result = fhir_client.try_fetch_and_store_record(&1_u64);
    assert!(
        result.is_err(),
        "Calling fetch_and_store_record before initialisation must revert"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests – re-entrant / repeated fetch
// ══════════════════════════════════════════════════════════════════════════════

/// Fetching the same record twice should be idempotent – the second call
/// either succeeds (overwrite) or surfaces a meaningful error, not a panic.
#[test]
fn test_cross_contract_duplicate_fetch_idempotent() {
    let TestFixture {
        env, fhir_client, ..
    } = setup();

    let record_id: u64 = 10;

    fhir_client.fetch_and_store_record(&record_id);
    let first_stored = fhir_client.get_record(&record_id);

    // Second fetch – must not panic.
    let result = fhir_client.try_fetch_and_store_record(&record_id);
    assert!(
        result.is_ok() || result == Err(Ok(FhirError::RecordAlreadyExists.into())),
        "Duplicate fetch must either succeed idempotently or return RecordAlreadyExists"
    );

    // If it succeeded, the stored data must still match the registry.
    if result.is_ok() {
        let second_stored = fhir_client.get_record(&record_id);
        assert_eq!(
            first_stored, second_stored,
            "Idempotent overwrite must produce identical data"
        );
    }
}
