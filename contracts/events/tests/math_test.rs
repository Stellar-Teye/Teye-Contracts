//! Issue 1: Data Overflow and Underflow Edge Cases
//!
//! Passes boundary values (u64::MAX, u32::MAX, etc.) into mathematical
//! computations to confirm the contract does not panic or wrap unsafely.

#![allow(clippy::unwrap_used)]

extern crate std;

use events::{EventError, EventStreamContract, EventStreamContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup() -> (Env, EventStreamContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventStreamContract, ());
    let client = EventStreamContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

fn register_schema(env: &Env, client: &EventStreamContractClient, admin: &Address, topic: &str) {
    let topic_str = String::from_str(env, topic);
    let hash = String::from_str(env, "sha256:abc123");
    client.register_schema(admin, &topic_str, &1, &hash);
}

// ── Event ID counter boundary ────────────────────────────────────────────────

/// Publishing many events should not overflow the u64 event counter.
/// We publish a large batch and verify the counter increments correctly.
#[test]
fn test_event_counter_increments_without_overflow() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create");

    let topic = String::from_str(&env, "records.vision.create");
    let payload = String::from_str(&env, "payload");

    // Publish 100 events — verifies no arithmetic panic in next_event_id
    for _ in 0..100 {
        client.publish_event(&admin, &topic, &1, &payload);
    }

    assert_eq!(client.get_event_count(), 100);
    assert_eq!(client.get_lamport_clock(), 100);
}

/// Lamport clock must increment strictly — verify it stays consistent
/// after a large number of ticks without wrapping.
#[test]
fn test_lamport_clock_strict_monotonic_large_batch() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create");

    let topic = String::from_str(&env, "records.vision.create");
    let payload = String::from_str(&env, "p");

    for _ in 0..50 {
        client.publish_event(&admin, &topic, &1, &payload);
    }

    let clock = client.get_lamport_clock();
    assert_eq!(clock, 50, "Lamport clock must equal number of published events");
}

// ── Schema version boundary ──────────────────────────────────────────────────

/// u32::MAX is the largest possible schema version — the contract must
/// accept it without panicking.
#[test]
fn test_schema_version_u32_max() {
    let (env, client, admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");
    let hash = String::from_str(&env, "sha256:maxversion");

    // Register a normal version first so u32::MAX is strictly greater
    client.register_schema(&admin, &topic, &1, &hash);

    let result = client.try_register_schema(&admin, &topic, &u32::MAX, &hash);
    // Should succeed — u32::MAX > 1
    assert!(result.is_ok(), "u32::MAX schema version should be accepted");

    let latest = client.get_latest_schema_version(&topic);
    assert_eq!(latest, u32::MAX);
}

/// Registering version 0 must always be rejected (InvalidSchema).
#[test]
fn test_schema_version_zero_boundary() {
    let (env, client, admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");
    let hash = String::from_str(&env, "sha256:v0");

    let result = client.try_register_schema(&admin, &topic, &0, &hash);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::InvalidSchema),
        _ => panic!("Expected InvalidSchema for version 0"),
    }
}

// ── Replay limit boundary ────────────────────────────────────────────────────

/// Passing u32::MAX as the replay limit must not panic — the contract
/// should simply return however many events exist.
#[test]
fn test_replay_limit_u32_max() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create");

    let topic = String::from_str(&env, "records.vision.create");
    let payload = String::from_str(&env, "p");
    client.publish_event(&admin, &topic, &1, &payload);

    // u32::MAX limit — should not panic, just return the 1 available event
    let events = client.replay_events(&1, &u32::MAX);
    assert_eq!(events.len(), 1);
}

/// Replay from u64::MAX as start ID — no events should match, returns empty.
#[test]
fn test_replay_from_u64_max_event_id() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create");

    let topic = String::from_str(&env, "records.vision.create");
    let payload = String::from_str(&env, "p");
    client.publish_event(&admin, &topic, &1, &payload);

    // No event will have ID >= u64::MAX, so result must be empty
    let events = client.replay_events(&u64::MAX, &10);
    assert_eq!(events.len(), 0);
}

/// Replay limit of zero must return InvalidInput — not a panic.
#[test]
fn test_replay_limit_zero_returns_error() {
    let (_env, client, _admin) = setup();

    let result = client.try_replay_events(&1, &0);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::InvalidInput),
        _ => panic!("Expected InvalidInput for limit=0"),
    }
}

// ── get_event with boundary IDs ──────────────────────────────────────────────

/// Fetching event ID 0 (never assigned — IDs start at 1) must return EventNotFound.
#[test]
fn test_get_event_id_zero() {
    let (_env, client, _admin) = setup();

    let result = client.try_get_event(&0);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::EventNotFound),
        _ => panic!("Expected EventNotFound for event_id=0"),
    }
}

/// Fetching event ID u64::MAX (never published) must return EventNotFound.
#[test]
fn test_get_event_id_u64_max() {
    let (_env, client, _admin) = setup();

    let result = client.try_get_event(&u64::MAX);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::EventNotFound),
        _ => panic!("Expected EventNotFound for event_id=u64::MAX"),
    }
}

// ── Subscription ID boundary ─────────────────────────────────────────────────

/// Unsubscribing with a non-existent subscription ID u64::MAX must return
/// SubscriptionNotFound — not a panic.
#[test]
fn test_unsubscribe_id_u64_max() {
    let (env, client, _admin) = setup();
    let subscriber = Address::generate(&env);

    let result = client.try_unsubscribe(&subscriber, &u64::MAX);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::SubscriptionNotFound),
        _ => panic!("Expected SubscriptionNotFound for subscription_id=u64::MAX"),
    }
}

// ── Checkpoint boundary ──────────────────────────────────────────────────────

/// Fetching checkpoint u64::MAX (never created) must return CheckpointNotFound.
#[test]
fn test_get_checkpoint_u64_max() {
    let (_env, client, _admin) = setup();

    let result = client.try_get_checkpoint(&u64::MAX);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::CheckpointNotFound),
        _ => panic!("Expected CheckpointNotFound for checkpoint_id=u64::MAX"),
    }
}

/// Retrying dead letter at index u32::MAX must return InvalidInput gracefully.
#[test]
fn test_retry_dead_letter_u32_max_index() {
    let (_env, client, admin) = setup();

    let result = client.try_retry_dead_letter(&admin, &u32::MAX);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::InvalidInput),
        _ => panic!("Expected InvalidInput for dead_letter_index=u32::MAX"),
    }
}
