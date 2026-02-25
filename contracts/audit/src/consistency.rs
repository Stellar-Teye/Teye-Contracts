/// Merkle consistency proofs — proving the log is truly append-only.
///
/// # What is a consistency proof?
///
/// Given:
/// * `root_v1` — the Merkle root published when the log had `size_v1` entries.
/// * `root_v2` — the Merkle root published when the log had `size_v2` entries
///               (`size_v2 ≥ size_v1`).
///
/// A **consistency proof** is a minimal set of Merkle hashes that lets a
/// verifier confirm:
/// 1. The first `size_v1` leaves of the `size_v2`-tree are identical to the
///    leaves of the `size_v1`-tree (no retroactive modification).
/// 2. `root_v2` is the Merkle root of a tree that *extends* `root_v1`.
///
/// # Algorithm
///
/// We follow RFC 6962 §2.1.2.  The proof is built by recursively traversing
/// the binary tree and collecting the sibling hashes needed to re-derive both
/// roots.  A key invariant: a node hash for a **complete** subtree that is
/// shared between the old and new tree is emitted once and used for both
/// derivations.
///
/// Proof size is O(log n); verification is O(log n).
///
/// # Time and Space Complexity
///
/// | Operation         | Time      | Space     |
/// |-------------------|-----------|-----------| 
/// | `generate`        | O(n)      | O(log n)  |
/// | `verify`          | O(log n)  | O(log n)  |

use alloc::vec::Vec;

use sha2::{Digest as Sha2Digest, Sha256};

use crate::{
    merkle_log::{compute_root, MerkleRoot},
    types::{AuditError, Digest},
};

