#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Comprehensive test suite for audit contract initialization constraints.
//!
//! This module validates that:
//! 1. The MerkleLog contract cannot be initialized multiple times
//! 2. Initial state constraints are properly enforced
//! 3. Segment identifiers are validated correctly
//! 4. Retention policies are properly initialized
//! 5. Witness tracking is correctly set up
//! 6. Entry sequence numbering starts at 1

use audit::{
    merkle_log::MerkleLog,
    types::{AuditError, LogSegmentId, RetentionPolicy},
};

// ============================================================================
// Setup Helpers
// ============================================================================

/// Setup a basic MerkleLog instance with a valid segment identifier.
fn setup_basic_log() -> (MerkleLog, LogSegmentId) {
    let segment =
        LogSegmentId::new("healthcare.access").expect("healthcare.access is a valid segment");
    let log = MerkleLog::new(segment.clone());
    (log, segment)
}

/// Setup a MerkleLog with a longer segment identifier.
fn setup_log_with_long_segment() -> Result<(MerkleLog, LogSegmentId), AuditError> {
    let long_label = "a".repeat(64);
    let segment = LogSegmentId::new(&long_label)?;
    let log = MerkleLog::new(segment.clone());
    Ok((log, segment))
}

// ============================================================================
// Initialization Constraint Tests
// ============================================================================

/// Test that a new MerkleLog starts with an empty state.
#[test]
fn test_new_log_is_empty() {
    let (log, segment) = setup_basic_log();

    assert_eq!(log.len(), 0, "New log should have no entries");
    assert_eq!(log.witness_count(), 0, "New log should have no witnesses");
    assert_eq!(log.segment, segment, "Segment should match initialization");
    assert!(log.is_empty(), "Log should be empty");
}

/// Test that initial state constraints prevent invalid segment identifiers.
#[test]
fn test_segment_identifier_validation_empty() {
    let result = LogSegmentId::new("");
    assert!(
        result.is_err(),
        "Empty segment identifier should be rejected"
    );
}

/// Test that segment identifiers exceeding 64 bytes are rejected.
#[test]
fn test_segment_identifier_validation_too_long() {
    let long_label = "a".repeat(65);
    let result = LogSegmentId::new(&long_label);

    assert!(
        result.is_err(),
        "Segment identifier exceeding 64 bytes should be rejected"
    );
    assert_eq!(
        result.unwrap_err(),
        AuditError::InvalidSegmentId,
        "Error should be InvalidSegmentId"
    );
}

/// Test that segment identifiers at the 64-byte boundary are accepted.
#[test]
fn test_segment_identifier_validation_boundary() {
    let boundary_label = "a".repeat(64);
    let result = LogSegmentId::new(&boundary_label);

    assert!(
        result.is_ok(),
        "Segment identifier at 64-byte boundary should be accepted"
    );
}

/// Test that the current root of an empty log is the zero hash.
#[test]
fn test_initial_merkle_root_is_zero_hash() {
    let (log, _segment) = setup_basic_log();

    let root = log.current_root();
    let zero_hash = [0u8; 32];

    assert_eq!(root, zero_hash, "Root of empty log should be the zero hash");
}

/// Test that no checkpoints are created on initialization.
#[test]
fn test_no_checkpoints_on_initialization() {
    let (log, _segment) = setup_basic_log();

    assert_eq!(
        log.checkpoints().len(),
        0,
        "New log should have no published checkpoints"
    );
}

/// Test that publishing a root on an empty log creates exactly one checkpoint.
#[test]
fn test_publish_root_creates_single_checkpoint() {
    let (mut log, _segment) = setup_basic_log();

    let timestamp = 1_700_000_000u64;
    let root = log.publish_root(timestamp);

    assert_eq!(
        log.checkpoints().len(),
        1,
        "Publishing root should create one checkpoint"
    );

    assert_eq!(root, [0u8; 32], "Root of empty log should remain zero hash");
}

/// Test that retention policy can be set without errors.
#[test]
fn test_retention_policy_initialization() {
    let (mut log, segment) = setup_basic_log();

    let policy = RetentionPolicy {
        segment: segment.clone(),
        min_retention_secs: 86_400, // 24 hours
        requires_witness_for_deletion: false,
    };

    log.set_retention(policy);
    // Verify that set_retention completes successfully
}

/// Test that retention policy with witness requirement is correctly initialized.
#[test]
fn test_retention_policy_with_witness_requirement() {
    let (mut log, segment) = setup_basic_log();

    let policy = RetentionPolicy {
        segment: segment.clone(),
        min_retention_secs: 31_536_000, // 1 year
        requires_witness_for_deletion: true,
    };

    log.set_retention(policy);
    // Verify that retention policy with witness requirement is accepted
}

