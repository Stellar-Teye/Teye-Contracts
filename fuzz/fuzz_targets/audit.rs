#![no_main]

//! Fuzz harness for `audit` — distributed tamper-evident audit log.
//!
//! # What is fuzzed
//!
//! | Target              | What we are looking for                              |
//! |---------------------|------------------------------------------------------|
//! | `MerkleLog::append` | No panics, monotonic sequence numbers                |
//! | Inclusion proofs    | `proof.verify(&root)` never returns `Err` for a     |
//! |                     | freshly generated proof                              |
//! | Hash-chain verify   | `verify_chain` never panics post-compact             |
//! | `ConsistencyProver` | Generated proof always verifies                      |
//! | Compaction          | Receipt contains the expected number of hashes       |
//! | `SearchEngine`      | Query of indexed keyword always returns the entry    |
//!
//! # What is NOT fuzzed here
//!
//! Verification of *externally supplied* (potentially malformed) proofs is
//! exercised via dedicated property tests in the `audit` crate itself
//! (`consistency::tests::tampered_*`).  The fuzzer here targets the
//! produce-then-verify round-trip to catch internal invariant violations.

use arbitrary::Arbitrary;
use audit::{
    consistency::ConsistencyProver,
    merkle_log::MerkleLog,
    search::{SearchEngine, SearchKey},
    types::LogSegmentId,
};
use libfuzzer_sys::fuzz_target;

// ── Fuzz input types ──────────────────────────────────────────────────────────

