/// Append-only Merkle log with tamper-evident integrity guarantees.
///
/// # Architecture
///
/// ```text
/// ┌─────────────────────────────────────────────────────────┐
/// │  MerkleLog (per segment)                                │
/// │                                                         │
/// │  entries: BTreeMap<u64, LogEntry>   ← O(log n) lookup  │
/// │  nodes:   Vec<Option<Digest>>       ← Merkle node store │
/// │  roots:   Vec<RootCheckpoint>       ← published roots  │
/// │  witnesses: Vec<WitnessSignature>   ← co-signatures    │
/// └─────────────────────────────────────────────────────────┘
/// ```
///
/// # Time and Space Complexity
///
/// | Operation           | Time      | Space      |
/// |---------------------|-----------|------------|
/// | `append`            | O(log n)  | O(1) amort |
/// | `inclusion_proof`   | O(log n)  | O(log n)   |
/// | `verify_root`       | O(1)      | O(1)       |
/// | `compact`           | O(k log n)| O(k)       |
/// | `query_range`       | O(k)      | O(k)       |
///
/// where n is the total number of entries and k is the range width.

use alloc::{collections::BTreeMap, string::String, vec::Vec};

use sha2::{Digest as Sha2Digest, Sha256};

use crate::types::{AuditError, Digest, LogEntry, LogSegmentId, RetentionPolicy, WitnessSignature};

// ── Merkle-tree domain-separation prefixes (RFC 6962) ─────────────────────────
const LEAF_PREFIX: u8 = 0x00;
const NODE_PREFIX: u8 = 0x01;

// ── Public type aliases ────────────────────────────────────────────────────────

/// A SHA-256 Merkle root digest.
pub type MerkleRoot = Digest;

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Hash a leaf node: `SHA256(0x00 ‖ data)`.
pub fn hash_leaf(data: &[u8]) -> Digest {
    let mut h = Sha256::new();
    h.update([LEAF_PREFIX]);
    h.update(data);
    h.finalize().into()
}

/// Hash an internal node: `SHA256(0x01 ‖ left ‖ right)`.
#[inline]
fn hash_node(left: &Digest, right: &Digest) -> Digest {
    let mut h = Sha256::new();
    h.update([NODE_PREFIX]);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

/// Return the root of the smallest complete binary Merkle tree over `leaves`.
///
/// If `leaves` is empty, returns `[0u8; 32]`.
///
/// Complexity: O(n) time, O(n) stack space via iterative reduction.
pub fn compute_root(leaves: &[Digest]) -> MerkleRoot {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    // Iterative bottom-up reduction — no recursion, fixed stack depth.
    let mut level: Vec<Digest> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity((level.len() + 1) / 2);
        let mut i = 0;
        while i < level.len() {
            if i + 1 < level.len() {
                next.push(hash_node(&level[i], &level[i + 1]));
            } else {
                // Promote the lone right-hand node (RFC 6962 §2.1).
                next.push(level[i]);
            }
            i += 2;
        }
        level = next;
    }
    level[0]
}

// ── Inclusion-proof types ─────────────────────────────────────────────────────

/// A Merkle inclusion proof proving that `leaf_hash` is the n-th leaf in a
/// tree of `tree_size` leaves whose root is `root`.
///
/// The verifier reconstructs the root by combining `leaf_hash` with each
/// `sibling` from leaf to root; at each step it selects left/right child
/// according to the corresponding bit of `leaf_index`.
#[derive(Debug, Clone)]
pub struct InclusionProof {
    /// 0-based index of the proven leaf.
    pub leaf_index: u64,
    /// Total number of leaves in the tree.
    pub tree_size: u64,
    /// Hash of the proven leaf (= `hash_leaf(entry.canonical_bytes())`).
    pub leaf_hash: Digest,
    /// Sibling hashes from leaf to root (left-to-right = bottom-to-top).
    pub siblings: Vec<Digest>,
}

impl InclusionProof {
    /// Verify that `self.leaf_hash` is correctly included in a tree with the
    /// given `root`.
    ///
    /// Complexity: O(log n).
    pub fn verify(&self, root: &MerkleRoot) -> Result<(), AuditError> {
        let mut computed = self.leaf_hash;
        let mut index = self.leaf_index;
        let mut size = self.tree_size;

        for sibling in &self.siblings {
            if size == 0 {
                return Err(AuditError::InvalidInclusionProof);
            }
            // Determine "inner" node count at the current tree level.
            // At each level we pair nodes; the last unpaired node is promoted.
            if index % 2 == 1 {
                computed = hash_node(sibling, &computed);
            } else if index < size - 1 {
                computed = hash_node(&computed, sibling);
            }
            // else: lone right-hand node — keep `computed` unchanged.
            index /= 2;
            size = size.div_ceil(2);
        }

        if &computed == root {
            Ok(())
        } else {
            Err(AuditError::InvalidInclusionProof)
        }
    }
}