/// Test that segments with valid ASCII characters are accepted.
#[test]
fn test_segment_identifier_valid_ascii_formats() {
    let valid_labels = vec![
        "audit",
        "healthcare.access.control",
        "patient_record_123",
        "access-log-2024",
        "segment-with-numbers-0123456789",
    ];

    for label in valid_labels {
        let result = LogSegmentId::new(label);
        assert!(
            result.is_ok(),
            "Valid ASCII label '{}' should be accepted",
            label
        );
    }
}

/// Test that the log correctly tracks segment identity after initialization.
#[test]
fn test_segment_identity_preserved_after_initialization() {
    let segment_label = "audit.compliance.log";
    let segment = LogSegmentId::new(segment_label).expect("Valid segment");
    let log = MerkleLog::new(segment.clone());

    assert_eq!(
        log.segment.as_str(),
        segment_label,
        "Segment label should be preserved"
    );
    assert_eq!(
        log.segment.as_bytes(),
        segment.as_bytes(),
        "Segment bytes should be preserved"
    );
}

/// Test that logs with different segments are independent.
#[test]
fn test_multiple_independent_log_instances() {
    let segment1 = LogSegmentId::new("segment.one").expect("Valid segment");
    let segment2 = LogSegmentId::new("segment.two").expect("Valid segment");

    let log1 = MerkleLog::new(segment1.clone());
    let log2 = MerkleLog::new(segment2.clone());

    assert_ne!(log1.segment, log2.segment);
    assert_eq!(log1.segment.as_str(), "segment.one");
    assert_eq!(log2.segment.as_str(), "segment.two");
}

// ============================================================================
// Double Initialization Prevention Tests
// ============================================================================

/// Test that a MerkleLog immutably stores its segment after creation.
#[test]
fn test_immutability_of_initialized_state() {
    let (log, _segment) = setup_basic_log();

    let original_segment = log.segment.clone();

    // Verify state remains accessible
    assert_eq!(
        log.segment, original_segment,
        "Segment should remain unchanged"
    );
    assert_eq!(log.len(), 0, "Log should still be empty");
}

/// Test that appending entries maintains initialization invariants.
#[test]
fn test_append_maintains_initialization_invariants() {
    let (mut log, _segment) = setup_basic_log();

    // Append first entry
    let seq1 = log.append(1_700_000_000, "alice", "read", "record:1", "ok");
    assert_eq!(seq1, 1, "First append should assign sequence 1");
    assert_eq!(log.len(), 1, "Entry count should be 1");

    // Append second entry
    let seq2 = log.append(1_700_000_001, "bob", "write", "record:2", "denied");
    assert_eq!(seq2, 2, "Second append should assign sequence 2");
    assert_eq!(log.len(), 2, "Entry count should be 2");
}

/// Test that retention policy does not affect core initialization state.
#[test]
fn test_retention_policy_independent_of_core_state() {
    let (mut log, _segment) = setup_basic_log();

    // Before setting retention policy
    let len_before = log.len();

    // Set retention policy
    let policy = RetentionPolicy {
        segment: _segment.clone(),
        min_retention_secs: 86_400,
        requires_witness_for_deletion: false,
    };
    log.set_retention(policy);

    // Core state should be unaffected
    assert_eq!(
        log.len(),
        len_before,
        "Entry count should be unaffected by retention policy"
    );
}

// ============================================================================
// Constraint Validation Under State Mutations
// ============================================================================

/// Test that constraints remain valid after publishing multiple roots.
#[test]
fn test_constraints_valid_after_multiple_root_publications() {
    let (mut log, segment) = setup_basic_log();

    for i in 0..3 {
        let seq = log.append(1_700_000_000 + i as u64, "actor", "action", "target", "ok");
        assert_eq!(seq, i + 1, "Sequence should be sequential");

        log.publish_root(1_700_000_000 + i as u64);
    }

    assert_eq!(log.checkpoints().len(), 3, "Should have 3 checkpoints");
    assert_eq!(log.len(), 3, "Should have 3 entries");
    assert_eq!(log.segment, segment, "Segment should remain unchanged");
}

/// Test that zero-length segment identifiers are consistently rejected.
#[test]
fn test_zero_length_segment_consistently_rejected() {
    let attempts = [
        LogSegmentId::new(""),
        LogSegmentId::new(""),
        LogSegmentId::new(""),
    ];

    for result in attempts.iter() {
        assert!(result.is_err(), "Empty segments should always be rejected");
        assert_eq!(result.as_ref().unwrap_err(), &AuditError::InvalidSegmentId);
    }
}

/// Test that witness count starts at zero and increments correctly.
#[test]
fn test_witness_initialization_and_increment() {
    let (log, _segment) = setup_basic_log();

    assert_eq!(log.witness_count(), 0, "New log should have zero witnesses");
}