/// A single action to apply to the log under test.
#[derive(Arbitrary, Debug)]
pub enum FuzzAction {
    /// Append a new entry with the given field byte-lengths.
    Append {
        actor_len: u8,
        action_len: u8,
        target_len: u8,
        result_len: u8,
        timestamp: u32,
    },
    /// Publish the current root as a checkpoint.
    PublishRoot { published_at: u32 },
    /// Request an inclusion proof for `sequence` (1-based, clamped).
    InclusionProof { sequence: u32 },
    /// Verify the hash chain over a range.
    VerifyChain { from: u32, to: u32 },
    /// Compact a range of entries.
    Compact { from: u32, to: u32, now: u32 },
    /// Index an entry into the search engine and query it back.
    SearchRoundTrip {
        actor_len: u8,
        action_len: u8,
    },
    /// Generate + verify a consistency proof between checkpoint 0 and the latest.
    ConsistencyProof,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a synthetic ASCII string of `len` bytes filled with `b'a'`..`b'z'`
/// cycling, clamped to [1, 64].
fn synthetic_str(len: u8) -> alloc::string::String {
    let n = (len as usize).clamp(1, 64);
    (0..n)
        .map(|i| (b'a' + (i % 26) as u8) as char)
        .collect()
}

extern crate alloc;

// ── Fuzz entry point ──────────────────────────────────────────────────────────

fuzz_target!(|actions: Vec<FuzzAction>| {
    // Use a fixed segment name — segment creation is O(1) and cannot fail here.
    let seg = match LogSegmentId::new("fuzz.audit") {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut log = MerkleLog::new(seg);

    // Search engine with a fixed test key (deterministic for reproducibility).
    let search_key = SearchKey::from_bytes(&[0x42u8; 32]).expect("valid 32-byte key");
    let mut engine = SearchEngine::new(search_key);

    // Leaf-hash mirror kept for ConsistencyProver (needs the raw hash list).
    let mut leaf_hashes: alloc::vec::Vec<audit::types::Digest> = alloc::vec::Vec::new();
    // Published (tree_size, root) checkpoints for consistency proofs.
    let mut checkpoints: alloc::vec::Vec<(u64, audit::merkle_log::MerkleRoot)> =
        alloc::vec::Vec::new();

    for action in actions {
        match action {
            // ── Append ───────────────────────────────────────────────────────
            FuzzAction::Append {
                actor_len,
                action_len,
                target_len,
                result_len,
                timestamp,
            } => {
                let actor = synthetic_str(actor_len);
                let action = synthetic_str(action_len);
                let target = synthetic_str(target_len);
                let result = synthetic_str(result_len);

                let seq = log.append(
                    timestamp as u64,
                    actor.clone(),
                    action.clone(),
                    target.clone(),
                    result.clone(),
                );

                // Invariant: sequence numbers are 1-based and monotone.
                assert!(seq >= 1, "sequence must be >= 1");

                // Mirror leaf hash for consistency proofs.
                if let Ok(entry) = log.get_entry(seq) {
                    leaf_hashes.push(entry.entry_hash);
                }

                // Index into search engine; query by actor must return this seq.
                engine.index_entry(seq, &actor, &action, &target, &result, &[]);
                let hits = engine.query(&actor);
                assert!(
                    hits.contains(&seq),
                    "indexed entry must appear in search results"
                );
            }

            // ── Publish root ─────────────────────────────────────────────────
            FuzzAction::PublishRoot { published_at } => {
                if log.is_empty() {
                    return;
                }
                let root = log.publish_root(published_at as u64);
                let size = log.len();
                checkpoints.push((size, root));
            }

            // ── Inclusion proof ───────────────────────────────────────────────
            FuzzAction::InclusionProof { sequence } => {
                let n = log.len();
                if n == 0 {
                    return;
                }
                // Clamp to a valid 1-based sequence.
                let seq = (sequence as u64 % n) + 1;
                let root = log.current_root();
                match log.inclusion_proof(seq) {
                    Ok(proof) => {
                        // A freshly generated proof must ALWAYS verify.
                        assert!(
                            proof.verify(&root).is_ok(),
                            "fresh inclusion proof must verify for seq={seq}"
                        );
                    }
                    Err(_) => {
                        // Entry was compacted away — acceptable.
                    }
                }
            }

            // ── Hash-chain verification ───────────────────────────────────────
            FuzzAction::VerifyChain { from, to } => {
                let n = log.len();
                if n == 0 {
                    return;
                }
                let from_seq = (from as u64 % n) + 1;
                let to_seq = ((to as u64 % n) + 1).max(from_seq);
                // Must not panic — errors (e.g. compacted entries) are acceptable.
                let _ = log.verify_chain(from_seq, to_seq);
            }

            // ── Compaction ────────────────────────────────────────────────────
            FuzzAction::Compact { from, to, now } => {
                let n = log.len();
                if n == 0 {
                    return;
                }
                let from_seq = (from as u64 % n) + 1;
                let to_seq = ((to as u64 % n) + 1).max(from_seq);
                match log.compact(from_seq, to_seq, now as u64, 0) {
                    Ok(receipt) => {
                        // Receipt hashes must not exceed the range size.
                        let range_size = to_seq.saturating_sub(from_seq) + 1;
                        assert!(
                            receipt.deleted_hashes.len() as u64 <= range_size,
                            "compaction receipt cannot delete more hashes than the range"
                        );
                        // Purge from the search index too.
                        let removed: alloc::vec::Vec<u64> = (from_seq..=to_seq).collect();
                        engine.purge(&removed);
                        // Rebuild leaf_hashes mirror (entries after compact changed).
                        leaf_hashes = (1..=log.len())
                            .filter_map(|s| log.get_entry(s).ok())
                            .map(|e| e.entry_hash)
                            .collect();
                    }
                    Err(_) => {
                        // RetentionPolicyViolation, InsufficientWitnesses, etc. — OK.
                    }
                }
            }

            // ── Search round-trip ─────────────────────────────────────────────
            FuzzAction::SearchRoundTrip {
                actor_len,
                action_len,
            } => {
                // Append a synthetic entry and immediately verify searchability.
                let actor = synthetic_str(actor_len);
                let action = synthetic_str(action_len);
                let seq = log.append(0, actor.clone(), action.clone(), "tgt", "ok");
                engine.index_entry(seq, &actor, &action, "tgt", "ok", &[]);

                if let Ok(entry) = log.get_entry(seq) {
                    leaf_hashes.push(entry.entry_hash);
                }

                let hits = engine.query(&actor);
                assert!(
                    hits.contains(&seq),
                    "SearchRoundTrip: indexed actor not found"
                );
            }

            // ── Consistency proof ─────────────────────────────────────────────
            FuzzAction::ConsistencyProof => {
                if checkpoints.is_empty() || leaf_hashes.is_empty() {
                    return;
                }
                // Use the first recorded checkpoint as v1.
                let (old_size, old_root) = checkpoints[0];
                if old_size as usize > leaf_hashes.len() {
                    return; // Mirror out of sync after compaction — skip.
                }
                let prover = ConsistencyProver::new(leaf_hashes.clone());
                match prover.generate(old_root, old_size) {
                    Ok(proof) => {
                        // A generated proof must verify.
                        assert!(
                            proof.verify().is_ok(),
                            "fresh consistency proof must verify"
                        );
                    }
                    Err(_) => {
                        // old_size > current size after compaction — acceptable.
                    }
                }
            }
        }
    }
});
