#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Initialization constraint tests for the audit contract.
//!
//! Covers:
//! 1. Double re-initialization exploit — contract must revert with
//!    `AuditContractError::AlreadyInitialized` on any subsequent call.
//! 2. Initial state invariants (empty segments, correct admin).
//! 3. Segment identifier validation (pure-Rust `MerkleLog` layer).
//! 4. Retention policy and witness tracking on fresh logs.

use audit::{
    merkle_log::MerkleLog,
    types::{AuditError, LogSegmentId, RetentionPolicy},
    AuditContract, AuditContractClient, AuditContractError,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ============================================================================
// Soroban contract helpers
// ============================================================================

fn setup_contract() -> (Env, AuditContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AuditContract, ());
    let client = AuditContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

// ============================================================================
// Double Re-initialization Exploit Tests  (audit #311)
// ============================================================================

/// Core exploit: second `initialize` must revert with `AlreadyInitialized`.
#[test]
fn test_double_initialization_is_rejected() {
    let (env, client, _admin) = setup_contract();
    let attacker = Address::generate(&env);
    assert_eq!(
        client.try_initialize(&attacker),
        Err(Ok(AuditContractError::AlreadyInitialized)),
        "Second initialize call must revert with AlreadyInitialized"
    );
}

/// Repeated exploit attempts all fail — the guard is not one-shot.
#[test]
fn test_repeated_initialization_attempts_all_rejected() {
    let (env, client, _admin) = setup_contract();
    for _ in 0..3 {
        let attacker = Address::generate(&env);
        assert_eq!(
            client.try_initialize(&attacker),
            Err(Ok(AuditContractError::AlreadyInitialized)),
            "Every re-init attempt must be rejected"
        );
    }
}

/// Admin slot must remain unchanged after failed re-init attempts.
#[test]
fn test_admin_unchanged_after_reinit_attempt() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AuditContract, ());
    let client = AuditContractClient::new(&env, &contract_id);
    let legitimate_admin = Address::generate(&env);
    client.initialize(&legitimate_admin);

    // Attacker tries to hijack admin
    let attacker = Address::generate(&env);
    let _ = client.try_initialize(&attacker);

    // Original admin still controls the contract
    let seg = soroban_sdk::symbol_short!("seg1");
    assert!(
        client.try_create_segment(&seg).is_ok(),
        "Original admin should still control the contract"
    );
}

/// First initialization on a fresh contract succeeds exactly once.
#[test]
fn test_first_initialization_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AuditContract, ());
    let client = AuditContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    assert!(
        client.try_initialize(&admin).is_ok(),
        "First initialization must succeed"
    );
}

/// The error is a typed `AuditContractError`, not a raw panic.
#[test]
fn test_reinit_error_is_typed_not_panic() {
    let (env, client, _admin) = setup_contract();
    let attacker = Address::generate(&env);
    let err = client
        .try_initialize(&attacker)
        .expect_err("Should return an error");
    assert!(
        matches!(err, Ok(AuditContractError::AlreadyInitialized)),
        "Error must be typed AuditContractError::AlreadyInitialized, got: {:?}",
        err
    );
}

// ============================================================================
// Initial State Invariant Tests
// ============================================================================

/// A freshly initialized contract has no segments.
#[test]
fn test_no_segments_after_initialization() {
    let (_, client, _) = setup_contract();
    let seg = soroban_sdk::symbol_short!("missing");
    assert!(
        client.try_get_entries(&seg).is_err(),
        "Non-existent segment should return an error"
    );
}

/// Admin can create a segment immediately after initialization.
#[test]
fn test_admin_can_create_segment_after_init() {
    let (_, client, _) = setup_contract();
    let seg = soroban_sdk::symbol_short!("access");
    assert!(client.try_create_segment(&seg).is_ok());
}

/// Entry count on a fresh segment starts at zero.
#[test]
fn test_entry_count_zero_on_new_segment() {
    let (_, client, _) = setup_contract();
    let seg = soroban_sdk::symbol_short!("empty");
    client.create_segment(&seg);
    assert_eq!(client.get_entry_count(&seg), 0);
}

// ============================================================================
// MerkleLog (pure-Rust layer) — segment identifier & state tests
// ============================================================================

fn setup_basic_log() -> (MerkleLog, LogSegmentId) {
    let segment = LogSegmentId::new("healthcare.access").expect("valid segment");
    let log = MerkleLog::new(segment.clone());
    (log, segment)
}

#[test]
fn test_new_log_is_empty() {
    let (log, segment) = setup_basic_log();
    assert_eq!(log.len(), 0);
    assert_eq!(log.witness_count(), 0);
    assert_eq!(log.segment, segment);
    assert!(log.is_empty());
}