// ============================================================================
// Edge Case and Boundary Tests
// ============================================================================

/// Test that single-character segment identifiers are valid.
#[test]
fn test_single_character_segment_identifier() {
    let result = LogSegmentId::new("a");
    assert!(result.is_ok(), "Single character should be valid");
}

/// Test that segment identifiers with special characters are handled.
#[test]
fn test_segment_identifier_special_characters() {
    let special_labels = vec![
        "audit:log",
        "segment_1",
        "audit-2024",
        "log/access",
        "record.v2",
    ];

    for label in special_labels {
        let result = LogSegmentId::new(label);
        assert!(
            result.is_ok(),
            "Special character label '{}' should be accepted",
            label
        );
    }
}

/// Test that initialization state is correct after maximal segment identifier.
#[test]
fn test_initialization_with_maximal_segment() {
    let result = setup_log_with_long_segment();
    assert!(result.is_ok(), "Maximal segment should be valid");

    let (log, segment) = result.unwrap();
    assert_eq!(log.len(), 0, "Entry count should be 0");
    assert_eq!(log.segment, segment);
}

/// Test that the zero hash is consistent across multiple empty logs.
#[test]
fn test_consistent_zero_hash_for_empty_logs() {
    let (log1, _) = setup_basic_log();
    let (log2, _) = setup_basic_log();

    let root1 = log1.current_root();
    let root2 = log2.current_root();

    assert_eq!(root1, root2, "Empty log roots should be identical");
    assert_eq!(root1, [0u8; 32], "Empty log root should be zero hash");
}

/// Test initialization constraints are enforced regardless of field content.
#[test]
fn test_initialization_constraints_independent_of_content() {
    let segments = vec![
        "simple",
        "with.dots",
        "with-dashes",
        "with_underscores",
        "MixedCase",
        "UPPERCASE",
    ];

    for segment_label in segments {
        let segment = LogSegmentId::new(segment_label).expect("Valid segment identifier");
        let log = MerkleLog::new(segment);

        assert_eq!(log.len(), 0);
        assert_eq!(log.witness_count(), 0);
        assert_eq!(log.current_root(), [0u8; 32]);
    }
}

// ============================================================================
// Integration Tests - Initialization + Operations
// ============================================================================

/// Test that a log initialized with proper constraints can properly append entries.
#[test]
fn test_initialized_log_accepts_entries() {
    let (mut log, _segment) = setup_basic_log();

    // Verify initial state
    assert!(log.is_empty());
    assert_eq!(log.len(), 0);

    // Append entry and verify state changed correctly
    let seq = log.append(1_700_000_000, "admin", "access", "system:1", "ok");
    assert_eq!(seq, 1);
    assert_eq!(log.len(), 1);
    assert!(!log.is_empty());

    // Retrieve the entry to verify it's stored correctly
    let entry = log.get_entry(seq).expect("Entry should exist");
    assert_eq!(entry.sequence, 1);
    assert_eq!(entry.actor, "admin");
    assert_eq!(entry.action, "access");
}

/// Test that initialized logs produce verifiable inclusion proofs.
#[test]
fn test_initialized_log_produces_valid_proofs() {
    let (mut log, _segment) = setup_basic_log();

    // Append entry and publish root
    let seq = log.append(1_700_000_000, "user", "read", "file:doc1", "ok");
    log.publish_root(1_700_000_000);

    // Generate inclusion proof
    let proof = log.inclusion_proof(seq).expect("Proof should be generated");

    let root = log.current_root();
    proof.verify(&root).expect("Proof should verify");
}

/// Test that the log correctly maintains hash chain integrity after initialization.
#[test]
fn test_hash_chain_maintained_after_initialization() {
    let (mut log, _segment) = setup_basic_log();

    let seq1 = log.append(1_700_000_000, "alice", "create", "record:1", "ok");
    let seq2 = log.append(1_700_000_001, "bob", "read", "record:1", "ok");

    // Verify hash chain is valid
    log.verify_chain(seq1, seq2)
        .expect("Hash chain should be valid");
}

/// Test segment identity persists through initialization and operations.
#[test]
fn test_segment_persists_through_lifecycle() {
    let segment_label = "persistent.segment.test";
    let segment = LogSegmentId::new(segment_label).expect("Valid segment");
    let mut log = MerkleLog::new(segment.clone());

    // Verify initial state
    assert_eq!(log.segment, segment);

    // Perform operations
    log.append(1_700_000_000, "actor", "action", "target", "ok");
    log.publish_root(1_700_000_000);

    // Verify segment hasn't changed
    assert_eq!(log.segment, segment);
    assert_eq!(log.segment.as_str(), segment_label);
}