// ── Domain-separation prefix (RFC 6962) ───────────────────────────────────────
const NODE_PREFIX: u8 = 0x01;

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Combine two sub-tree hashes: `SHA256(0x01 ‖ left ‖ right)`.
#[inline]
fn hash_node(left: &Digest, right: &Digest) -> Digest {
    let mut h = Sha256::new();
    h.update([NODE_PREFIX]);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

/// Compute the Merkle root of `leaves[begin..end]` using the same algorithm as
/// `compute_root` in `merkle_log.rs` (iterative, RFC 6962 promotion for odd
/// nodes).
///
/// Complexity: O(end - begin).
fn sub_root(leaves: &[Digest], begin: usize, end: usize) -> Digest {
    let n = end - begin;
    if n == 0 {
        return [0u8; 32];
    }
    // Delegate to compute_root so both modules share the same tree shape.
    compute_root(&leaves[begin..end])
}

/// Return the largest power of two strictly less than `n` (`n ≥ 2`).
#[inline]
fn largest_power_of_two_less_than(n: usize) -> usize {
    debug_assert!(n >= 2);
    1 << (usize::BITS - 1 - (n - 1).leading_zeros()) as usize
}

// ── ConsistencyProof ─────────────────────────────────────────────────────────

/// A Merkle consistency proof between two log snapshots.
///
/// Produced by [`ConsistencyProver::generate`] and consumed by
/// [`ConsistencyProof::verify`].
#[derive(Debug, Clone)]
pub struct ConsistencyProof {
    /// Number of leaves in the *older* snapshot.
    pub size_v1: u64,
    /// Number of leaves in the *newer* snapshot.
    pub size_v2: u64,
    /// Merkle root of the older snapshot.
    pub root_v1: MerkleRoot,
    /// Merkle root of the newer snapshot.
    pub root_v2: MerkleRoot,
    /// Proof hashes (ordered as produced by the RFC 6962 algorithm).
    ///
    /// Length is at most 2·⌊log₂(size_v2)⌋ + 2.
    pub proof_hashes: Vec<Digest>,
}

impl ConsistencyProof {
    /// Verify this consistency proof.
    ///
    /// Reconstructs both `root_v1` and `root_v2` from `proof_hashes` and
    /// checks them against the stored values.
    ///
    /// # Errors
    ///
    /// * [`AuditError::InvalidConsistencyProof`] — proof hashes are malformed.
    /// * [`AuditError::RootMismatch`] — a reconstructed root does not match.
    ///
    /// # Complexity  O(log n).
    pub fn verify(&self) -> Result<(), AuditError> {
        if self.size_v1 == 0 {
            return Ok(());
        }
        if self.size_v1 == self.size_v2 {
            return if self.root_v1 == self.root_v2 {
                Ok(())
            } else {
                Err(AuditError::RootMismatch)
            };
        }
        if self.proof_hashes.is_empty() {
            return Err(AuditError::InvalidConsistencyProof);
        }

        let mut idx = 0usize;
        let (r1, r2) = verify_inner(
            &self.proof_hashes,
            &mut idx,
            self.size_v1 as usize,
            self.size_v2 as usize,
        )
        .ok_or(AuditError::InvalidConsistencyProof)?;

        // All proof hashes must be consumed.
        if idx != self.proof_hashes.len() {
            return Err(AuditError::InvalidConsistencyProof);
        }

        if r1 != self.root_v1 {
            return Err(AuditError::RootMismatch);
        }
        if r2 != self.root_v2 {
            return Err(AuditError::InvalidConsistencyProof);
        }
        Ok(())
    }
}

// ── Verifier ─────────────────────────────────────────────────────────────────

/// Recursive verifier.
///
/// Covers the sub-problem:
/// * The old tree has `n1` leaves.
/// * The new tree has `n2` leaves (`n2 >= n1 >= 1`).
/// * `at_root` is true iff this call is at a split point (not yet combined
///   with its complement) — needed to correctly handle the shared-node case.
///
/// Returns `(old_subtree_root, new_subtree_root)` or `None` on malformed proof.
fn verify_inner(
    proof: &[Digest],
    idx: &mut usize,
    n1: usize,
    n2: usize,
) -> Option<(Digest, Digest)> {
    // Base: the two sub-trees are identical and complete — emit one shared hash.
    if n1 == n2 {
        let h = *proof.get(*idx)?;
        *idx += 1;
        return Some((h, h));
    }

    if n2 == 1 {
        // n1 must equal n2 == 1 (handled above); reaching here is malformed.
        return None;
    }

    let split = largest_power_of_two_less_than(n2);

    if n1 <= split {
        // Old tree fits entirely in the left sub-tree.
        let (old_left, new_left) = verify_inner(proof, idx, n1, split)?;
        // Consume the right sub-tree (entirely new).
        let right = *proof.get(*idx)?;
        *idx += 1;
        let new_root = hash_node(&new_left, &right);
        // old_root: if the old tree exactly covers the left half (n1 == split),
        // its root is new_left combined with right. But wait — the old tree of
        // size `split` IS the full left subtree, so old_root = hash_node(new_left, right)?
        // NO: the old tree of size `n1` has its own root independently of the
        // right branch of the new tree. When n1 < split, old_root = old_left.
        // When n1 == split, the old tree IS the left branch, so old_root = old_left
        // (which equals new_left since n1==split → shared subtree → old_left == new_left).
        // The right branch was NOT part of the old tree at all.
        let old_root = old_left;
        Some((old_root, new_root))
    } else {
        // Old tree extends into the right sub-tree.
        // Left sub-tree is complete and shared — consume one proof hash.
        let left = *proof.get(*idx)?;
        *idx += 1;
        let (old_right, new_right) = verify_inner(proof, idx, n1 - split, n2 - split)?;
        let old_root = hash_node(&left, &old_right);
        let new_root = hash_node(&left, &new_right);
        Some((old_root, new_root))
    }
}

// ── Prover ───────────────────────────────────────────────────────────────────

/// Collect the proof hashes for a consistency proof between `old_size` and
/// `new_size` leaves, in the order expected by `verify_inner`.
fn collect_proof_hashes(leaves: &[Digest], old_size: usize, new_size: usize) -> Vec<Digest> {
    debug_assert!(old_size >= 1 && old_size <= new_size);
    let mut proof: Vec<Digest> = Vec::new();
    collect_inner(leaves, old_size, new_size, &mut proof);
    proof
}

/// Recursive helper — mirrors `verify_inner` exactly.
fn collect_inner(
    leaves: &[Digest],
    n1: usize,
    n2: usize,
    proof: &mut Vec<Digest>,
) {
    if n1 == n2 {
        // Shared complete sub-tree: emit its root as one hash.
        proof.push(sub_root(leaves, 0, n2));
        return;
    }

    if n2 == 1 {
        return; // Unreachable for valid inputs.
    }

    let split = largest_power_of_two_less_than(n2);

    if n1 <= split {
        // Recurse left (old tree fits in left sub-tree).
        collect_inner(&leaves[..split], n1, split, proof);
        // Emit right sub-tree root as proof.
        proof.push(sub_root(leaves, split, n2));
    } else {
        // Emit left sub-tree root as proof.
        proof.push(sub_root(leaves, 0, split));
        // Recurse right.
        collect_inner(&leaves[split..], n1 - split, n2 - split, proof);
    }
}

// ── ConsistencyProver ─────────────────────────────────────────────────────────

/// Generates consistency proofs between two snapshots of the same log.
///
/// Instantiate with the full leaf-hash array of the *newer* (larger) snapshot,
/// then call [`ConsistencyProver::generate`] for any `(root_v1, size_v1)` pair
/// that was previously published as a checkpoint.
pub struct ConsistencyProver {
    /// All leaf hashes in the current (newest) log.
    leaf_hashes: Vec<Digest>,
}

impl ConsistencyProver {
    /// Create a prover from the full leaf-hash sequence of the newest snapshot.
    pub fn new(leaf_hashes: Vec<Digest>) -> Self {
        Self { leaf_hashes }
    }

    /// Generate a consistency proof showing that the current log is an
    /// append-only extension of the snapshot described by `(root_v1, size_v1)`.
    ///
    /// # Errors
    ///
    /// * [`AuditError::InvalidConsistencyProof`] — if `size_v1 > size_v2`.
    ///
    /// # Complexity  O(n).
    pub fn generate(
        &self,
        root_v1: MerkleRoot,
        size_v1: u64,
    ) -> Result<ConsistencyProof, AuditError> {
        let size_v2 = self.leaf_hashes.len() as u64;

        if size_v1 > size_v2 {
            return Err(AuditError::InvalidConsistencyProof);
        }

        let root_v2 = compute_root(&self.leaf_hashes);

        if size_v1 == size_v2 || size_v1 == 0 {
            return Ok(ConsistencyProof {
                size_v1,
                size_v2,
                root_v1,
                root_v2,
                proof_hashes: Vec::new(),
            });
        }

        let proof_hashes = collect_proof_hashes(
            &self.leaf_hashes,
            size_v1 as usize,
            size_v2 as usize,
        );

        Ok(ConsistencyProof {
            size_v1,
            size_v2,
            root_v1,
            root_v2,
            proof_hashes,
        })
    }
}

// ─── LogHistory: multi-checkpoint consistency management ─────────────────────

/// Maintains a history of `(root, size)` checkpoints and can generate
/// consistency proofs between any two checkpoints.
///
/// Consumers typically keep one `LogHistory` per segment and add a checkpoint
/// after every `MerkleLog::publish_root` call.
pub struct LogHistory {
    /// Ordered list of (tree_size, root) pairs.
    checkpoints: Vec<(u64, MerkleRoot)>,
}

impl LogHistory {
    /// Create an empty history.
    pub fn new() -> Self {
        Self {
            checkpoints: Vec::new(),
        }
    }

    /// Record a new checkpoint.
    ///
    /// `tree_size` must be ≥ the size of the last recorded checkpoint.
    pub fn push(&mut self, tree_size: u64, root: MerkleRoot) {
        self.checkpoints.push((tree_size, root));
    }

    /// Generate a proof that the checkpoint at `new_idx` is a consistent
    /// extension of the checkpoint at `old_idx`.
    ///
    /// # Errors
    ///
    /// * [`AuditError::EntryNotFound`] — if either index is out of bounds.
    /// * [`AuditError::InvalidConsistencyProof`] — if `new_idx < old_idx`.
    pub fn prove_consistency(
        &self,
        leaf_hashes: &[Digest],
        old_idx: usize,
        new_idx: usize,
    ) -> Result<ConsistencyProof, AuditError> {
        let (old_size, old_root) = self
            .checkpoints
            .get(old_idx)
            .copied()
            .ok_or(AuditError::EntryNotFound {
                sequence: old_idx as u64,
            })?;
        let (new_size, _new_root) = self
            .checkpoints
            .get(new_idx)
            .copied()
            .ok_or(AuditError::EntryNotFound {
                sequence: new_idx as u64,
            })?;

        if new_size < old_size {
            return Err(AuditError::InvalidConsistencyProof);
        }

        let prover = ConsistencyProver::new(leaf_hashes[..new_size as usize].to_vec());
        prover.generate(old_root, old_size)
    }

    /// Number of recorded checkpoints.
    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    /// True when no checkpoints have been recorded.
    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }
}

