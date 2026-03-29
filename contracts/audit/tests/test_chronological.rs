//! Chronological-order enforcement tests for the `audit` crate.
//!
//! Verifies that `MerkleLog::append` rejects out-of-order timestamps and
//! returns a structured `OutOfOrderTimestamp` error with the correct fields.

use audit::{
    merkle_log::MerkleLog,
    types::{AuditError, LogSegmentId},
};

fn seg(name: &str) -> LogSegmentId {
    LogSegmentId::new(name).unwrap()
}

// ── Basic acceptance / rejection ──────────────────────────────────────────────

#[test]
fn test_first_entry_any_timestamp_accepted() {
    let mut log = MerkleLog::new(seg("chrono-first"));
    // Any timestamp for the first entry must succeed (no predecessor to compare).
    assert!(log.append(0, "actor", "action", "target", "ok").is_ok());
}

#[test]
fn test_equal_timestamp_accepted() {
    // Two entries sharing the same ledger timestamp (same block) are valid.
    let mut log = MerkleLog::new(seg("chrono-equal"));
    log.append(1_000, "a", "read", "r:1", "ok").unwrap();
    assert!(
        log.append(1_000, "b", "write", "r:2", "ok").is_ok(),
        "equal timestamp must be accepted (same ledger slot)"
    );
}

#[test]
fn test_strictly_increasing_timestamp_accepted() {
    let mut log = MerkleLog::new(seg("chrono-inc"));
    log.append(1_000, "a", "read", "r:1", "ok").unwrap();
    assert!(
        log.append(1_001, "b", "write", "r:2", "ok").is_ok(),
        "strictly increasing timestamp must be accepted"
    );
}

#[test]
fn test_decreasing_timestamp_rejected() {
    let mut log = MerkleLog::new(seg("chrono-dec"));
    log.append(2_000, "a", "read", "r:1", "ok").unwrap();
    let result = log.append(1_999, "b", "write", "r:2", "ok");
    assert!(
        result.is_err(),
        "timestamp less than the previous entry must be rejected"
    );
}

// ── Error variant structure ───────────────────────────────────────────────────

#[test]
fn test_out_of_order_error_contains_correct_sequence() {
    let mut log = MerkleLog::new(seg("chrono-seq"));
    log.append(5_000, "a", "read", "r:1", "ok").unwrap();
    let err = log.append(4_999, "b", "write", "r:2", "ok").unwrap_err();

    match err {
        AuditError::OutOfOrderTimestamp { sequence, .. } => {
            assert_eq!(
                sequence, 2,
                "sequence in error must be the rejected entry's sequence"
            );
        }
        other => panic!("expected OutOfOrderTimestamp, got {other:?}"),
    }
}

#[test]
fn test_out_of_order_error_contains_supplied_timestamp() {
    let mut log = MerkleLog::new(seg("chrono-supplied"));
    log.append(5_000, "a", "read", "r:1", "ok").unwrap();
    let err = log.append(3_000, "b", "write", "r:2", "ok").unwrap_err();

    match err {
        AuditError::OutOfOrderTimestamp { supplied, .. } => {
            assert_eq!(
                supplied, 3_000,
                "supplied field must equal the rejected timestamp"
            );
        }
        other => panic!("expected OutOfOrderTimestamp, got {other:?}"),
    }
}

#[test]
fn test_out_of_order_error_contains_minimum_timestamp() {
    let mut log = MerkleLog::new(seg("chrono-min"));
    log.append(7_000, "a", "read", "r:1", "ok").unwrap();
    let err = log.append(6_000, "b", "write", "r:2", "ok").unwrap_err();

    match err {
        AuditError::OutOfOrderTimestamp { minimum, .. } => {
            assert_eq!(
                minimum, 7_000,
                "minimum must equal the last accepted timestamp"
            );
        }
        other => panic!("expected OutOfOrderTimestamp, got {other:?}"),
    }
}

// ── Log integrity after rejection ─────────────────────────────────────────────

#[test]
fn test_rejected_entry_not_appended_to_log() {
    let mut log = MerkleLog::new(seg("chrono-no-append"));
    log.append(1_000, "a", "read", "r:1", "ok").unwrap();
    let _ = log.append(500, "b", "write", "r:2", "ok"); // must fail

    // Only the first entry should exist.
    assert_eq!(
        log.get_entry(1).is_ok(),
        true,
        "first entry must still exist"
    );
    assert_eq!(
        log.get_entry(2).is_ok(),
        false,
        "rejected entry must not be stored"
    );
}

#[test]
fn test_root_unchanged_after_rejection() {
    let mut log = MerkleLog::new(seg("chrono-root"));
    log.append(1_000, "a", "read", "r:1", "ok").unwrap();
    let root_before = log.current_root();

    let _ = log.append(500, "b", "write", "r:2", "ok"); // must fail

    assert_eq!(
        log.current_root(),
        root_before,
        "Merkle root must not change after a rejected append"
    );
}

#[test]
fn test_valid_entry_accepted_after_failed_append() {
    // A failed append must not corrupt the log; subsequent valid appends succeed.
    let mut log = MerkleLog::new(seg("chrono-recover"));
    log.append(1_000, "a", "read", "r:1", "ok").unwrap();
    let _ = log.append(500, "b", "write", "r:2", "ok"); // rejected
    assert!(
        log.append(2_000, "c", "delete", "r:3", "ok").is_ok(),
        "valid timestamp after a rejected append must succeed"
    );
}

// ── Multi-entry sequences ─────────────────────────────────────────────────────

#[test]
fn test_long_monotonic_sequence_accepted() {
    let mut log = MerkleLog::new(seg("chrono-long"));
    for ts in (0u64..20).map(|i| i * 100) {
        log.append(ts, "actor", "action", "target", "ok")
            .unwrap_or_else(|e| panic!("append at ts={ts} failed: {e:?}"));
    }
}

#[test]
fn test_rejection_in_the_middle_of_sequence() {
    let mut log = MerkleLog::new(seg("chrono-middle"));
    for ts in [100u64, 200, 300, 400, 500] {
        log.append(ts, "actor", "action", "target", "ok").unwrap();
    }
    // Insert a past timestamp — must be rejected.
    assert!(
        log.append(250, "actor", "action", "target", "ok").is_err(),
        "timestamp in the past must be rejected even mid-sequence"
    );
    // Log should still have exactly 5 entries.
    assert_eq!(log.get_entry(5).is_ok(), true);
    assert_eq!(log.get_entry(6).is_ok(), false);
}