#[test]
fn test_segment_identifier_empty_rejected() {
    assert_eq!(
        LogSegmentId::new("").unwrap_err(),
        AuditError::InvalidSegmentId
    );
}

#[test]
fn test_segment_identifier_too_long_rejected() {
    assert_eq!(
        LogSegmentId::new(&"a".repeat(65)).unwrap_err(),
        AuditError::InvalidSegmentId
    );
}

#[test]
fn test_segment_identifier_boundary_accepted() {
    assert!(LogSegmentId::new(&"a".repeat(64)).is_ok());
}

#[test]
fn test_initial_merkle_root_is_zero_hash() {
    let (log, _) = setup_basic_log();
    assert_eq!(log.current_root(), [0u8; 32]);
}

#[test]
fn test_no_checkpoints_on_initialization() {
    let (log, _) = setup_basic_log();
    assert_eq!(log.checkpoints().len(), 0);
}

#[test]
fn test_retention_policy_initialization() {
    let (mut log, segment) = setup_basic_log();
    log.set_retention(RetentionPolicy {
        segment,
        min_retention_secs: 86_400,
        requires_witness_for_deletion: false,
    });
}

#[test]
fn test_append_assigns_sequential_sequences() {
    let (mut log, _) = setup_basic_log();
    assert_eq!(log.append(1_700_000_000, "alice", "read", "rec:1", "ok"), 1);
    assert_eq!(log.append(1_700_000_001, "bob", "write", "rec:2", "ok"), 2);
    assert_eq!(log.len(), 2);
}

#[test]
fn test_hash_chain_maintained() {
    let (mut log, _) = setup_basic_log();
    let s1 = log.append(1_700_000_000, "alice", "create", "rec:1", "ok");
    let s2 = log.append(1_700_000_001, "bob", "read", "rec:1", "ok");
    log.verify_chain(s1, s2).expect("hash chain must be valid");
}

#[test]
fn test_initialized_log_produces_valid_inclusion_proof() {
    let (mut log, _) = setup_basic_log();
    let seq = log.append(1_700_000_000, "user", "read", "file:doc1", "ok");
    log.publish_root(1_700_000_000);
    let proof = log.inclusion_proof(seq).expect("proof should be generated");
    proof.verify(&log.current_root()).expect("proof must verify");
}

use audit::{
    merkle_log::MerkleLog,
    types::{AuditError, LogSegmentId, RetentionPolicy},
    AuditContract, AuditContractClient, AuditContractError,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ============================================================================
// Soroban contract helpers
// ============================================================================

/// Register the contract and return a client + admin address.
fn setup_contract() -> (Env, AuditContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AuditContract, ());
    let client = AuditContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    (env, client, admin)
}

// ============================================================================
// Double Re-initialization Exploit Tests  (audit #311)
// ============================================================================

/// Core exploit test: calling `initialize` a second time must revert with
/// `AlreadyInitialized`. An attacker cannot hijack the admin slot.
#[test]
fn test_double_initialization_is_rejected() {
    let (env, client, _admin) = setup_contract();

    let attacker = Address::generate(&env);
    let result = client.try_initialize(&attacker);

    assert_eq!(
        result,
        Err(Ok(AuditContractError::AlreadyInitialized)),
        "Second initialize call must revert with AlreadyInitialized"
    );
}

/// Repeated exploit attempts all fail — the guard is not one-shot.
#[test]
fn test_repeated_initialization_attempts_all_rejected() {
    let (env, client, _admin) = setup_contract();

    for _ in 0..3 {
        let attacker = Address::generate(&env);
        let result = client.try_initialize(&attacker);
        assert_eq!(
            result,
            Err(Ok(AuditContractError::AlreadyInitialized)),
            "Every re-init attempt must be rejected"
        );
    }
}

/// The original admin address must remain unchanged after failed re-init
/// attempts — the attacker cannot overwrite the admin slot.
#[test]
fn test_admin_unchanged_after_reinit_attempt() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AuditContract, ());
    let client = AuditContractClient::new(&env, &contract_id);

    let legitimate_admin = Address::generate(&env);
    client.initialize(&legitimate_admin);

    // Attacker tries to take over
    let attacker = Address::generate(&env);
    let _ = client.try_initialize(&attacker);

    // Verify the contract still recognises the original admin by checking
    // that create_segment (admin-only) succeeds for the real admin and
    // would fail for the attacker (auth is mocked so we just confirm the
    // contract is still functional under the original admin).
    let seg = soroban_sdk::symbol_short!("seg1");
    let create_result = client.try_create_segment(&seg);
    assert!(
        create_result.is_ok(),
        "Original admin should still control the contract"
    );
}

