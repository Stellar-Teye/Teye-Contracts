//! Zero-value and boundary parameter tests for the `audit` crate.
//!
//! Verifies that functions accept or correctly reject zero, empty, and extreme
//! boundary inputs without panicking or producing corrupt state.

use audit::{
    merkle_log::MerkleLog,
    search::{SearchEngine, SearchKey},
    types::{AuditError, LogSegmentId},
};

// ── LogSegmentId boundary tests ───────────────────────────────────────────────

#[test]
fn test_segment_id_empty_string_is_rejected() {
    // An empty label has no identity and must be rejected.
    assert!(LogSegmentId::new("").is_err());
}

#[test]
fn test_segment_id_max_length_is_valid() {
    // A 64-byte label is exactly at the limit — must be accepted.
    let label = "a".repeat(64);
    assert!(LogSegmentId::new(&label).is_ok());
}

#[test]
fn test_segment_id_over_max_length_rejected() {
    // A 65-byte label exceeds the limit — must return InvalidSegmentId.
    let label = "a".repeat(65);
    assert_eq!(
        LogSegmentId::new(&label),
        Err(AuditError::InvalidSegmentId),
        "65-byte label must be rejected"
    );
}

// ── MerkleLog append with zero / boundary values ──────────────────────────────

#[test]
fn test_append_with_zero_timestamp() {
    let seg = LogSegmentId::new("zero-ts").unwrap();
    let mut log = MerkleLog::new(seg);
    // timestamp = 0 is a valid edge case; must be accepted without panic.
    let seq = log.append(0, "actor", "action", "target", "ok").unwrap();
    assert_eq!(seq, 1, "first entry must have sequence 1");
}

#[test]
fn test_append_with_u64_max_timestamp() {
    let seg = LogSegmentId::new("max-ts").unwrap();
    let mut log = MerkleLog::new(seg);
    let seq = log
        .append(u64::MAX, "actor", "action", "target", "ok")
        .unwrap();
    assert_eq!(seq, 1);
    // Inclusion proof must still be constructable.
    assert!(log.inclusion_proof(seq).is_ok());
}

#[test]
fn test_append_with_empty_actor_and_action() {
    let seg = LogSegmentId::new("empty-fields").unwrap();
    let mut log = MerkleLog::new(seg);
    // All string fields empty — must be accepted without panic.
    let seq = log.append(1000, "", "", "", "").unwrap();
    assert_eq!(seq, 1);
    assert!(log.get_entry(seq).is_ok());
}

#[test]
fn test_append_increments_sequence_correctly() {
    let seg = LogSegmentId::new("seq-check").unwrap();
    let mut log = MerkleLog::new(seg);
    let s1 = log.append(0, "", "", "", "").unwrap();
    let s2 = log.append(0, "", "", "", "").unwrap();
    let s3 = log.append(0, "", "", "", "").unwrap();
    assert_eq!(s1, 1);
    assert_eq!(s2, 2);
    assert_eq!(s3, 3);
}

// ── get_entry on missing sequence ─────────────────────────────────────────────

#[test]
fn test_get_entry_nonexistent_returns_error() {
    let seg = LogSegmentId::new("missing").unwrap();
    let log = MerkleLog::new(seg);
    // No entries appended — sequence 1 does not exist.
    let result = log.get_entry(1);
    assert!(
        matches!(result, Err(AuditError::EntryNotFound { sequence: 1 })),
        "get_entry on missing sequence must return EntryNotFound"
    );
}

#[test]
fn test_get_entry_sequence_zero_returns_error() {
    let seg = LogSegmentId::new("seq-zero").unwrap();
    let mut log = MerkleLog::new(seg);
    log.append(1000, "actor", "action", "target", "ok").unwrap();
    // Sequence 0 is never a valid entry (sequences start at 1).
    let result = log.get_entry(0);
    assert!(result.is_err());
}

// ── inclusion_proof on non-existent entry ─────────────────────────────────────

#[test]
fn test_inclusion_proof_on_empty_log_returns_error() {
    let seg = LogSegmentId::new("empty-proof").unwrap();
    let log = MerkleLog::new(seg);
    assert!(
        log.inclusion_proof(1).is_err(),
        "proof on empty log must return an error"
    );
}

// ── SearchEngine with empty / zero-length inputs ──────────────────────────────

#[test]
fn test_search_query_on_empty_engine_returns_empty() {
    let key = SearchKey::from_bytes(&[0u8; 32]).unwrap();
    let engine = SearchEngine::new(key);
    assert!(
        engine.query("anything").is_empty(),
        "query on un-indexed engine must return empty results"
    );
}

#[test]
fn test_search_query_empty_string_returns_empty() {
    let key = SearchKey::from_bytes(&[0u8; 32]).unwrap();
    let mut engine = SearchEngine::new(key);
    engine.index_entry(1, "actor", "read", "target", "ok", &[]);
    // Empty-string query should match nothing.
    assert!(engine.query("").is_empty());
}

#[test]
fn test_search_key_all_zeros_is_valid() {
    // A 32-byte all-zero key should be constructed without error.
    assert!(SearchKey::from_bytes(&[0u8; 32]).is_ok());
}

#[test]
fn test_search_key_wrong_length_rejected() {
    // A key that is not 32 bytes must be rejected.
    assert!(SearchKey::from_bytes(&[0u8; 16]).is_err());
    assert!(SearchKey::from_bytes(&[]).is_err());
}
