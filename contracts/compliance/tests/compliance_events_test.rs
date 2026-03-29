//! Event Emission Verification tests for the `compliance` crate.
//!
//! Simulates standard user flows and strictly verifies that all corresponding
//! state-changed events are emitted to the Soroban environment with the
//! correct topics and data payloads.

use soroban_sdk::{
    testutils::Events,
    vec, Env, IntoVal, Symbol,
};

use compliance::contract::{ComplianceContract, ComplianceContractClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Boot a fresh Soroban `Env`, register the contract, and return both the
/// environment and a ready-to-use client.
fn setup() -> (Env, ComplianceContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &contract_id);
    (env, client)
}

// ── Policy registration events ────────────────────────────────────────────────

#[test]
fn test_policy_registered_event_emitted() {
    let (env, client) = setup();

    let policy_id = Symbol::new(&env, "policy_001");
    let description = soroban_sdk::String::from_str(&env, "HIPAA Data Access Policy");

    client.register_policy(&policy_id, &description);

    let events = env.events().all();
    assert!(
        !events.is_empty(),
        "At least one event must be emitted on policy registration"
    );

    // The most recent event should carry the policy_registered topic.
    let (_, topics, data) = events.last().unwrap();
    let expected_topic = Symbol::new(&env, "policy_registered");
    assert_eq!(
        topics.get(0).unwrap(),
        expected_topic.into_val(&env),
        "First topic must be 'policy_registered'"
    );
    assert_eq!(
        data,
        policy_id.into_val(&env),
        "Event data must be the registered policy ID"
    );
}

#[test]
fn test_policy_updated_event_emitted() {
    let (env, client) = setup();

    let policy_id = Symbol::new(&env, "policy_002");
    let initial = soroban_sdk::String::from_str(&env, "Initial description");
    let updated = soroban_sdk::String::from_str(&env, "Updated description");

    client.register_policy(&policy_id, &initial);
    client.update_policy(&policy_id, &updated);

    let events = env.events().all();
    // Find the policy_updated event among all emitted events.
    let update_event = events.iter().find(|(_, topics, _)| {
        topics
            .get(0)
            .map(|t| t == Symbol::new(&env, "policy_updated").into_val(&env))
            .unwrap_or(false)
    });

    assert!(
        update_event.is_some(),
        "A 'policy_updated' event must be emitted after updating a policy"
    );

    let (_, _, data) = update_event.unwrap();
    assert_eq!(
        data,
        policy_id.into_val(&env),
        "Update event data must contain the policy ID"
    );
}

// ── Compliance check events ───────────────────────────────────────────────────

#[test]
fn test_compliance_check_passed_event_emitted() {
    let (env, client) = setup();

    let policy_id = Symbol::new(&env, "policy_read");
    let description = soroban_sdk::String::from_str(&env, "Read access policy");
    client.register_policy(&policy_id, &description);

    // Performing a check that should pass emits a check_passed event.
    let subject = soroban_sdk::Address::generate(&env);
    client.check_compliance(&subject, &policy_id);

    let events = env.events().all();
    let passed_event = events.iter().find(|(_, topics, _)| {
        topics
            .get(0)
            .map(|t| t == Symbol::new(&env, "check_passed").into_val(&env))
            .unwrap_or(false)
    });

    assert!(
        passed_event.is_some(),
        "A 'check_passed' event must be emitted when compliance check succeeds"
    );
}

#[test]
fn test_compliance_check_failed_event_emitted() {
    let (env, client) = setup();

    // Checking against a non-existent / restrictive policy triggers check_failed.
    let policy_id = Symbol::new(&env, "policy_deny");
    let subject = soroban_sdk::Address::generate(&env);

    // If the contract emits check_failed for an unregistered policy, capture it.
    let _ = client.try_check_compliance(&subject, &policy_id);

    let events = env.events().all();
    let failed_event = events.iter().find(|(_, topics, _)| {
        topics
            .get(0)
            .map(|t| t == Symbol::new(&env, "check_failed").into_val(&env))
            .unwrap_or(false)
    });

    assert!(
        failed_event.is_some(),
        "A 'check_failed' event must be emitted when compliance check fails"
    );
}

// ── Role assignment events ────────────────────────────────────────────────────

#[test]
fn test_role_granted_event_emitted() {
    let (env, client) = setup();

    let grantee = soroban_sdk::Address::generate(&env);
    let role = Symbol::new(&env, "Auditor");

    client.grant_role(&grantee, &role);

    let events = env.events().all();
    let grant_event = events.iter().find(|(_, topics, _)| {
        topics
            .get(0)
            .map(|t| t == Symbol::new(&env, "role_granted").into_val(&env))
            .unwrap_or(false)
    });

    assert!(
        grant_event.is_some(),
        "A 'role_granted' event must be emitted when a role is granted"
    );

    let (_, _, data) = grant_event.unwrap();
    assert_eq!(
        data,
        grantee.into_val(&env),
        "role_granted event data must be the grantee address"
    );
}

#[test]
fn test_role_revoked_event_emitted() {
    let (env, client) = setup();

    let grantee = soroban_sdk::Address::generate(&env);
    let role = Symbol::new(&env, "Clinician");

    client.grant_role(&grantee, &role);
    client.revoke_role(&grantee, &role);

    let events = env.events().all();
    let revoke_event = events.iter().find(|(_, topics, _)| {
        topics
            .get(0)
            .map(|t| t == Symbol::new(&env, "role_revoked").into_val(&env))
            .unwrap_or(false)
    });

    assert!(
        revoke_event.is_some(),
        "A 'role_revoked' event must be emitted when a role is revoked"
    );
}

// ── Event ordering guarantee ──────────────────────────────────────────────────

#[test]
fn test_event_ordering_registration_then_update() {
    let (env, client) = setup();

    let policy_id = Symbol::new(&env, "policy_ord");
    let v1 = soroban_sdk::String::from_str(&env, "v1");
    let v2 = soroban_sdk::String::from_str(&env, "v2");

    client.register_policy(&policy_id, &v1);
    client.update_policy(&policy_id, &v2);

    let events = env.events().all();
    let topics_list: Vec<Symbol> = events
        .iter()
        .filter_map(|(_, topics, _)| {
            topics.get(0).and_then(|t| t.try_into_val(&env).ok())
        })
        .collect();

    let reg_pos = topics_list
        .iter()
        .position(|t| *t == Symbol::new(&env, "policy_registered"));
    let upd_pos = topics_list
        .iter()
        .position(|t| *t == Symbol::new(&env, "policy_updated"));

    assert!(reg_pos.is_some(), "policy_registered event must exist");
    assert!(upd_pos.is_some(), "policy_updated event must exist");
    assert!(
        reg_pos.unwrap() < upd_pos.unwrap(),
        "policy_registered must be emitted before policy_updated"
    );
}

// ── No spurious events ────────────────────────────────────────────────────────

#[test]
fn test_no_events_emitted_on_read_only_query() {
    let (env, client) = setup();

    let policy_id = Symbol::new(&env, "policy_ro");
    let desc = soroban_sdk::String::from_str(&env, "Read-only policy");
    client.register_policy(&policy_id, &desc);

    // Drain registration event.
    let _ = env.events().all();

    // A pure read should not emit any new events.
    let _ = client.get_policy(&policy_id);

    let events_after = env.events().all();
    assert!(
        events_after.is_empty(),
        "Read-only queries must not emit any events"
    );
}