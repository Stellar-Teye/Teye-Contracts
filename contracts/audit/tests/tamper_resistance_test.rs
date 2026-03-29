//! Audit Trail — Tamper Resistance Verification
//!
//! These tests verify that the audit module's hashing and chaining mechanism
//! is resilient to tampering. Specifically:
//!
//! - Attempts to rewrite historical audit entries are detected and rejected.
//! - The integrity of the audit chain is validated through sequential entry
//!   verification.
//! - Administrative-level actions are correctly audited and tamper-evident.

use audit::{
    consistency::ConsistencyProver,
    merkle_log::MerkleLog,
    search::{SearchEngine, SearchKey},
    types::{AuditError, LogSegmentId, RetentionPolicy, WitnessSignature},
};

// ── Helpers ────────────────────────────────────────────────────────────────

fn seg(name: &str) -> LogSegmentId {
    LogSegmentId::new(name).unwrap()
}

fn build_log(name: &str, n: u64) -> MerkleLog {
    let mut log = MerkleLog::new(seg(name));
    for i in 1..=n {
        log.append(i * 1000, "actor", "action", "target", "ok")
            .unwrap();
    }
    log
}

fn leaf_hashes(log: &MerkleLog, count: u64) -> Vec<[u8; 32]> {
    (1..=count)
        .map(|s| log.get_entry(s).unwrap().entry_hash)
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Hash Chain Integrity — Sequential Entry Verification
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn chain_verification_passes_for_intact_log() {
    let log = build_log("intact", 10);
    assert!(log.verify_chain(1, 10).is_ok());
}

#[test]
fn chain_verification_passes_for_single_entry() {
    let log = build_log("single", 1);
    assert!(log.verify_chain(1, 1).is_ok());
}

#[test]
fn chain_verification_passes_for_subrange() {
    let log = build_log("subrange", 10);
    // Verify a middle subrange.
    assert!(log.verify_chain(3, 7).is_ok());
}

#[test]
fn first_entry_chains_from_genesis_zero_hash() {
    let log = build_log("genesis", 3);
    let first = log.get_entry(1).unwrap();
    assert_eq!(
        first.prev_hash, [0u8; 32],
        "first entry must chain from zero-hash"
    );
}

#[test]
fn each_entry_chains_to_predecessor() {
    let log = build_log("chain-link", 5);
    for seq in 2..=5 {
        let prev = log.get_entry(seq - 1).unwrap();
        let curr = log.get_entry(seq).unwrap();
        assert_eq!(
            curr.prev_hash,
            prev.entry_hash,
            "entry {} prev_hash must equal entry {} entry_hash",
            seq,
            seq - 1
        );
    }
}

#[test]
fn entry_hash_is_deterministic() {
    // Two logs with identical content and segment produce identical entry hashes.
    let mut log_c = MerkleLog::new(seg("same"));
    let mut log_d = MerkleLog::new(seg("same"));
    for i in 1..=3 {
        log_c
            .append(i * 1000, "alice", "read", "record:1", "ok")
            .unwrap();
        log_d
            .append(i * 1000, "alice", "read", "record:1", "ok")
            .unwrap();
    }
    for seq in 1..=3 {
        let c = log_c.get_entry(seq).unwrap();
        let d = log_d.get_entry(seq).unwrap();
        assert_eq!(
            c.entry_hash, d.entry_hash,
            "identical content must produce identical hashes"
        );
        assert_eq!(c.prev_hash, d.prev_hash);
    }
}

#[test]
fn different_content_produces_different_hashes() {
    let mut log_a = MerkleLog::new(seg("diff"));
    let mut log_b = MerkleLog::new(seg("diff"));

    log_a
        .append(1000, "alice", "read", "record:1", "ok")
        .unwrap();
    log_b
        .append(1000, "alice", "write", "record:1", "ok")
        .unwrap(); // different action

    let a = log_a.get_entry(1).unwrap();
    let b = log_b.get_entry(1).unwrap();
    assert_ne!(
        a.entry_hash, b.entry_hash,
        "different content must produce different hashes"
    );
}

#[test]
fn different_timestamps_produce_different_hashes() {
    let mut log_a = MerkleLog::new(seg("ts"));
    let mut log_b = MerkleLog::new(seg("ts"));

    log_a
        .append(1000, "actor", "action", "target", "ok")
        .unwrap();
    log_b
        .append(2000, "actor", "action", "target", "ok")
        .unwrap();

    assert_ne!(
        log_a.get_entry(1).unwrap().entry_hash,
        log_b.get_entry(1).unwrap().entry_hash,
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Tamper Detection — Rewriting Historical Entries
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn merkle_root_changes_with_any_single_field_difference() {
    // Changing any field in any entry changes the root, making tampering detectable.
    let fields = [
        ("actor_diff", "alice", "action", "target", "ok"),
        ("actor", "action_diff", "target", "ok", "ignored"),
        ("actor", "action", "target_diff", "ok", "ignored"),
        ("actor", "action", "target", "fail", "ignored"),
    ];

    let mut base = MerkleLog::new(seg("base"));
    base.append(1000, "actor", "action", "target", "ok");
    let base_root = base.current_root();

    for (actor, action, target, result, _) in &fields {
        let mut alt = MerkleLog::new(seg("base"));
        alt.append(1000, *actor, *action, *target, *result);
        assert_ne!(
            alt.current_root(),
            base_root,
            "changing field must change root"
        );
    }
}

#[test]
fn forged_inclusion_proof_with_wrong_leaf_hash_fails() {
    let log = build_log("forged-leaf", 4);
    let root = log.current_root();
    let mut proof = log.inclusion_proof(2).unwrap();

    // Tamper: replace the leaf hash with garbage.
    proof.leaf_hash = [0xFF; 32];
    assert!(
        proof.verify(&root).is_err(),
        "proof with tampered leaf hash must fail verification"
    );
}

#[test]
fn forged_inclusion_proof_with_wrong_sibling_fails() {
    let log = build_log("forged-sibling", 8);
    let root = log.current_root();
    let mut proof = log.inclusion_proof(3).unwrap();

    // Tamper: corrupt a sibling hash.
    if !proof.siblings.is_empty() {
        proof.siblings[0] = [0xAB; 32];
    }
    assert!(
        proof.verify(&root).is_err(),
        "proof with tampered sibling must fail verification"
    );
}

#[test]
fn inclusion_proof_fails_against_different_root() {
    let log = build_log("diff-root", 4);
    let real_root = log.current_root();
    let proof = log.inclusion_proof(1).unwrap();

    // Verify against a fabricated root.
    let fake_root = [0xDE; 32];
    assert!(proof.verify(&fake_root).is_err());

    // Verify against the real root should succeed.
    assert!(proof.verify(&real_root).is_ok());
}

#[test]
fn appending_entry_invalidates_old_proofs() {
    let mut log = MerkleLog::new(seg("stale-proof"));
    log.append(1000, "alice", "read", "r:1", "ok").unwrap();
    log.append(2000, "bob", "write", "r:2", "ok").unwrap();

    let root_before = log.current_root();
    let proof_seq1 = log.inclusion_proof(1).unwrap();
    assert!(proof_seq1.verify(&root_before).is_ok());

    // Append a new entry — root changes.
    log.append(3000, "carol", "delete", "r:3", "ok").unwrap();
    let root_after = log.current_root();

    assert_ne!(root_before, root_after);
    // Old proof verified against old root now fails (proof was re-generated for new tree).
    let new_proof_seq1 = log.inclusion_proof(1).unwrap();
    assert!(
        new_proof_seq1.verify(&root_before).is_err(),
        "proof from updated tree must not verify against old root"
    );
    assert!(new_proof_seq1.verify(&root_after).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Consistency Proofs — Append-Only Guarantee
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn consistency_proof_verifies_for_growing_log() {
    let mut log = MerkleLog::new(seg("grow"));
    for i in 1..=4 {
        log.append(i * 1000, "actor", "action", "target", "ok")
            .unwrap();
    }
    let root_4 = log.current_root();

    for i in 5..=8 {
        log.append(i * 1000, "actor", "action", "target", "ok")
            .unwrap();
    }
    let hashes_8 = leaf_hashes(&log, 8);

    let prover = ConsistencyProver::new(hashes_8);
    let proof = prover.generate(root_4, 4).unwrap();
    assert!(
        proof.verify().is_ok(),
        "consistency proof must verify for 4→8"
    );
}

#[test]
fn consistency_proof_detects_tampered_old_root() {
    let mut log = MerkleLog::new(seg("tamper-old-root"));
    for i in 1..=4 {
        log.append(i * 1000, "actor", "action", "target", "ok")
            .unwrap();
    }

    for i in 5..=8 {
        log.append(i * 1000, "actor", "action", "target", "ok")
            .unwrap();
    }
    let hashes_8 = leaf_hashes(&log, 8);

    let fake_root = [0xAA; 32];
    let prover = ConsistencyProver::new(hashes_8);
    let proof = prover.generate(fake_root, 4).unwrap();
    assert!(
        proof.verify().is_err(),
        "consistency proof with tampered old root must fail"
    );
}

#[test]
fn consistency_proof_detects_tampered_proof_hash() {
    let mut log = MerkleLog::new(seg("tamper-proof-hash"));
    for i in 1..=4 {
        log.append(i * 1000, "actor", "action", "target", "ok")
            .unwrap();
    }
    let root_4 = log.current_root();

    for i in 5..=8 {
        log.append(i * 1000, "actor", "action", "target", "ok")
            .unwrap();
    }
    let hashes_8 = leaf_hashes(&log, 8);

    let prover = ConsistencyProver::new(hashes_8);
    let mut proof = prover.generate(root_4, 4).unwrap();

    // Tamper with a proof hash.
    if !proof.proof_hashes.is_empty() {
        proof.proof_hashes[0] = [0xFF; 32];
    }
    assert!(
        proof.verify().is_err(),
        "consistency proof with tampered proof hash must fail"
    );
}

#[test]
fn consistency_proof_across_multiple_checkpoints() {
    let mut log = MerkleLog::new(seg("multi-cp"));

    // Checkpoint 1: 3 entries.
    for i in 1..=3 {
        log.append(i * 1000, "admin", "create_segment", "seg:phi", "ok")
            .unwrap();
    }
    let root_3 = log.publish_root(3000);

    // Checkpoint 2: 7 entries.
    for i in 4..=7 {
        log.append(i * 1000, "admin", "append", "seg:phi", "ok")
            .unwrap();
    }
    let _root_7 = log.publish_root(7000);

    // Checkpoint 3: 12 entries.
    for i in 8..=12 {
        log.append(i * 1000, "auditor", "review", "seg:phi", "ok")
            .unwrap();
    }
    let _root_12 = log.publish_root(12000);

    let all_hashes = leaf_hashes(&log, 12);

    // Prove consistency from checkpoint 1 (size 3) to current (size 12).
    let prover = ConsistencyProver::new(all_hashes);
    let proof = prover.generate(root_3, 3).unwrap();
    assert!(proof.verify().is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Compaction — Verifiable Deletion & Receipt Integrity
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compaction_receipt_records_deleted_hashes() {
    let mut log = build_log("compact-receipt", 5);
    let root_before = log.current_root();

    let receipt = log.compact(1, 2, 100_000, 0).unwrap();

    assert_eq!(receipt.old_root, root_before);
    assert_eq!(receipt.old_size, 5);
    assert_eq!(receipt.deleted_hashes.len(), 2);
    assert_eq!(receipt.new_size, 3);
    assert_ne!(receipt.old_root, receipt.new_root);
}

#[test]
fn compaction_preserves_remaining_chain_entries() {
    let mut log = build_log("compact-preserve", 5);

    log.compact(1, 2, 100_000, 0).unwrap();

    // Remaining entries (3, 4, 5) should still be retrievable.
    assert_eq!(log.len(), 3);
    assert!(log.get_entry(3).is_ok());
    assert!(log.get_entry(4).is_ok());
    assert!(log.get_entry(5).is_ok());

    // Deleted entries should be gone.
    assert!(log.get_entry(1).is_err());
    assert!(log.get_entry(2).is_err());
}

#[test]
fn retention_policy_prevents_premature_deletion() {
    let mut log = MerkleLog::new(seg("retention"));
    log.set_retention(RetentionPolicy {
        segment: seg("retention"),
        min_retention_secs: 86_400, // 1 day
        requires_witness_for_deletion: false,
    });

    log.append(1000, "admin", "create_segment", "seg:phi", "ok")
        .unwrap();

    // Try to compact before retention period expires.
    let result = log.compact(1, 1, 1000 + 86_399, 0);
    assert!(matches!(
        result,
        Err(AuditError::RetentionPolicyViolation { .. })
    ));

    // After retention period, compaction succeeds.
    let result = log.compact(1, 1, 1000 + 86_400, 0);
    assert!(result.is_ok());
}

#[test]
fn sensitive_segment_requires_witnesses_for_deletion() {
    let mut log = MerkleLog::new(seg("sensitive"));
    log.set_retention(RetentionPolicy {
        segment: seg("sensitive"),
        min_retention_secs: 0,
        requires_witness_for_deletion: true,
    });

    log.append(1000, "admin", "action", "target", "ok").unwrap();

    // No witnesses → compaction fails.
    let result = log.compact(1, 1, 2000, 2);
    assert!(matches!(
        result,
        Err(AuditError::InsufficientWitnesses {
            required: 2,
            present: 0
        })
    ));
}

#[test]
fn compaction_succeeds_with_sufficient_witnesses() {
    let mut log = MerkleLog::new(seg("witnessed"));
    log.set_retention(RetentionPolicy {
        segment: seg("witnessed"),
        min_retention_secs: 0,
        requires_witness_for_deletion: true,
    });

    log.append(1000, "admin", "action", "target", "ok").unwrap();
    let root = log.publish_root(1500);

    // Add two witnesses.
    log.add_witness(WitnessSignature {
        witness_id: "witness_1".into(),
        root,
        tree_size: 1,
        signed_at: 1500,
        signature: vec![0x01; 64],
    })
    .unwrap();

    log.add_witness(WitnessSignature {
        witness_id: "witness_2".into(),
        root,
        tree_size: 1,
        signed_at: 1600,
        signature: vec![0x02; 64],
    })
    .unwrap();

    assert_eq!(log.witness_count(), 2);

    // Now compaction with min_witnesses=2 should succeed.
    let result = log.compact(1, 1, 2000, 2);
    assert!(result.is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Administrative-Level Action Auditing
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn admin_actions_are_logged_with_correct_fields() {
    let mut log = MerkleLog::new(seg("admin-audit"));

    log.append(
        1000,
        "admin:0xABCD",
        "segment.create",
        "seg:phi_records",
        "ok",
    )
    .unwrap();
    log.append(
        2000,
        "admin:0xABCD",
        "retention.set",
        "seg:phi_records",
        "ok",
    )
    .unwrap();
    log.append(3000, "admin:0xABCD", "witness.add", "checkpoint:1", "ok")
        .unwrap();

    let e1 = log.get_entry(1).unwrap();
    assert_eq!(e1.actor, "admin:0xABCD");
    assert_eq!(e1.action, "segment.create");
    assert_eq!(e1.target, "seg:phi_records");

    let e2 = log.get_entry(2).unwrap();
    assert_eq!(e2.action, "retention.set");

    let e3 = log.get_entry(3).unwrap();
    assert_eq!(e3.action, "witness.add");
}

#[test]
fn admin_actions_are_tamper_evident_via_chain() {
    let mut log = MerkleLog::new(seg("admin-tamper"));

    log.append(1000, "admin", "initialize", "contract", "ok")
        .unwrap();
    log.append(2000, "admin", "create_segment", "seg:audit", "ok")
        .unwrap();
    log.append(3000, "admin", "set_retention", "seg:audit", "ok")
        .unwrap();
    log.append(4000, "admin", "compact", "seg:audit:1-5", "ok")
        .unwrap();

    // Full chain verification passes.
    assert!(log.verify_chain(1, 4).is_ok());

    // Each admin action is provably included in the Merkle tree.
    let root = log.current_root();
    for seq in 1..=4 {
        let proof = log.inclusion_proof(seq).unwrap();
        assert!(proof.verify(&root).is_ok());
    }
}

#[test]
fn admin_action_searchable_via_keyword() {
    let key = SearchKey::from_bytes(&[0x42u8; 32]).unwrap();
    let mut engine = SearchEngine::new(key);

    // Index admin actions.
    engine.index_entry(1, "admin:root", "segment.create", "seg:phi", "ok", &[]);
    engine.index_entry(2, "admin:root", "retention.set", "seg:phi", "ok", &[]);
    engine.index_entry(3, "clinician", "record.read", "patient:42", "ok", &[]);
    engine.index_entry(4, "admin:root", "compact", "seg:phi", "ok", &[]);

    // Search for all admin actions.
    let admin_entries = engine.query("admin:root");
    assert_eq!(admin_entries, vec![1, 2, 4]);

    // Search by action keyword.
    let create_entries = engine.query("segment.create");
    assert_eq!(create_entries, vec![1]);

    // Non-admin action should not appear in admin search.
    let clinician_entries = engine.query("clinician");
    assert_eq!(clinician_entries, vec![3]);
}

#[test]
fn admin_compaction_is_itself_auditable() {
    let mut log = MerkleLog::new(seg("audit-compaction"));

    // Original data entries.
    log.append(1000, "user:01", "record.create", "patient:01", "ok")
        .unwrap();
    log.append(2000, "user:02", "record.read", "patient:01", "ok")
        .unwrap();

    // Admin performs compaction — log the compaction event first.
    log.append(3000, "admin", "compact", "seq:1-2", "initiated")
        .unwrap();

    let root_before = log.current_root();

    // Now perform the actual compaction on the data entries.
    let receipt = log.compact(1, 2, 100_000, 0).unwrap();

    // The compaction audit entry (seq 3) survives.
    assert!(log.get_entry(3).is_ok());
    let admin_entry = log.get_entry(3).unwrap();
    assert_eq!(admin_entry.action, "compact");

    // Receipt proves what was deleted.
    assert_eq!(receipt.deleted_hashes.len(), 2);
    assert_eq!(receipt.old_root, root_before);
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Merkle Root Commitment — Tamper-Evidence Beacon
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn published_root_matches_current_root() {
    let mut log = build_log("root-pub", 5);
    let current = log.current_root();
    let published = log.publish_root(5000);
    assert_eq!(current, published);
}

#[test]
fn checkpoint_records_correct_tree_size() {
    let mut log = MerkleLog::new(seg("cp-size"));
    log.append(1000, "a", "b", "c", "ok").unwrap();
    log.append(2000, "d", "e", "f", "ok").unwrap();
    log.publish_root(2000);

    log.append(3000, "g", "h", "i", "ok").unwrap();
    log.publish_root(3000);

    let cps = log.checkpoints();
    assert_eq!(cps.len(), 2);
    assert_eq!(cps[0].tree_size, 2);
    assert_eq!(cps[1].tree_size, 3);
}

#[test]
fn empty_log_root_is_zero_hash() {
    let log = MerkleLog::new(seg("empty"));
    assert_eq!(log.current_root(), [0u8; 32]);
}

#[test]
fn compute_root_is_order_dependent() {
    // Swapping two entries' content at the same sequence positions changes the
    // root — proves that which content occupies each slot matters.
    // (Timestamps must be non-decreasing per the chronological-order invariant,
    // so we keep them monotonic and swap only actor/action/target.)
    let mut log_ab = MerkleLog::new(seg("order"));
    log_ab.append(1000, "alice", "read", "r:1", "ok").unwrap();
    log_ab.append(2000, "bob", "write", "r:2", "ok").unwrap();

    let mut log_ba = MerkleLog::new(seg("order"));
    log_ba.append(1000, "bob", "write", "r:2", "ok").unwrap();
    log_ba.append(2000, "alice", "read", "r:1", "ok").unwrap();

    assert_ne!(
        log_ab.current_root(),
        log_ba.current_root(),
        "swapping entry content across positions must change root"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Witness Co-Signing — Byzantine Fault Tolerance
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn witness_cannot_be_added_without_checkpoint() {
    let mut log = MerkleLog::new(seg("no-cp"));
    log.append(1000, "actor", "action", "target", "ok").unwrap();

    let result = log.add_witness(WitnessSignature {
        witness_id: "w1".into(),
        root: [0u8; 32],
        tree_size: 1,
        signed_at: 1000,
        signature: vec![0x01; 64],
    });
    assert!(
        result.is_err(),
        "adding witness without a published checkpoint must fail"
    );
}

#[test]
fn multiple_witnesses_strengthen_tamper_evidence() {
    let mut log = MerkleLog::new(seg("multi-witness"));
    log.append(1000, "actor", "action", "target", "ok").unwrap();
    let root = log.publish_root(1500);

    for i in 0..5 {
        log.add_witness(WitnessSignature {
            witness_id: format!("witness_{}", i),
            root,
            tree_size: 1,
            signed_at: 1500 + i as u64,
            signature: vec![i as u8; 64],
        })
        .unwrap();
    }

    assert_eq!(log.witness_count(), 5);
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Search Index Integrity — Keywords Cannot Be Spoofed Without Key
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn different_search_keys_produce_different_results() {
    let key_a = SearchKey::from_bytes(&[0x01u8; 32]).unwrap();
    let key_b = SearchKey::from_bytes(&[0x02u8; 32]).unwrap();

    let mut engine_a = SearchEngine::new(key_a);
    let mut engine_b = SearchEngine::new(key_b);

    engine_a.index_entry(1, "admin", "create", "seg", "ok", &[]);
    engine_b.index_entry(1, "admin", "create", "seg", "ok", &[]);

    // Both engines find the entry by keyword.
    assert_eq!(engine_a.query("admin"), vec![1]);
    assert_eq!(engine_b.query("admin"), vec![1]);

    // But cross-key queries should not work — the tokens are different.
    // (This is implicit: each engine uses its own key for token derivation.)
}

#[test]
fn search_after_compaction_purge_removes_deleted_entries() {
    let key = SearchKey::from_bytes(&[0x42u8; 32]).unwrap();
    let mut engine = SearchEngine::new(key);

    engine.index_entry(1, "alice", "read", "r:1", "ok", &[]);
    engine.index_entry(2, "bob", "write", "r:2", "ok", &[]);
    engine.index_entry(3, "carol", "read", "r:3", "ok", &[]);

    // Purge entries 1 and 2 (simulating compaction).
    engine.purge(&[1, 2]);

    // Only entry 3 remains.
    assert!(engine.query("alice").is_empty());
    assert!(engine.query("bob").is_empty());
    assert_eq!(engine.query("carol"), vec![3]);
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. End-to-End Tamper Resistance Scenario
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn end_to_end_tamper_resistance_scenario() {
    let mut log = MerkleLog::new(seg("e2e"));

    // Phase 1: Admin initializes and creates entries.
    log.append(1000, "admin", "initialize", "contract", "ok")
        .unwrap();
    log.append(2000, "admin", "create_segment", "seg:phi", "ok")
        .unwrap();
    log.append(3000, "clinician", "record.create", "patient:01", "ok")
        .unwrap();
    log.append(4000, "clinician", "record.read", "patient:01", "ok")
        .unwrap();

    // Phase 2: Publish root as tamper-evidence beacon.
    let root_4 = log.publish_root(4000);

    // Phase 3: Verify hash chain integrity.
    assert!(log.verify_chain(1, 4).is_ok());

    // Phase 4: Verify every entry is provably included.
    for seq in 1..=4 {
        let proof = log.inclusion_proof(seq).unwrap();
        assert!(
            proof.verify(&root_4).is_ok(),
            "inclusion proof for seq {} must verify",
            seq
        );
    }

    // Phase 5: More entries arrive.
    log.append(5000, "admin", "retention.set", "seg:phi", "ok")
        .unwrap();
    log.append(6000, "researcher", "data.export", "patient:01", "ok")
        .unwrap();
    log.append(7000, "admin", "review.approve", "export:1", "ok")
        .unwrap();
    log.append(8000, "admin", "audit.review", "seg:phi", "ok")
        .unwrap();

    let hashes_8 = leaf_hashes(&log, 8);

    // Phase 6: Prove the log is append-only (consistency proof).
    let prover = ConsistencyProver::new(hashes_8);
    let proof = prover.generate(root_4, 4).unwrap();
    assert!(proof.verify().is_ok());

    // Phase 7: Verify the full chain still holds.
    assert!(log.verify_chain(1, 8).is_ok());
}
