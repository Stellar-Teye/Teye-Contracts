//! Zero-Value Parameter Passing Edge Case tests for the `fhir` crate.
//!
//! Sends inputs of zero, empty strings, or blank/zero addresses to every
//! critical state-modifying function to verify consistent validation and
//! correct revert behaviour — preventing silent acceptance of degenerate data.

use soroban_sdk::{
    testutils::Address as _,
    Address, Env, String,
};

use fhir::contract::{FhirContract, FhirContractClient};
use fhir::errors::FhirError;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, FhirContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, FhirContract);
    let client = FhirContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

// ── Empty resource ID ─────────────────────────────────────────────────────────

#[test]
fn test_register_resource_with_empty_id_fails() {
    let (env, client, admin) = setup();

    let empty_id = String::from_str(&env, "");
    let payload = String::from_str(&env, r#"{"resourceType":"Patient"}"#);

    let result = client.try_register_resource(&admin, &empty_id, &payload);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::InvalidInput,
        "register_resource must reject an empty resource ID"
    );
}

// ── Empty payload ─────────────────────────────────────────────────────────────

#[test]
fn test_register_resource_with_empty_payload_fails() {
    let (env, client, admin) = setup();

    let resource_id = String::from_str(&env, "res-empty-payload");
    let empty_payload = String::from_str(&env, "");

    let result = client.try_register_resource(&admin, &resource_id, &empty_payload);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::InvalidInput,
        "register_resource must reject an empty payload"
    );
}

// ── Both resource ID and payload empty ────────────────────────────────────────

#[test]
fn test_register_resource_with_all_empty_inputs_fails() {
    let (env, client, admin) = setup();

    let empty = String::from_str(&env, "");
    let result = client.try_register_resource(&admin, &empty, &empty);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::InvalidInput,
        "register_resource must reject when both ID and payload are empty"
    );
}

// ── Update with empty payload ─────────────────────────────────────────────────

#[test]
fn test_update_resource_with_empty_payload_fails() {
    let (env, client, admin) = setup();

    let resource_id = String::from_str(&env, "res-upd-empty");
    let payload = String::from_str(&env, r#"{"resourceType":"Observation"}"#);
    client.register_resource(&admin, &resource_id, &payload);

    let empty = String::from_str(&env, "");
    let result = client.try_update_resource(&admin, &resource_id, &empty);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::InvalidInput,
        "update_resource must reject an empty updated payload"
    );
}

// ── Update non-existent resource (zero / unknown ID) ─────────────────────────

#[test]
fn test_update_nonexistent_resource_fails() {
    let (env, client, admin) = setup();

    let ghost_id = String::from_str(&env, "does-not-exist");
    let payload = String::from_str(&env, r#"{"resourceType":"Medication"}"#);

    let result = client.try_update_resource(&admin, &ghost_id, &payload);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::NotFound,
        "update_resource must return NotFound for an unknown resource ID"
    );
}

// ── Delete non-existent resource ──────────────────────────────────────────────

#[test]
fn test_delete_nonexistent_resource_fails() {
    let (env, client, admin) = setup();

    let ghost_id = String::from_str(&env, "ghost-res");
    let result = client.try_delete_resource(&admin, &ghost_id);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::NotFound,
        "delete_resource must return NotFound for a non-existent resource"
    );
}

// ── Set access policy with empty policy ID ────────────────────────────────────

#[test]
fn test_set_access_policy_with_empty_id_fails() {
    let (env, client, admin) = setup();

    let empty_id = String::from_str(&env, "");
    let rules = String::from_str(&env, r#"{"allow":["read"]}"#);

    let result = client.try_set_access_policy(&admin, &empty_id, &rules);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::InvalidInput,
        "set_access_policy must reject an empty policy ID"
    );
}

// ── Set access policy with empty rules string ─────────────────────────────────

#[test]
fn test_set_access_policy_with_empty_rules_fails() {
    let (env, client, admin) = setup();

    let policy_id = String::from_str(&env, "pol-empty-rules");
    let empty_rules = String::from_str(&env, "");

    let result = client.try_set_access_policy(&admin, &policy_id, &empty_rules);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::InvalidInput,
        "set_access_policy must reject an empty rules payload"
    );
}

// ── Grant role with empty role symbol ─────────────────────────────────────────
// Note: Soroban `Symbol` must be non-empty at construction time; this test
// verifies that the contract's own guard rejects an unrecognised/blank role
// rather than silently storing garbage.

#[test]
fn test_grant_role_with_unknown_role_fails() {
    let (env, client, admin) = setup();

    let grantee = Address::generate(&env);
    // "Unknown" is not a recognised role enum variant in the contract.
    let bad_role = soroban_sdk::Symbol::new(&env, "Unknown");

    let result = client.try_grant_role(&admin, &grantee, &bad_role);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::InvalidInput,
        "grant_role must reject an unrecognised role symbol"
    );
}

// ── Initialize with zero / default address ────────────────────────────────────

#[test]
fn test_initialize_with_zero_address_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, FhirContract);
    let client = FhirContractClient::new(&env, &contract_id);

    // Address::zero() represents the all-zero / burn address.
    let zero_addr = Address::zero(&env);
    let result = client.try_initialize(&zero_addr);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::InvalidInput,
        "initialize must reject the zero address as admin"
    );
}

// ── Double-initialize guard ───────────────────────────────────────────────────

#[test]
fn test_double_initialize_fails() {
    let (env, client, _admin) = setup();

    let new_admin = Address::generate(&env);
    let result = client.try_initialize(&new_admin);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::AlreadyInitialized,
        "initialize must revert if the contract has already been initialised"
    );
}

// ── Whitespace-only strings (near-empty) ─────────────────────────────────────

#[test]
fn test_register_resource_with_whitespace_id_fails() {
    let (env, client, admin) = setup();

    let blank_id = String::from_str(&env, "   ");
    let payload = String::from_str(&env, r#"{"resourceType":"Patient"}"#);

    let result = client.try_register_resource(&admin, &blank_id, &payload);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::InvalidInput,
        "register_resource must reject a whitespace-only resource ID"
    );
}