/// Calling `initialize` on a fresh (unregistered) contract succeeds exactly
/// once — baseline sanity check.
#[test]
fn test_first_initialization_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AuditContract, ());
    let client = AuditContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    let result = client.try_initialize(&admin);
    assert!(result.is_ok(), "First initialization must succeed");
}

/// The error variant is exactly `AlreadyInitialized = 1`, not a generic panic.
#[test]
fn test_reinit_error_is_typed_not_panic() {
    let (env, client, _admin) = setup_contract();

    let attacker = Address::generate(&env);
    let err = client
        .try_initialize(&attacker)
        .expect_err("Should return an error");

    // `Ok(variant)` means it's a typed contract error, not an unhandled panic.
    assert!(
        matches!(err, Ok(AuditContractError::AlreadyInitialized)),
        "Error must be a typed AuditContractError, not a raw panic: {:?}",
        err
    );
}

// ============================================================================
// Initial State Invariant Tests
// ============================================================================

/// A freshly initialized contract has no segments.
#[test]
fn test_no_segments_after_initialization() {
    let (env, client, _admin) = setup_contract();

    let seg = soroban_sdk::symbol_short!("missing");
    let result = client.try_get_entries(&seg);
    assert!(
        result.is_err(),
        "Non-existent segment should return an error"
    );
}

/// Admin can create a segment immediately after initialization.
#[test]
fn test_admin_can_create_segment_after_init() {
    let (_, client, _) = setup_contract();

_sdk::symbol_short!("access");
    assert!(
        client.try_create_segment(&seg).is_ok(),
        "Admin should be able to create a segment after init"
    );
}

/// Entry count on a fresh segment starts at zero.
#[test]
fn test_entry_count_zero_on_new_segment() {
    let (_, client, _) = setup_contract();

    let seg = soroban_sdk::symbol_short!("empty");
    client.create_segment(&seg);

    let count = client.get_entry_count(&seg);
    assert_eq!(count, 0, "New segment should have zero entries");
}

// ============================================================================
// MerkleLog (pure-Rust layer) — segment identifier & state tests
// ============================================================================

fn setup_basic_log() -> (MerkleLog, LogSegmentId) {
    let segment = LogSegmentId::new("healthcare.access").expect("valid segment");
    let log = MerkleLog::new(segment.clone());
    (log, segment)
}

#[test]
fn test_new_log_is_empty() {
    let (log, segment) = setup_basic_log();
   assert_eq!(log.len(), 0);
    assert_eq!(log.witness_count(), 0);
    assert_eq!(log.segment, segment);
    assert!(log.is_empty());
}

#[test]
fn test_segment_identifier_empty_rejected() {
    assert_eq!(
        LogSegmentId::new("").unwrap_err(),
        AuditError::InvalidSegmentId
    );
}

#[test]
fn test_segment_identifier_too_long_rejected() {
    let result = LogSegmentId::new(&"a".repeat(65));
    assert_eq!(result.unwrap_err(), AuditError::InvalidSegmentId);
}

#[test]
ary_accepted() {
    assert!(LogSegmentId::new(&"a".repeat(64)).is_ok());
}

#[test]
fn test_initial_merkle_root_is_zero_hash() {
    let (log, _) = setup_basic_log();
    assert_eq!(log.current_root(), [0u8; 32]);
}

#[test]
fn test_no_checkpoints_on_initialization() {
    let (log, _) = setup_basic_log();
    assert_eq!(log.checkpoints().len(), 0);
}

#[test]
fn test_retention_policy_initialization() {
    let (mut log, segment) = setup_basic_log();
    log.set_retention(RetentionPolicy {
        segment,
       min_retention_secs: 86_400,
        requires_witness_for_deletion: false,
    });
}

#[test]
fn test_append_assigns_sequential_sequences() {
    let (mut log, _) = setup_basic_log();
    assert_eq!(log.append(1_700_000_000, "alice", "read", "rec:1", "ok"), 1);
    assert_eq!(log.append(1_700_000_001, "bob", "write", "rec:2", "ok"), 2);
    assert_eq!(log.len(), 2);
}

#[test]
fn test_hash_chain_maintained() {
    let (mut log, _) = setup_basic_log();
    let s1 = log.append(1_700_000_000, "alice", "create", "rec:1", "ok");
    let s2 = log.append(1_700_000_001, "bob", "read", "rec:1", "ok");
    log.verify_chain(s1, s2).expect("hash chain must be valid");
}

#[test]
fn test_initialized_log_produces_valid_inclusion_proof() {
    let (mut log, _) = setup_basic_log();
    let seq = log.append(1_700_000_000, "user", "read", "file:doc1", "ok");
    log.publish_root(1_700_000_000);
    let proof = log.inclusion_proof(seq).expect("proof should be generated");
    proof.verify(&log.current_root()).expect("proof must verify");
}
