//! Event Emission Verification tests for the `fhir` crate.
//!
//! Simulates standard user flows and strictly verifies that all corresponding
//! state-changed events are emitted to the Soroban environment with the
//! correct topics and data payloads.

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol,
};

use fhir::contract::{FhirContract, FhirContractClient};

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

/// Returns the first event whose first topic matches `topic_name`.
fn find_event<'a>(
    env: &'a Env,
    events: &'a soroban_sdk::Vec<(soroban_sdk::Address, soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val)>,
    topic_name: &str,
) -> Option<(soroban_sdk::Address, soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val)> {
    let needle = Symbol::new(env, topic_name).into_val(env);
    events.iter().find(|(_, topics, _)| {
        topics.get(0).map(|t| t == needle).unwrap_or(false)
    })
}

// ── Resource lifecycle events ─────────────────────────────────────────────────

#[test]
fn test_resource_registered_event_emitted() {
    let (env, client, admin) = setup();

    let resource_id = String::from_str(&env, "res-evt-001");
    let payload = String::from_str(&env, r#"{"resourceType":"Patient","id":"p1"}"#);

    client.register_resource(&admin, &resource_id, &payload);

    let events = env.events().all();
    let evt = find_event(&env, &events, "resource_registered");

    assert!(
        evt.is_some(),
        "A 'resource_registered' event must be emitted on registration"
    );

    let (_, _, data) = evt.unwrap();
    assert_eq!(
        data,
        resource_id.into_val(&env),
        "resource_registered event data must be the resource ID"
    );
}

#[test]
fn test_resource_updated_event_emitted() {
    let (env, client, admin) = setup();

    let resource_id = String::from_str(&env, "res-evt-002");
    let v1 = String::from_str(&env, r#"{"resourceType":"Observation","status":"preliminary"}"#);
    let v2 = String::from_str(&env, r#"{"resourceType":"Observation","status":"final"}"#);

    client.register_resource(&admin, &resource_id, &v1);
    client.update_resource(&admin, &resource_id, &v2);

    let events = env.events().all();
    let evt = find_event(&env, &events, "resource_updated");

    assert!(
        evt.is_some(),
        "A 'resource_updated' event must be emitted after updating a resource"
    );

    let (_, _, data) = evt.unwrap();
    assert_eq!(
        data,
        resource_id.into_val(&env),
        "resource_updated event data must carry the resource ID"
    );
}

#[test]
fn test_resource_deleted_event_emitted() {
    let (env, client, admin) = setup();

    let resource_id = String::from_str(&env, "res-evt-003");
    let payload = String::from_str(&env, r#"{"resourceType":"Condition"}"#);

    client.register_resource(&admin, &resource_id, &payload);
    client.delete_resource(&admin, &resource_id);

    let events = env.events().all();
    let evt = find_event(&env, &events, "resource_deleted");

    assert!(
        evt.is_some(),
        "A 'resource_deleted' event must be emitted after deleting a resource"
    );

    let (_, _, data) = evt.unwrap();
    assert_eq!(
        data,
        resource_id.into_val(&env),
        "resource_deleted event data must carry the deleted resource ID"
    );
}

// ── Access policy events ──────────────────────────────────────────────────────

#[test]
fn test_access_policy_set_event_emitted() {
    let (env, client, admin) = setup();

    let policy_id = String::from_str(&env, "pol-evt-001");
    let rules = String::from_str(&env, r#"{"allow":["read","write"]}"#);

    client.set_access_policy(&admin, &policy_id, &rules);

    let events = env.events().all();
    let evt = find_event(&env, &events, "policy_set");

    assert!(
        evt.is_some(),
        "A 'policy_set' event must be emitted when an access policy is set"
    );

    let (_, _, data) = evt.unwrap();
    assert_eq!(
        data,
        policy_id.into_val(&env),
        "policy_set event data must contain the policy ID"
    );
}

// ── Role management events ────────────────────────────────────────────────────

#[test]
fn test_role_granted_event_emitted() {
    let (env, client, admin) = setup();

    let grantee = Address::generate(&env);
    let role = Symbol::new(&env, "Clinician");

    client.grant_role(&admin, &grantee, &role);

    let events = env.events().all();
    let evt = find_event(&env, &events, "role_granted");

    assert!(
        evt.is_some(),
        "A 'role_granted' event must be emitted when a role is granted"
    );

    let (_, topics, data) = evt.unwrap();

    // Second topic should be the role symbol.
    assert_eq!(
        topics.get(1).unwrap(),
        role.into_val(&env),
        "Second topic of role_granted must be the role symbol"
    );

    assert_eq!(
        data,
        grantee.into_val(&env),
        "role_granted event data must be the grantee address"
    );
}

#[test]
fn test_role_revoked_event_emitted() {
    let (env, client, admin) = setup();

    let grantee = Address::generate(&env);
    let role = Symbol::new(&env, "Researcher");

    client.grant_role(&admin, &grantee, &role);
    client.revoke_role(&admin, &grantee, &role);

    let events = env.events().all();
    let evt = find_event(&env, &events, "role_revoked");

    assert!(
        evt.is_some(),
        "A 'role_revoked' event must be emitted when a role is revoked"
    );

    let (_, _, data) = evt.unwrap();
    assert_eq!(
        data,
        grantee.into_val(&env),
        "role_revoked event data must be the address whose role was revoked"
    );
}

// ── Admin transfer event ──────────────────────────────────────────────────────

#[test]
fn test_admin_transferred_event_emitted() {
    let (env, client, admin) = setup();

    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);

    let events = env.events().all();
    let evt = find_event(&env, &events, "admin_transferred");

    assert!(
        evt.is_some(),
        "An 'admin_transferred' event must be emitted on admin transfer"
    );

    let (_, _, data) = evt.unwrap();
    assert_eq!(
        data,
        new_admin.into_val(&env),
        "admin_transferred event data must be the new admin address"
    );
}

// ── Event ordering guarantee ──────────────────────────────────────────────────

#[test]
fn test_register_then_update_event_ordering() {
    let (env, client, admin) = setup();

    let resource_id = String::from_str(&env, "res-order");
    let v1 = String::from_str(&env, r#"{"resourceType":"Medication","status":"active"}"#);
    let v2 = String::from_str(&env, r#"{"resourceType":"Medication","status":"inactive"}"#);

    client.register_resource(&admin, &resource_id, &v1);
    client.update_resource(&admin, &resource_id, &v2);

    let events = env.events().all();

    let reg_pos = events.iter().position(|(_, topics, _)| {
        topics
            .get(0)
            .map(|t| t == Symbol::new(&env, "resource_registered").into_val(&env))
            .unwrap_or(false)
    });

    let upd_pos = events.iter().position(|(_, topics, _)| {
        topics
            .get(0)
            .map(|t| t == Symbol::new(&env, "resource_updated").into_val(&env))
            .unwrap_or(false)
    });

    assert!(reg_pos.is_some(), "resource_registered event must exist");
    assert!(upd_pos.is_some(), "resource_updated event must exist");
    assert!(
        reg_pos.unwrap() < upd_pos.unwrap(),
        "resource_registered must be emitted before resource_updated"
    );
}

// ── No spurious events on read ────────────────────────────────────────────────

#[test]
fn test_no_events_on_read_only_call() {
    let (env, client, admin) = setup();

    let resource_id = String::from_str(&env, "res-read-only");
    let payload = String::from_str(&env, r#"{"resourceType":"Patient"}"#);
    client.register_resource(&admin, &resource_id, &payload);

    // Consume all setup events.
    let _ = env.events().all();

    // Pure read — must not emit anything.
    let _ = client.get_resource(&resource_id);

    let after = env.events().all();
    assert!(
        after.is_empty(),
        "Read-only calls must not emit any events"
    );
}

// ── Multiple resources: each emits its own event ──────────────────────────────

#[test]
fn test_multiple_registrations_each_emit_event() {
    let (env, client, admin) = setup();

    let ids = ["res-m1", "res-m2", "res-m3"];
    for id in &ids {
        let resource_id = String::from_str(&env, id);
        let payload = String::from_str(&env, r#"{"resourceType":"Device"}"#);
        client.register_resource(&admin, &resource_id, &payload);
    }

    let events = env.events().all();
    let reg_count = events.iter().filter(|(_, topics, _)| {
        topics
            .get(0)
            .map(|t| t == Symbol::new(&env, "resource_registered").into_val(&env))
            .unwrap_or(false)
    }).count();

    assert_eq!(
        reg_count,
        ids.len(),
        "Each registered resource must emit exactly one 'resource_registered' event"
    );
}