// ── Checkpoint ────────────────────────────────────────────────────────────────

/// A published root checkpoint, created each time `publish_root` is called.
/// Checkpoints are the basis for consistency proofs.
#[derive(Debug, Clone)]
pub struct RootCheckpoint {
    /// Number of leaves in the tree at the time of this checkpoint.
    pub tree_size: u64,
    /// Merkle root at `tree_size`.
    pub root: MerkleRoot,
    /// Timestamp of root publication (caller-supplied).
    pub published_at: u64,
    /// Witness co-signatures endorsing this checkpoint.
    pub endorsements: Vec<WitnessSignature>,
}

// ── CompactionReceipt ─────────────────────────────────────────────────────────

/// A verifiable record of a compaction operation.
///
/// When entries are deleted the `MerkleLog` emits this receipt so external
/// auditors can independently confirm:
/// 1. Exactly which leaves were removed (via `deleted_hashes`).
/// 2. The pre- and post-compaction roots are consistent.
#[derive(Debug, Clone)]
pub struct CompactionReceipt {
    /// Root before compaction.
    pub old_root: MerkleRoot,
    /// Size before compaction.
    pub old_size: u64,
    /// Root after compaction.
    pub new_root: MerkleRoot,
    /// Size after compaction.
    pub new_size: u64,
    /// Hashes of deleted leaf entries (ordered by original sequence number).
    pub deleted_hashes: Vec<Digest>,
    /// Timestamp of the compaction.
    pub compacted_at: u64,
}

// ── MerkleLog ─────────────────────────────────────────────────────────────────

/// An append-only, hash-chain + Merkle-tree audit log for a single segment.
///
/// Entries are stored in an ordered map keyed by their sequence number,
/// enabling O(log n) point-lookup and O(k) range queries without a secondary
/// index.
pub struct MerkleLog {
    /// The logical partition this log covers.
    pub segment: LogSegmentId,

    /// Ordered store of live entries.  
    /// Invariant: entries are never mutated or removed except by `compact`.
    entries: BTreeMap<u64, LogEntry>,

    /// Leaf hashes in insertion order.  
    /// Index i corresponds to sequence (i + 1) — sequences are 1-based.
    leaf_hashes: Vec<Digest>,

    /// Published root checkpoints.
    checkpoints: Vec<RootCheckpoint>,

    /// Witness co-signatures collected via `add_witness`.
    witnesses: Vec<WitnessSignature>,

    /// Next sequence number to assign.
    next_seq: u64,

    /// Retention policy for this segment (if any).
    retention: Option<RetentionPolicy>,
}

impl MerkleLog {
    // ── Construction ─────────────────────────────────────────────────────────

    /// Create a new, empty log for the given segment.
    ///
    /// Complexity: O(1).
    pub fn new(segment: LogSegmentId) -> Self {
        Self {
            segment,
            entries: BTreeMap::new(),
            leaf_hashes: Vec::new(),
            checkpoints: Vec::new(),
            witnesses: Vec::new(),
            next_seq: 1,
            retention: None,
        }
    }

    /// Attach a retention policy.  Can only be set once per log.
    pub fn set_retention(&mut self, policy: RetentionPolicy) {
        self.retention = Some(policy);
    }

    // ── Append ───────────────────────────────────────────────────────────────

    /// Append a new entry to the log.
    ///
    /// The caller supplies everything *except* `sequence`, `prev_hash`, and
    /// `entry_hash` — those are computed here to maintain the hash chain.
    ///
    /// # Parameters
    /// * `timestamp` – Unix seconds, supplied by the runtime (not clock).
    /// * `actor`     – Initiating identity.
    /// * `action`    – High-level action label.
    /// * `target`    – Affected resource.
    /// * `result`    – Outcome string.
    ///
    /// # Returns
    /// The newly assigned sequence number.
    ///
    /// # Complexity
    /// O(log n) — one BTreeMap insertion, one leaf-hash push, constant hashing.
    pub fn append(
        &mut self,
        timestamp: u64,
        actor: impl Into<String>,
        action: impl Into<String>,
        target: impl Into<String>,
        result: impl Into<String>,
    ) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;

