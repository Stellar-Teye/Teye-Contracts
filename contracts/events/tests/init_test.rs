//! Issues 3 & 4: Double Re-initialization Exploits + Initialization Constraints Validation
//!
//! Issue 3: Verifies that calling initialize() more than once safely reverts
//!          with AlreadyInitialized — no state corruption occurs.
//!
//! Issue 4: Validates that all initial state constraints are correctly set and
//!          that contract functions cannot be used before initialization.

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

// ── Issue 3: Double Re-initialization Exploits ───────────────────────────────

/// Calling initialize() a second time must return AlreadyInitialized.
#[test]
fn test_double_initialize_returns_already_initialized() {
    let (env, client, _admin) = setup();
    let attacker = Address::generate(&env);

    let result = client.try_initialize(&attacker);
    match result {
        Err(Ok(e)) => assert_eq!(
            e,
            EventError::AlreadyInitialized,
            "Second initialize must return AlreadyInitialized"
        ),
        _ => panic!("Expected AlreadyInitialized error on second initialize call"),
    }
}

/// Repeated initialize() attempts must all fail — not just the second one.
#[test]
fn test_repeated_initialize_always_fails() {
    let (env, client, _admin) = setup();

    for _ in 0..5 {
        let attacker = Address::generate(&env);
        let result = client.try_initialize(&attacker);
        match result {
            Err(Ok(e)) => assert_eq!(e, EventError::AlreadyInitialized),
            _ => panic!("Every re-initialize attempt must return AlreadyInitialized"),
        }
    }
}

/// After a failed re-initialize, the original admin must remain unchanged.
#[test]
fn test_admin_unchanged_after_failed_reinitialize() {
    let (env, client, original_admin) = setup();
    let attacker = Address::generate(&env);

    // Attempt to hijack admin via re-initialize
    let _ = client.try_initialize(&attacker);

    let current_admin = client.get_admin();
    assert_eq!(
        current_admin, original_admin,
        "Admin must not change after a failed re-initialize"
    );
}

/// After a failed re-initialize, event counter must remain at its pre-attack value.
#[test]
fn test_state_unchanged_after_failed_reinitialize() {
    let (env, client, admin) = setup();

    // Publish some events to advance state
    let topic = String::from_str(&env, "records.vision.create");
    let hash = String::from_str(&env, "sha256:abc");
    client.register_schema(&admin, &topic, &1, &hash);
    let payload = String::from_str(&env, "p");
    client.publish_event(&admin, &topic, &1, &payload);
    client.publish_event(&admin, &topic, &1, &payload);

    let count_before = client.get_event_count();

    // Attempt re-initialize
    let attacker = Address::generate(&env);
    let _ = client.try_initialize(&attacker);

    let count_after = client.get_event_count();
    assert_eq!(
        count_before, count_after,
        "Event count must not reset after failed re-initialize"
    );
}

// ── Issue 4: Initialization Constraints Validation ───────────────────────────

/// Before initialize(), is_initialized() must return false.
#[test]
fn test_not_initialized_before_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventStreamContract, ());
    let client = EventStreamContractClient::new(&env, &contract_id);

    assert!(
        !client.is_initialized(),
        "Contract must not be initialized before initialize() is called"
    );
}

/// After initialize(), is_initialized() must return true.
#[test]
fn test_is_initialized_after_initialize() {
    let (_env, client, _admin) = setup();
    assert!(
        client.is_initialized(),
        "Contract must be initialized after initialize()"
    );
}

/// After initialize(), get_admin() must return the exact address passed in.
#[test]
fn test_admin_set_correctly_on_initialize() {
    let (env, client, admin) = setup();
    let stored_admin = client.get_admin();
    assert_eq!(
        stored_admin, admin,
        "Admin stored must match the address passed to initialize()"
    );
}

/// After initialize(), event counter must start at 0.
#[test]
fn test_event_counter_starts_at_zero() {
    let (_env, client, _admin) = setup();
    assert_eq!(
        client.get_event_count(),
        0,
        "Event counter must be 0 immediately after initialization"
    );
}

/// After initialize(), Lamport clock must start at 0.
#[test]
fn test_lamport_clock_starts_at_zero() {
    let (_env, client, _admin) = setup();
    assert_eq!(
        client.get_lamport_clock(),
        0,
        "Lamport clock must be 0 immediately after initialization"
    );
}

/// Calling get_admin() before initialize() must return NotInitialized.
#[test]
fn test_get_admin_before_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventStreamContract, ());
    let client = EventStreamContractClient::new(&env, &contract_id);

    let result = client.try_get_admin();
    match result {
        Err(Ok(e)) => assert_eq!(
            e,
            EventError::NotInitialized,
            "get_admin() before init must return NotInitialized"
        ),
        _ => panic!("Expected NotInitialized error"),
    }
}

/// Calling get_event_count() before initialize() must return NotInitialized.
#[test]
fn test_get_event_count_before_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventStreamContract, ());
    let client = EventStreamContractClient::new(&env, &contract_id);

    let result = client.try_get_event_count();
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::NotInitialized),
        _ => panic!("Expected NotInitialized error"),
    }
}

/// Calling get_lamport_clock() before initialize() must return NotInitialized.
#[test]
fn test_get_lamport_clock_before_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventStreamContract, ());
    let client = EventStreamContractClient::new(&env, &contract_id);

    let result = client.try_get_lamport_clock();
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::NotInitialized),
        _ => panic!("Expected NotInitialized error"),
    }
}

/// publish_event() before initialize() must return NotInitialized.
#[test]
fn test_publish_before_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventStreamContract, ());
    let client = EventStreamContractClient::new(&env, &contract_id);

    let caller = Address::generate(&env);
    let topic = String::from_str(&env, "records.vision.create");
    let payload = String::from_str(&env, "p");

    let result = client.try_publish_event(&caller, &topic, &1, &payload);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::NotInitialized),
        _ => panic!("Expected NotInitialized error"),
    }
}

/// subscribe() before initialize() must return NotInitialized.
#[test]
fn test_subscribe_before_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventStreamContract, ());
    let client = EventStreamContractClient::new(&env, &contract_id);

    let subscriber = Address::generate(&env);
    let pattern = String::from_str(&env, "records.*");

    let result = client.try_subscribe(&subscriber, &pattern);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::NotInitialized),
        _ => panic!("Expected NotInitialized error"),
    }
}

/// replay_events() before initialize() must return NotInitialized.
#[test]
fn test_replay_before_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventStreamContract, ());
    let client = EventStreamContractClient::new(&env, &contract_id);

    let result = client.try_replay_events(&1, &10);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::NotInitialized),
        _ => panic!("Expected NotInitialized error"),
    }
}