impl Default for LogHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle_log::{compute_root, MerkleLog};
    use crate::types::LogSegmentId;

    fn seg() -> LogSegmentId {
        LogSegmentId::new("consistency-test").unwrap()
    }

    fn build_log(n: u64) -> MerkleLog {
        let mut log = MerkleLog::new(seg());
        for i in 0..n {
            log.append(i, "actor", "action", "target", "ok");
        }
        log
    }

    fn get_hashes(log: &MerkleLog, count: u64) -> Vec<Digest> {
        (1..=count)
            .map(|s| log.get_entry(s).unwrap().entry_hash)
            .collect()
    }

    // ── Core algorithm tests ─────────────────────────────────────────────────

    #[test]
    fn proof_for_equal_sizes_is_trivially_valid() {
        let mut log = build_log(4);
        let root = log.publish_root(1000);
        let hashes = get_hashes(&log, 4);
        let prover = ConsistencyProver::new(hashes);
        let proof = prover.generate(root, 4).unwrap();
        assert!(proof.verify().is_ok());
    }

    #[test]
    fn proof_verifies_for_growing_log_4_to_8() {
        let mut log = build_log(4);
        let root_4 = log.publish_root(1000);

        for i in 4..8u64 {
            log.append(i, "actor", "action", "target", "ok");
        }
        let all_hashes = get_hashes(&log, 8);

        let prover = ConsistencyProver::new(all_hashes);
        let proof = prover.generate(root_4, 4).unwrap();
        assert!(
            proof.verify().is_ok(),
            "consistency proof should verify for 4→8"
        );
    }

    #[test]
    fn tampered_root_fails_verification() {
        let mut log = build_log(4);
        let root_4 = log.publish_root(1000);

        for i in 4..8u64 {
            log.append(i, "actor", "action", "target", "ok");
        }
        let all_hashes = get_hashes(&log, 8);

        let prover = ConsistencyProver::new(all_hashes);
        let mut proof = prover.generate(root_4, 4).unwrap();
        proof.root_v1 = [0xAB; 32];
        assert!(proof.verify().is_err());
    }

    #[test]
    fn log_history_proves_consistency_across_checkpoints() {
        let mut log = build_log(0);
        let mut history = LogHistory::new();
        let mut all_hashes: Vec<Digest> = Vec::new();

        for i in 0..3u64 {
            log.append(i, "u", "a", "t", "ok");
        }
        let r3 = log.publish_root(1000);
        all_hashes.extend(get_hashes(&log, 3));
        history.push(3, r3);

        for i in 3..7u64 {
            log.append(i, "u", "a", "t", "ok");
        }
        let r7 = log.publish_root(2000);
        all_hashes.extend((4..=7).map(|s| log.get_entry(s).unwrap().entry_hash));
        history.push(7, r7);

        let proof = history.prove_consistency(&all_hashes, 0, 1).unwrap();
        assert!(proof.verify().is_ok());
    }

    #[test]
    fn single_to_multi_entry_proof() {
        let mut log = build_log(1);
        let root_1 = log.publish_root(1000);

        for i in 1..5u64 {
            log.append(i, "u", "a", "t", "ok");
        }
        let hashes = get_hashes(&log, 5);
        let prover = ConsistencyProver::new(hashes);
        let proof = prover.generate(root_1, 1).unwrap();
        assert!(proof.verify().is_ok());
    }

    // ── Exhaustive parametric test ───────────────────────────────────────────

    #[test]
    fn proof_verifies_for_all_small_cases() {
        for new_n in 1u64..=16 {
            let log = build_log(new_n);
            let all_hashes = get_hashes(&log, new_n);

            for old_n in 1..=new_n {
                let root_v1 = compute_root(&all_hashes[..old_n as usize]);
                let prover = ConsistencyProver::new(all_hashes.clone());
                let proof = prover.generate(root_v1, old_n).unwrap();
                assert!(
                    proof.verify().is_ok(),
                    "proof failed for old_n={old_n} new_n={new_n}"
                );
            }
        }
    }

    #[test]
    fn tampered_proof_hash_fails() {
        let log = build_log(8);
        let all_hashes = get_hashes(&log, 8);
        let root_4 = compute_root(&all_hashes[..4]);

        let prover = ConsistencyProver::new(all_hashes);
        let mut proof = prover.generate(root_4, 4).unwrap();
        if !proof.proof_hashes.is_empty() {
            proof.proof_hashes[0] = [0xFF; 32];
        }
        assert!(proof.verify().is_err());
    }
}
