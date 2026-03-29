//! Unauthenticated Admin Function Call tests for the `fhir` crate.
//!
//! Ensures that every admin-gated entry point reverts with `Unauthorized`
//! when invoked by a random, unauthenticated address — mirroring the Soroban
//! `require_auth` / `Unauthorized` access-control pattern.

use soroban_sdk::{
    testutils::Address as _,
    Address, Env, String,
};

use fhir::contract::{FhirContract, FhirContractClient};
use fhir::errors::FhirError;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Boot a fresh Soroban `Env`, register the contract, initialise it under a
/// dedicated admin key, and hand back the environment, client, and a separate
/// *random* (non-admin) address that will be used as the unauthenticated caller.
fn setup() -> (Env, FhirContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, FhirContract);
    let client = FhirContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let stranger = Address::generate(&env);

    // Initialise the contract; admin is the privileged account.
    client.initialize(&admin);

    (env, client, admin, stranger)
}

// ── register_resource ─────────────────────────────────────────────────────────

#[test]
fn test_unauthenticated_cannot_register_resource() {
    let (env, client, _admin, stranger) = setup();

    // Disable all mock-auth so the stranger has no credentials.
    env.set_auths(&[]);

    let resource_id = String::from_str(&env, "res-001");
    let payload = String::from_str(&env, r#"{"resourceType":"Patient"}"#);

    let result = client.try_register_resource(&stranger, &resource_id, &payload);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::Unauthorized,
        "register_resource must revert with Unauthorized for unauthenticated caller"
    );
}

// ── update_resource ───────────────────────────────────────────────────────────

#[test]
fn test_unauthenticated_cannot_update_resource() {
    let (env, client, admin, stranger) = setup();

    // Register a resource as admin first.
    let resource_id = String::from_str(&env, "res-002");
    let payload_v1 = String::from_str(&env, r#"{"resourceType":"Observation","status":"preliminary"}"#);
    client.register_resource(&admin, &resource_id, &payload_v1);

    // Strip all auth, attempt update as stranger.
    env.set_auths(&[]);
    let payload_v2 = String::from_str(&env, r#"{"resourceType":"Observation","status":"final"}"#);

    let result = client.try_update_resource(&stranger, &resource_id, &payload_v2);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::Unauthorized,
        "update_resource must revert with Unauthorized for unauthenticated caller"
    );
}

// ── delete_resource ───────────────────────────────────────────────────────────

#[test]
fn test_unauthenticated_cannot_delete_resource() {
    let (env, client, admin, stranger) = setup();

    let resource_id = String::from_str(&env, "res-003");
    let payload = String::from_str(&env, r#"{"resourceType":"Condition"}"#);
    client.register_resource(&admin, &resource_id, &payload);

    env.set_auths(&[]);

    let result = client.try_delete_resource(&stranger, &resource_id);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::Unauthorized,
        "delete_resource must revert with Unauthorized for unauthenticated caller"
    );
}

// ── set_access_policy ─────────────────────────────────────────────────────────

#[test]
fn test_unauthenticated_cannot_set_access_policy() {
    let (env, client, _admin, stranger) = setup();

    env.set_auths(&[]);

    let policy_id = String::from_str(&env, "pol-001");
    let rules = String::from_str(&env, r#"{"allow":["read"]}"#);

    let result = client.try_set_access_policy(&stranger, &policy_id, &rules);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::Unauthorized,
        "set_access_policy must revert with Unauthorized for unauthenticated caller"
    );
}

// ── grant_role ────────────────────────────────────────────────────────────────

#[test]
fn test_unauthenticated_cannot_grant_role() {
    let (env, client, _admin, stranger) = setup();

    env.set_auths(&[]);

    let grantee = Address::generate(&env);
    let role = soroban_sdk::Symbol::new(&env, "Clinician");

    let result = client.try_grant_role(&stranger, &grantee, &role);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::Unauthorized,
        "grant_role must revert with Unauthorized for unauthenticated caller"
    );
}

// ── revoke_role ───────────────────────────────────────────────────────────────

#[test]
fn test_unauthenticated_cannot_revoke_role() {
    let (env, client, admin, stranger) = setup();

    let grantee = Address::generate(&env);
    let role = soroban_sdk::Symbol::new(&env, "Researcher");
    client.grant_role(&admin, &grantee, &role);

    env.set_auths(&[]);

    let result = client.try_revoke_role(&stranger, &grantee, &role);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::Unauthorized,
        "revoke_role must revert with Unauthorized for unauthenticated caller"
    );
}

// ── transfer_admin ────────────────────────────────────────────────────────────

#[test]
fn test_unauthenticated_cannot_transfer_admin() {
    let (env, client, _admin, stranger) = setup();

    env.set_auths(&[]);

    let new_admin = Address::generate(&env);
    let result = client.try_transfer_admin(&stranger, &new_admin);

    assert_eq!(
        result.err().unwrap().unwrap(),
        FhirError::Unauthorized,
        "transfer_admin must revert with Unauthorized for unauthenticated caller"
    );
}

// ── Confirm admin succeeds (positive baseline) ────────────────────────────────

#[test]
fn test_admin_can_register_resource() {
    let (env, client, admin, _stranger) = setup();

    env.mock_all_auths();

    let resource_id = String::from_str(&env, "res-ok");
    let payload = String::from_str(&env, r#"{"resourceType":"Patient"}"#);

    // Should not panic / return an error.
    client.register_resource(&admin, &resource_id, &payload);
}

#[test]
fn test_admin_can_grant_and_revoke_role() {
    let (env, client, admin, _stranger) = setup();

    env.mock_all_auths();

    let grantee = Address::generate(&env);
    let role = soroban_sdk::Symbol::new(&env, "Auditor");

    client.grant_role(&admin, &grantee, &role);
    client.revoke_role(&admin, &grantee, &role);
}