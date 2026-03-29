//! Event emission verification tests for the `audit` crate.
//!
//! "Events" here are audit-log entries emitted into the append-only Merkle
//! log. Tests verify that each append produces the correct hash-chain linkage,
//! that Merkle inclusion proofs validate against the current root, and that
//! the root is updated deterministically after every append.

use audit::{merkle_log::MerkleLog, types::LogSegmentId};

// ── Hash-chain linkage ────────────────────────────────────────────────────────

#[test]
fn test_first_entry_prev_hash_is_zero() {
    let seg = LogSegmentId::new("chain-init").unwrap();
    let mut log = MerkleLog::new(seg);
    let seq = log
        .append(1_000, "alice", "read", "record:1", "ok")
        .unwrap();
    let entry = log.get_entry(seq).expect("entry must exist");
    assert_eq!(
        entry.prev_hash, [0u8; 32],
        "first entry prev_hash must be all-zero sentinel"
    );
}

#[test]
fn test_second_entry_prev_hash_links_to_first() {
    let seg = LogSegmentId::new("chain-link").unwrap();
    let mut log = MerkleLog::new(seg);
    let seq1 = log
        .append(1_000, "alice", "write", "record:1", "ok")
        .unwrap();
    let seq2 = log.append(1_001, "bob", "read", "record:1", "ok").unwrap();
    let entry1 = log.get_entry(seq1).expect("entry 1 must exist");
    let entry2 = log.get_entry(seq2).expect("entry 2 must exist");
    assert_eq!(
        entry2.prev_hash, entry1.entry_hash,
        "second entry prev_hash must equal first entry's entry_hash"
    );
}

#[test]
fn test_hash_chain_across_many_entries() {
    let seg = LogSegmentId::new("chain-long").unwrap();
    let mut log = MerkleLog::new(seg);
    for i in 0..8u64 {
        log.append(1_000 + i, "actor", "action", "target", "ok")
            .unwrap();
    }
    // verify_chain should confirm an unbroken chain over all 8 entries.
    assert!(
        log.verify_chain(1, 8).is_ok(),
        "hash chain over 8 entries must be intact"
    );
}

// ── Root changes after each append ────────────────────────────────────────────

#[test]
fn test_root_changes_with_each_append() {
    let seg = LogSegmentId::new("root-change").unwrap();
    let mut log = MerkleLog::new(seg);
    let root0 = log.current_root();
    log.append(1_000, "alice", "read", "record:1", "ok")
        .unwrap();
    let root1 = log.current_root();
    log.append(1_001, "bob", "write", "record:2", "ok").unwrap();
    let root2 = log.current_root();
    assert_ne!(root0, root1, "root must change after first append");
    assert_ne!(root1, root2, "root must change after second append");
}

#[test]
fn test_same_content_different_timestamp_yields_different_root() {
    let make_log = |ts: u64| {
        let seg = LogSegmentId::new("ts-root").unwrap();
        let mut log = MerkleLog::new(seg);
        log.append(ts, "actor", "action", "target", "ok").unwrap();
        log.current_root()
    };
    let r1 = make_log(1_000);
    let r2 = make_log(2_000);
    assert_ne!(r1, r2, "different timestamps must produce different roots");
}

// ── Merkle inclusion proofs ───────────────────────────────────────────────────

#[test]
fn test_inclusion_proof_verifies_for_single_entry() {
    let seg = LogSegmentId::new("proof-single").unwrap();
    let mut log = MerkleLog::new(seg);
    let seq = log
        .append(1_000, "alice", "read", "record:1", "ok")
        .unwrap();
    let root = log.current_root();
    let proof = log.inclusion_proof(seq).expect("proof must exist");
    assert!(
        proof.verify(&root).is_ok(),
        "inclusion proof for a single entry must verify"
    );
}

#[test]
fn test_inclusion_proofs_verify_for_all_entries() {
    let seg = LogSegmentId::new("proof-all").unwrap();
    let mut log = MerkleLog::new(seg);
    let mut seqs = vec![];
    for i in 0..6u64 {
        seqs.push(
            log.append(1_000 + i, "actor", "action", "target", "ok")
                .unwrap(),
        );
    }
    let root = log.current_root();
    for seq in seqs {
        let proof = log.inclusion_proof(seq).expect("proof must exist");
        assert!(
            proof.verify(&root).is_ok(),
            "inclusion proof for seq {seq} must verify"
        );
    }
}

#[test]
fn test_stale_root_invalidates_proof() {
    let seg = LogSegmentId::new("stale-root").unwrap();
    let mut log = MerkleLog::new(seg);
    let seq1 = log.append(1_000, "alice", "read", "r1", "ok").unwrap();
    let old_root = log.current_root();
    // Append another entry so the root advances.
    log.append(1_001, "bob", "write", "r2", "ok").unwrap();
    // The proof for seq1 was built against the old root.
    let proof = log.inclusion_proof(seq1).expect("proof must exist");
    // Verifying seq1's proof against the updated root must fail.
    assert!(
        proof.verify(&old_root).is_err(),
        "proof verified against a stale root must fail"
    );
}

// ── query_range correctness ───────────────────────────────────────────────────

#[test]
fn test_query_range_returns_correct_entries() {
    let seg = LogSegmentId::new("range-test").unwrap();
    let mut log = MerkleLog::new(seg);
    for i in 0..5u64 {
        log.append(1_000 + i, "actor", "action", "target", "ok")
            .unwrap();
    }
    let range = log.query_range(2, 4);
    assert_eq!(range.len(), 3, "range [2, 4] must return 3 entries");
    assert_eq!(range[0].sequence, 2);
    assert_eq!(range[2].sequence, 4);
}