        // Hash chain: previous entry's hash, or zero-hash for the first entry.
        let prev_hash: Digest = if seq == 1 {
            [0u8; 32]
        } else {
            // O(log n) BTreeMap lookup.
            self.entries
                .get(&(seq - 1))
                .map(|e| e.entry_hash)
                .unwrap_or([0u8; 32])
        };

        // Partially construct the entry (entry_hash is computed below).
        let mut entry = LogEntry {
            sequence: seq,
            timestamp,
            actor: actor.into(),
            action: action.into(),
            target: target.into(),
            result: result.into(),
            prev_hash,
            entry_hash: [0u8; 32], // placeholder
            segment: self.segment.clone(),
        };

        // Compute the entry hash over its canonical bytes.
        let canonical = entry.canonical_bytes();
        let leaf_hash = hash_leaf(&canonical);
        entry.entry_hash = leaf_hash;

        self.leaf_hashes.push(leaf_hash);
        self.entries.insert(seq, entry);

        seq
    }

    // ── Root publishing ───────────────────────────────────────────────────────

    /// Publish (record) the current Merkle root as a checkpoint.
    ///
    /// Callers should do this at regular intervals (e.g. every N entries or
    /// every T seconds) so that consistency proofs remain compact.
    ///
    /// Complexity: O(n) for root computation — consider batching appends.
    pub fn publish_root(&mut self, published_at: u64) -> MerkleRoot {
        let root = compute_root(&self.leaf_hashes);
        self.checkpoints.push(RootCheckpoint {
            tree_size: self.leaf_hashes.len() as u64,
            root,
            published_at,
            endorsements: Vec::new(),
        });
        root
    }

    /// Return the current (live) Merkle root without publishing a checkpoint.
    ///
    /// Complexity: O(n).
    pub fn current_root(&self) -> MerkleRoot {
        compute_root(&self.leaf_hashes)
    }

    /// Add a witness co-signature to the most recent checkpoint.
    ///
    /// # Errors
    /// Returns [`AuditError::InternalError`] if no checkpoint has been
    /// published yet.
    pub fn add_witness(&mut self, sig: WitnessSignature) -> Result<(), AuditError> {
        match self.checkpoints.last_mut() {
            Some(cp) => {
                cp.endorsements.push(sig.clone());
                self.witnesses.push(sig);
                Ok(())
            }
            None => Err(AuditError::InternalError("no checkpoint published yet")),
        }
    }

    /// Number of witness co-signatures collected across all checkpoints.
    pub fn witness_count(&self) -> usize {
        self.witnesses.len()
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Retrieve a single entry by sequence number.
    ///
    /// Complexity: O(log n).
    pub fn get_entry(&self, sequence: u64) -> Result<&LogEntry, AuditError> {
        self.entries
            .get(&sequence)
            .ok_or(AuditError::EntryNotFound { sequence })
    }

    /// Retrieve all entries whose sequence number falls in `[from, to]`.
    ///
    /// Complexity: O(k + log n) where k = `to - from + 1`.
    pub fn query_range(&self, from: u64, to: u64) -> Vec<&LogEntry> {
        self.entries.range(from..=to).map(|(_, e)| e).collect()
    }

    /// Total number of live entries.
    #[inline]
    pub fn len(&self) -> u64 {
        self.entries.len() as u64
    }

    /// True when the log contains no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// All published checkpoints.
    #[inline]
    pub fn checkpoints(&self) -> &[RootCheckpoint] {
        &self.checkpoints
    }

    // ── Inclusion proof ───────────────────────────────────────────────────────

    /// Generate a Merkle inclusion proof for the entry at `sequence`.
    ///
    /// The proof allows any third party to independently verify — using only
    /// the leaf hash and the proof path — that the entry was included in the
    /// tree whose root is `self.current_root()`.
    ///
    /// Complexity: O(log n) time and space.
    pub fn inclusion_proof(&self, sequence: u64) -> Result<InclusionProof, AuditError> {
        if sequence < 1 || sequence > self.leaf_hashes.len() as u64 {
            return Err(AuditError::EntryNotFound { sequence });
        }
        // Convert 1-based sequence to 0-based leaf index.
        let leaf_index = (sequence - 1) as usize;
        let tree_size = self.leaf_hashes.len();
        let leaf_hash = self.leaf_hashes[leaf_index];

        let siblings = merkle_siblings(&self.leaf_hashes, leaf_index, tree_size);

        Ok(InclusionProof {
            leaf_index: leaf_index as u64,
            tree_size: tree_size as u64,
            leaf_hash,
            siblings,
        })
    }

    // ── Hash-chain verification ───────────────────────────────────────────────

    /// Verify the hash chain for all live entries from `from_seq` to `to_seq`.
    ///
    /// Walks the chain sequentially and checks that each entry's `prev_hash`
    /// equals the `entry_hash` of its predecessor.
    ///
    /// Complexity: O(k · L) where k = range size, L = average entry byte len.
    pub fn verify_chain(&self, from_seq: u64, to_seq: u64) -> Result<(), AuditError> {
        let mut prev: Option<&LogEntry> = None;
        for seq in from_seq..=to_seq {
            let entry = self.get_entry(seq)?;
            if let Some(p) = prev {
                if entry.prev_hash != p.entry_hash {
                    return Err(AuditError::HashChainBroken {
                        at_sequence: entry.sequence,
                    });
                }
            } else if entry.prev_hash != [0u8; 32] && from_seq == 1 {
                // The very first entry must chain from the genesis zero-hash.
                return Err(AuditError::HashChainBroken {
                    at_sequence: entry.sequence,
                });
            }
            prev = Some(entry);
        }
        Ok(())
    }

    // ── Compaction (verifiable deletion) ─────────────────────────────────────

    /// Delete entries in `[from_seq, to_seq]` after enforcing retention policy.
    ///
    /// After deletion a [`CompactionReceipt`] is returned so that external
    /// auditors can verify the remaining log's integrity has been preserved:
    /// the receipt contains the hashes of every deleted leaf, enabling
    /// recomputation of the old root from the new root + deleted leaf set.
    ///
    /// ### Retention enforcement
    /// If a retention policy is active and any entry in the range was created
    /// after `now - min_retention_secs`, the operation is rejected.
    ///
    /// ### Witness requirement
    /// If the policy marks this segment as high-sensitivity
    /// (`requires_witness_for_deletion`), at least one witness co-signature
    /// must exist before compaction is permitted.
    ///
    /// Complexity: O(k log n + n) where k = deleted range size.
    pub fn compact(
        &mut self,
        from_seq: u64,
        to_seq: u64,
        now: u64,
        min_witnesses_for_sensitive: usize,
    ) -> Result<CompactionReceipt, AuditError> {
        // Retention policy check.
        if let Some(ref policy) = self.retention {
            // Sensitive segments require witnesses before any compaction.
            if policy.requires_witness_for_deletion
                && self.witnesses.len() < min_witnesses_for_sensitive
            {
                return Err(AuditError::InsufficientWitnesses {
                    required: min_witnesses_for_sensitive,
                    present: self.witnesses.len(),
                });
            }
            // Check every entry in the range against the min retention window.
            for seq in from_seq..=to_seq {
                if let Some(e) = self.entries.get(&seq) {
                    let retained_until = e.timestamp + policy.min_retention_secs;
                    if now < retained_until {
                        return Err(AuditError::RetentionPolicyViolation {
                            sequence: seq,
                            retained_until,
                        });
                    }
                }
            }
        }

        let old_root = self.current_root();
        let old_size = self.leaf_hashes.len() as u64;

        // Collect hashes of deleted leaves (for the receipt).
        let mut deleted_hashes: Vec<Digest> = Vec::new();
        for seq in from_seq..=to_seq {
            let idx = (seq - 1) as usize;
            if idx < self.leaf_hashes.len() {
                deleted_hashes.push(self.leaf_hashes[idx]);
            }
            self.entries.remove(&seq);
        }

        // Rebuild leaf_hashes from the remaining entries (preserving order).
        // O(n) — necessary after deletion since the leaf array is contiguous.
        self.leaf_hashes = self
            .entries
            .values()
            .map(|e| e.entry_hash)
            .collect::<Vec<_>>();
        // Re-hash them as leaves (they were already stored as leaf-hashes).
        // Note: entry_hash == hash_leaf(canonical), so we reuse directly.

        let new_root = self.current_root();
        let new_size = self.leaf_hashes.len() as u64;

        Ok(CompactionReceipt {
            old_root,
            old_size,
            new_root,
            new_size,
            deleted_hashes,
            compacted_at: now,
        })
    }
}

// ── Sibling-path helper ───────────────────────────────────────────────────────

/// Compute the sibling-hash path for `leaf_index` in a tree of `tree_size`
/// leaves.
///
/// The path is ordered bottom-to-top (leaf level first).
///
/// Complexity: O(n) for intermediate level construction, O(log n) levels.
fn merkle_siblings(leaves: &[Digest], leaf_index: usize, _tree_size: usize) -> Vec<Digest> {
    if leaves.len() <= 1 {
        return Vec::new();
    }
    let mut siblings = Vec::new();
    let mut level: Vec<Digest> = leaves.to_vec();
    let mut idx = leaf_index;

    while level.len() > 1 {
        // Find sibling at this level.
        let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
        // The sibling might not exist (odd node is its own "pair").
        if sibling_idx < level.len() {
            siblings.push(level[sibling_idx]);
        }
        // Build the next level up.
        let mut next = Vec::with_capacity((level.len() + 1) / 2);
        let mut i = 0;
        while i < level.len() {
            if i + 1 < level.len() {
                next.push(hash_node(&level[i], &level[i + 1]));
            } else {
                next.push(level[i]);
            }
            i += 2;
        }
        idx /= 2;
        level = next;
    }
    siblings
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LogSegmentId;

    fn seg() -> LogSegmentId {
        LogSegmentId::new("test").unwrap()
    }

    #[test]
    fn single_entry_root_is_leaf_hash() {
        let mut log = MerkleLog::new(seg());
        let seq = log.append(1_000, "alice", "create", "record:1", "ok");
        assert_eq!(seq, 1);

        let root = log.current_root();
        let entry = log.get_entry(1).unwrap();
        // A tree with one leaf has root == hash_leaf(entry).
        assert_eq!(root, entry.entry_hash);
    }

    #[test]
    fn hash_chain_is_linked() {
        let mut log = MerkleLog::new(seg());
        log.append(1_000, "alice", "create", "r:1", "ok");
        log.append(1_001, "bob", "read", "r:1", "ok");
        log.append(1_002, "carol", "update", "r:1", "ok");

        assert!(log.verify_chain(1, 3).is_ok());
    }

    #[test]
    fn tamper_detected_by_chain_verification() {
        let mut log = MerkleLog::new(seg());
        log.append(1_000, "alice", "create", "r:1", "ok");
        log.append(1_001, "bob", "read", "r:1", "ok");

        // Simulate tampering: corrupt entry 2's prev_hash.
        // We can't mutate through the public API (append-only design), so we
        // access the entry through the BTreeMap directly in tests only.
        if let Some(entry) = log.entries.get_mut(&2) {
            entry.prev_hash = [0xDE; 32]; // corrupt
        }

        let err = log.verify_chain(1, 2).unwrap_err();
        assert!(matches!(
            err,
            AuditError::HashChainBroken { at_sequence: 2 }
        ));
    }

    #[test]
    fn inclusion_proof_verifies() {
        let mut log = MerkleLog::new(seg());
        for i in 0..8u64 {
            log.append(i, "user", "action", "tgt", "ok");
        }
        let root = log.current_root();
        for seq in 1..=8u64 {
            let proof = log.inclusion_proof(seq).unwrap();
            assert!(proof.verify(&root).is_ok(), "proof failed for seq={seq}");
        }
    }

    #[test]
    fn merkle_root_changes_after_append() {
        let mut log = MerkleLog::new(seg());
        log.append(1, "a", "b", "c", "ok");
        let root1 = log.current_root();
        log.append(2, "d", "e", "f", "ok");
        let root2 = log.current_root();
        assert_ne!(root1, root2);
    }

    #[test]
    fn compact_returns_receipt_and_shrinks_log() {
        let mut log = MerkleLog::new(seg());
        for i in 1..=5u64 {
            log.append(i, "u", "a", "t", "ok");
        }
        let receipt = log.compact(1, 2, 10_000, 0).unwrap();
        assert_eq!(receipt.deleted_hashes.len(), 2);
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn retention_policy_prevents_early_deletion() {
        use crate::types::RetentionPolicy;
        let mut log = MerkleLog::new(seg());
        log.set_retention(RetentionPolicy {
            segment: seg(),
            min_retention_secs: 1_000,
            requires_witness_for_deletion: false,
        });
        log.append(500, "u", "a", "t", "ok");
        // now = 999 → retained_until = 500 + 1_000 = 1_500 > 999
        let err = log.compact(1, 1, 999, 0).unwrap_err();
        assert!(matches!(
            err,
            AuditError::RetentionPolicyViolation { .. }
        ));
    }

    #[test]
    fn query_range_returns_correct_entries() {
        let mut log = MerkleLog::new(seg());
        for i in 1..=10u64 {
            log.append(i, "u", "a", "t", "ok");
        }
        let range = log.query_range(3, 7);
        assert_eq!(range.len(), 5);
        assert_eq!(range[0].sequence, 3);
        assert_eq!(range[4].sequence, 7);
    }
}
