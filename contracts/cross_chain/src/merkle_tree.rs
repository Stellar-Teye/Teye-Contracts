//! # Sparse Merkle Tree (SMT)
//!
//! This module provides a sparse Merkle tree suited for 256-bit medical record
//! IDs in a `no_std` / `wasm32` Soroban environment.
//!
//! ## Design Rationale
//!
//! ### Why Sparse?
//! A full binary Merkle tree at depth 256 would require 2^256 leaves — clearly
//! infeasible.  A *sparse* tree exploits the fact that almost all leaves are
//! empty: any subtree that contains only empty leaves collapses to a
//! deterministic **default hash** that can be computed from first principles
//! without storing anything.
//!
//! ### Node Addressing (Content-Addressed Store)
//! Nodes are keyed by their own 32-byte SHA-256 digest, not by position.
//! `NodeEntry { left, right }` is stored under the hash `H(left ‖ right)`.
//! This gives structural sharing for free: two identical subtrees share
//! exactly one entry in the map.
//!
//! ### Default Hashes
//! Let `D[0] = H([0u8; 32])` (the empty-leaf sentinel).
//! Then `D[i] = H(D[i-1] ‖ D[i-1])` for `i > 0`.
//! The initial root of an empty tree is represented as `[0u8; 32]`
//! ("null root" sentinel), saving an O(DEPTH) initialisation cost.
//!
//! When we descend into a node not present in the store we treat the entire
//! subtree as empty and fill sibling hashes with the appropriate `D[i]`.
//!
//! ### Hash Domain Separation
//! | Kind      | Pre-image             |
//! |-----------|-----------------------|
//! | Leaf      | `"LEAF:" ‖ raw_value` |
//! | Node      | `left[32] ‖ right[32]`|
//! | Empty     | `[0u8; 32]`          |
//!
//! Domain-separating leaves from internal nodes prevents second-preimage
//! attacks where an attacker crafts a leaf whose value equals the serialised
//! form of a valid internal node.
//!
//! ### Field-Level Proofs
//! A [`FieldProof`] wraps a *second-level* SMT built over a record's
//! individual fields.  The main tree stores only the field-tree root
//! (`record_root`), so a verifier can confirm one field without learning any
//! other.  The `depth` field in [`FieldProof`] is typically much smaller than
//! [`TREE_DEPTH`] (e.g. `32` for up to 2³² named fields).
//!
//! ### Storage Strategy
//! [`TreeState`] is a `#[contracttype]` value that contracts may persist in
//! `env.storage().persistent()` like any other Soroban type.
//! [`SparseMerkleTree`] is an ephemeral wrapper that borrows `TreeState` and
//! exposes methods; it is *not* itself stored on-chain.
//!
//! ### no_std / wasm32 Constraints
//! * No `std::collections` — [`soroban_sdk::Map`] and [`soroban_sdk::Vec`]
//!   are used for heap-allocated structures.
//! * Path traversal uses a fixed-size Rust array `[[u8; 32]; TREE_DEPTH]`
//!   allocated on the stack, avoiding any dynamic allocation for hot loops.
//! * All SHA-256 calls go through `env.crypto().sha256()`.

#![allow(dead_code)]

use soroban_sdk::{contracttype, Bytes, BytesN, Env};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of tree levels == number of key bits.
/// 256 levels give a 2^256 key-space, large enough for any SHA-256-derived
/// medical record identifier.
pub const TREE_DEPTH: usize = 256;

/// Sentinel value used as the root of a newly created, completely empty tree.
/// Distinct from any real hash so `insert` can recognise an uninitialised
/// root without performing a map lookup.
const NULL_ROOT: [u8; 32] = [0u8; 32];

// ---------------------------------------------------------------------------
// Storable / exchangeable types  (all use `#[contracttype]` for XDR encoding)
// ---------------------------------------------------------------------------

/// An internal node in the content-addressed node store.
///
/// Stored under key `H(left ‖ right)`.  Both children are 32-byte hashes;
/// at the leaf level they are SHA-256 digests of leaf values.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeEntry {
    /// Hash of the left subtree (bit = 0 branch).
    pub left: BytesN<32>,
    /// Hash of the right subtree (bit = 1 branch).
    pub right: BytesN<32>,
}

/// Persistent state of a [`SparseMerkleTree`].
///
/// Contracts can store and retrieve this type with
/// `env.storage().persistent().set(key, &state)`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeState {
    /// Current root hash.  `[0u8; 32]` means the tree is empty.
    pub root: BytesN<32>,
    /// Content-addressed node store: `node_hash → NodeEntry { left, right }`.
    pub nodes: soroban_sdk::Map<BytesN<32>, NodeEntry>,
}

/// Sibling-path inclusion proof for a single key-value pair.
///
/// To verify: recompute `leaf_hash = H("LEAF:" ‖ value)`, then fold
/// `siblings` from index 0 (root level) up to index `TREE_DEPTH - 1`
/// (deepest level), re-hashing at each step based on the key bit,
/// and finally compare with the expected root.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MerkleProof {
    /// The raw key that was proved (up to 32 bytes; shorter keys are
    /// implicitly right-padded with zero bits).
    pub key: Bytes,
    /// The raw value that was proved.
    pub value: Bytes,
    /// Sibling hashes, one per tree level, in top-down order
    /// (`siblings[0]` is the sibling of the root's child on the key path).
    pub siblings: soroban_sdk::Vec<BytesN<32>>,
    /// Pre-computed leaf hash `H("LEAF:" ‖ value)` for convenience.
    pub leaf_hash: BytesN<32>,
}

/// A single named field in a per-record field sub-tree.
///
/// `key` should be a deterministic identifier for the field (e.g. the raw
/// field name bytes, or a SHA-256 of the name).  `value` is the raw field
/// data.  Both are stored in the [`FieldProof`] and used to build a
/// second-level sparse Merkle tree over the record's fields.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldEntry {
    /// Field identifier (typically the field name bytes).
    pub key: Bytes,
    /// Raw field value bytes.
    pub value: Bytes,
}

/// Field-level inclusion proof.
///
/// Proves that a single named field within a medical record has a specific
/// value, *without* revealing any other field.
///
/// ## How it fits in the larger system
/// ```text
/// Main SMT leaf:  H("LEAF:" ‖ record_root)
///                         │
///             Field sub-tree (depth ≪ TREE_DEPTH)
///             ┌──────────────────────┐
///             │  key  = H(field_name)│
///             │  value= field_bytes  │
///             └──────────────────────┘
/// ```
/// The caller proves the main SMT inclusion (`MerkleProof` at the record
/// level) separately, then uses [`FieldProof`] to drill down to one field.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldProof {
    /// Root hash of the per-record field sub-tree.
    pub record_root: BytesN<32>,
    /// Key identifying the field (typically `H(field_name_bytes)`).
    pub field_key: Bytes,
    /// Raw bytes of the field value being proved.
    pub field_value: Bytes,
    /// Sibling hashes within the field sub-tree, top-down order.
    pub siblings: soroban_sdk::Vec<BytesN<32>>,
    /// Depth of the field sub-tree.  May be much less than [`TREE_DEPTH`];
    /// e.g. `32` comfortably accommodates 2³² distinct field names.
    pub depth: u32,
}

// ---------------------------------------------------------------------------
// SparseMerkleTree — ephemeral logic wrapper
// ---------------------------------------------------------------------------

/// Sparse Merkle tree with a configurable depth up to [`TREE_DEPTH`] = 256.
///
/// Wrap an existing [`TreeState`] (loaded from contract storage) or create a
/// new one with [`SparseMerkleTree::new`].  When done, call
/// [`SparseMerkleTree::into_state`] and persist the result.
///
/// For field sub-trees use [`SparseMerkleTree::with_depth`] to construct a
/// shallower tree; this keeps [`FieldProof`] sizes manageable.
///
/// All methods accept `env: &Env` because SHA-256 is a host function.
pub struct SparseMerkleTree {
    state: TreeState,
    /// Number of levels this instance traverses (1 ≤ depth ≤ TREE_DEPTH).
    depth: usize,
}

impl SparseMerkleTree {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// Create a brand-new, empty tree at the maximum [`TREE_DEPTH`] = 256.
    ///
    /// The root is initialised to `[0u8; 32]` (null root) to avoid the cost
    /// of computing `D[TREE_DEPTH]` up front.
    pub fn new(env: &Env) -> Self {
        Self::with_depth(env, TREE_DEPTH)
    }

    /// Create a brand-new, empty tree at a custom depth `d`.
    ///
    /// `d` must satisfy `1 ≤ d ≤ TREE_DEPTH`.  Smaller values yield cheaper
    /// proofs; use `TREE_DEPTH` for the main patient-record tree and a small
    /// `d` (e.g. `32`) for per-record field sub-trees.
    pub fn with_depth(env: &Env, d: usize) -> Self {
        let depth = d.clamp(1, TREE_DEPTH);
        Self {
            state: TreeState {
                root: BytesN::from_array(env, &NULL_ROOT),
                nodes: soroban_sdk::Map::new(env),
            },
            depth,
        }
    }

    /// Restore an existing tree from its persisted [`TreeState`].
    ///
    /// The restored tree is assumed to be a full-depth (`TREE_DEPTH`) tree.
    /// For field sub-trees (which are never persisted), use [`with_depth`].
    pub fn from_state(state: TreeState) -> Self {
        Self { state, depth: TREE_DEPTH }
    }

    /// Consume the wrapper and return the inner [`TreeState`] ready for
    /// contract storage.
    pub fn into_state(self) -> TreeState {
        self.state
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Current root hash.  Returns `[0u8; 32]` if the tree is empty.
    pub fn root(&self) -> &BytesN<32> {
        &self.state.root
    }

    // -----------------------------------------------------------------------
    // Core operations
    // -----------------------------------------------------------------------

    /// Insert or update `(key, value)` and return the new root hash.
    ///
    /// `key` is treated as a big-endian bit string; bytes shorter than
    /// 32 are implicitly zero-padded on the right.  `value` may be any
    /// byte string; it is stored as `H("LEAF:" ‖ value)`.
    ///
    /// # Complexity
    /// O(DEPTH) SHA-256 calls and O(DEPTH) map operations.
    pub fn insert(&mut self, env: &Env, key: &[u8], value: &[u8]) -> BytesN<32> {
        let value_bytes = Bytes::from_slice(env, value);
        self.insert_bytes(env, key, &value_bytes)
    }

    /// Like [`insert`] but accepts a pre-built [`Bytes`] value.
    ///
    /// Used internally by [`build_field_proof`] to avoid re-encoding.
    fn insert_bytes(&mut self, env: &Env, key: &[u8], value: &Bytes) -> BytesN<32> {
        let depth = self.depth;
        let leaf = hash_leaf(env, value);

        // Stack-allocated path arrays sized for TREE_DEPTH (the maximum);
        // only the first `depth` slots are used at runtime.  No heap needed.
        let mut sibling_hashes = [[0u8; 32]; TREE_DEPTH];
        let mut bits = [0u8; TREE_DEPTH];

        // -----------------------------------------------------------------
        // Phase 1: descend from root toward the target leaf, collecting
        // sibling hashes and key bits at each level.
        // -----------------------------------------------------------------
        let mut current = self.state.root.to_array();
        let mut empty_from: usize = depth; // sentinel: no empty break yet

        'descent: for lvl in 0..depth {
            bits[lvl] = get_bit(key, lvl);

            // Null root or unknown hash → empty subtree; fill defaults.
            let node_key = BytesN::from_array(env, &current);
            match self.state.nodes.get(node_key) {
                Some(node) => {
                    let (child, sibling) = if bits[lvl] == 0 {
                        (node.left.to_array(), node.right.to_array())
                    } else {
                        (node.right.to_array(), node.left.to_array())
                    };
                    sibling_hashes[lvl] = sibling;
                    current = child;
                }
                None => {
                    // `current` is not a known internal node — treat this
                    // entire subtree as empty from here down.
                    empty_from = lvl;
                    break 'descent;
                }
            }
        }

        // If we hit an empty subtree, fill the remaining sibling hashes with
        // the appropriate default (empty-subtree) hashes.  We compute them
        // in a single bottom-up pass: O(depth) total, not O(depth²).
        if empty_from < depth {
            // D[0] = H([0u8; 32])  (empty-leaf hash)
            let mut def: [u8; 32] = hash_empty_leaf(env).to_array();
            // sibling at the deepest key-bit level has remaining_depth=0
            // i.e. its default is D[0].  We fill from bottom up.
            for lvl in (empty_from..depth).rev() {
                bits[lvl] = get_bit(key, lvl);
                sibling_hashes[lvl] = def;
                // Advance: D[i+1] = H(D[i] ‖ D[i])
                def = hash_pair_raw(env, &def, &def).to_array();
            }
        }

        // -----------------------------------------------------------------
        // Phase 2: ascend from the new leaf back to the root, computing and
        // storing each new internal node.
        // -----------------------------------------------------------------
        let mut new_hash = leaf;
        for lvl in (0..depth).rev() {
            let sibling = BytesN::from_array(env, &sibling_hashes[lvl]);
            let (left, right) = if bits[lvl] == 0 {
                (new_hash.clone(), sibling)
            } else {
                (sibling, new_hash.clone())
            };
            let parent = hash_node(env, &left, &right);
            self.state
                .nodes
                .set(parent.clone(), NodeEntry { left, right });
            new_hash = parent;
        }

        self.state.root = new_hash.clone();
        new_hash
    }

    /// Generate a sibling-path inclusion proof for `(key, value)`.
    ///
    /// The proof is valid against the current root.  If `key` was never
    /// inserted, the proof witnesses absence: the recomputed root will differ
    /// from the stored root, so `verify` will return `false`.
    pub fn prove(&self, env: &Env, key: &[u8], value: &[u8]) -> MerkleProof {
        let value_bytes = Bytes::from_slice(env, value);
        self.prove_bytes(env, key, &value_bytes)
    }

    /// Like [`prove`] but accepts a pre-built [`Bytes`] value.
    fn prove_bytes(&self, env: &Env, key: &[u8], value: &Bytes) -> MerkleProof {
        let depth = self.depth;
        let mut siblings: soroban_sdk::Vec<BytesN<32>> = soroban_sdk::Vec::new(env);

        let mut current = self.state.root.to_array();
        let mut empty_from: usize = depth;

        for lvl in 0..depth {
            let bit = get_bit(key, lvl);
            let node_key = BytesN::from_array(env, &current);
            match self.state.nodes.get(node_key) {
                Some(node) => {
                    let (child, sibling) = if bit == 0 {
                        (node.left.clone(), node.right.clone())
                    } else {
                        (node.right.clone(), node.left.clone())
                    };
                    siblings.push_back(sibling);
                    current = child.to_array();
                }
                None => {
                    empty_from = lvl;
                    break;
                }
            }
        }

        // Fill remaining levels with default (empty) sibling hashes.
        if empty_from < depth {
            let mut def: [u8; 32] = hash_empty_leaf(env).to_array();
            for _lvl in (empty_from..depth).rev() {
                siblings.push_back(BytesN::from_array(env, &def));
                def = hash_pair_raw(env, &def, &def).to_array();
            }
            // The siblings were pushed in reverse order for the empty section;
            // reverse that section so the Vec is always top-down.
            siblings = reverse_suffix(env, siblings, empty_from as u32);
        }

        let leaf_hash = hash_leaf(env, value);
        MerkleProof {
            key: Bytes::from_slice(env, key),
            value: value.clone(),
            siblings,
            leaf_hash,
        }
    }

    /// Verify an inclusion proof against a given root.
    ///
    /// Returns `true` iff recomputing the root from `(key, value, proof)`
    /// yields exactly `root`.
    ///
    /// The depth is inferred from `proof.siblings.len()`, so the same function
    /// handles both full-depth proofs (256 siblings) and field sub-tree proofs
    /// (fewer siblings), as long as `root` matches the corresponding tree root.
    pub fn verify(env: &Env, root: &BytesN<32>, key: &[u8], value: &[u8], proof: &MerkleProof) -> bool {
        let depth = proof.siblings.len() as usize;
        if depth == 0 || depth > TREE_DEPTH {
            return false;
        }

        let value_bytes = Bytes::from_slice(env, value);
        let expected_leaf = hash_leaf(env, &value_bytes);
        if expected_leaf != proof.leaf_hash {
            return false;
        }

        let mut current = proof.leaf_hash.clone();

        // Fold from the deepest level up to the root.
        for lvl in (0..depth).rev() {
            let bit = get_bit(key, lvl);
            let sibling = match proof.siblings.get(lvl as u32) {
                Some(s) => s,
                None => return false,
            };
            let (left, right) = if bit == 0 {
                (current, sibling)
            } else {
                (sibling, current)
            };
            current = hash_node(env, &left, &right);
        }

        &current == root
    }

    // -----------------------------------------------------------------------
    // Field-level proof helpers
    // -----------------------------------------------------------------------
    //
    // The field sub-tree is a SparseMerkleTree with `depth = field_depth`
    // (typically 32), NOT the full 256-level tree.  This ensures that:
    //   1. `record_root` = root of the depth-`field_depth` mini SMT
    //   2. `FieldProof.siblings` has exactly `field_depth` entries
    //   3. `verify_field` folds exactly `field_depth` times to reach
    //      `record_root`
    // The mini SMT is ephemeral (never persisted) so no TreeState migration
    // is required when `field_depth` changes between deployments.

    /// Build a field sub-tree over `fields` and return the sub-tree root plus
    /// an individual [`FieldProof`] for the field identified by `field_key`.
    ///
    /// `fields` is a list of [`FieldEntry`] `{ key, value }` pairs.
    /// Keys must be ≤ `field_depth / 8` bytes (excess bits are zeroed).
    ///
    /// The returned `record_root` should be the *value* inserted into the
    /// main SMT for that patient's record entry.
    ///
    /// # Example flow
    /// ```text
    /// let (record_root, field_proof) =
    ///     SparseMerkleTree::build_field_proof(&env, &fields, b"dob", 32);
    /// main_tree.insert(&env, &patient_id, &record_root.to_array());
    /// // Later — given proof.record_root, verify a single field:
    /// assert!(SparseMerkleTree::verify_field(&env, &field_proof));
    /// ```
    pub fn build_field_proof(
        env: &Env,
        fields: &soroban_sdk::Vec<FieldEntry>,
        field_key: &[u8],
        field_depth: u32,
    ) -> (BytesN<32>, FieldProof) {
        // A mini SMT at exactly `field_depth` levels — proof siblings are
        // `field_depth` entries naturally without any post-hoc truncation.
        let mut mini = SparseMerkleTree::with_depth(env, field_depth as usize);

        let target_key = Bytes::from_slice(env, field_key);
        let mut found_value: Bytes = Bytes::from_slice(env, &[]);

        let flen = fields.len();
        for i in 0..flen {
            let Some(entry) = fields.get(i) else {
                continue;
            };
            // Normalise the field key to the sub-tree's fixed-width key array.
            let fk_arr = bytes_to_key_array(&entry.key, field_depth);
            mini.insert_bytes(env, &fk_arr, &entry.value);
            if entry.key == target_key {
                found_value = entry.value.clone();
            }
        }

        // `record_root` is the mini SMT root — callers insert THIS as the
        // value under the patient key in the main (256-level) SMT.
        let record_root = mini.root().clone();

        // Build the inclusion proof for the requested field in the mini SMT.
        // `prove_bytes` on a depth-`field_depth` tree returns exactly
        // `field_depth` siblings — no truncation needed.
        let proof_key = bytes_to_key_array(&target_key, field_depth);
        let inner = mini.prove_bytes(env, &proof_key, &found_value);

        let proof = FieldProof {
            record_root: record_root.clone(),
            field_key: target_key,
            field_value: found_value,
            siblings: inner.siblings,
            depth: field_depth,
        };

        (record_root, proof)
    }

    /// Verify a [`FieldProof`] against `record_root`.
    ///
    /// This is structurally identical to [`Self::verify`] but uses the
    /// shallower `field_depth` and the field sub-tree's own root.
    pub fn verify_field(env: &Env, proof: &FieldProof) -> bool {
        let depth = proof.depth as usize;
        if proof.siblings.len() as usize != depth {
            return false;
        }

        let mut current = hash_leaf(env, &proof.field_value);
        // Convert the Bytes key to a plain [u8; 32] so get_bit can walk it.
        let field_key_arr = bytes_to_array_32(&proof.field_key);

        for lvl in (0..depth).rev() {
            let bit = get_bit(&field_key_arr, lvl);
            let sibling = match proof.siblings.get(lvl as u32) {
                Some(s) => s,
                None => return false,
            };
            let (left, right) = if bit == 0 {
                (current, sibling)
            } else {
                (sibling, current)
            };
            current = hash_node(env, &left, &right);
        }

        current == proof.record_root
    }
}

// ---------------------------------------------------------------------------
// Private hashing helpers
// ---------------------------------------------------------------------------

/// `H("LEAF:" ‖ value)` — domain-separated leaf hash.
///
/// The `"LEAF:"` prefix ensures a leaf hash can never collide with an
/// internal-node hash (which is produced by [`hash_node`]).
///
/// In soroban-sdk v25+, `env.crypto().sha256()` returns
/// `soroban_sdk::crypto::Hash<32>`; `.into()` converts it to `BytesN<32>`
/// via the `From<Hash<N>> for BytesN<N>` impl provided by the SDK.
#[inline]
fn hash_leaf(env: &Env, value: &Bytes) -> BytesN<32> {
    let mut data = Bytes::from_slice(env, b"LEAF:");
    data.append(value);
    env.crypto().sha256(&data).into()
}

/// `H([0u8; 32])` — hash of the canonical empty-leaf sentinel.
#[inline]
fn hash_empty_leaf(env: &Env) -> BytesN<32> {
    env.crypto()
        .sha256(&Bytes::from_slice(env, &[0u8; 32]))
        .into()
}

/// `H(left ‖ right)` — internal-node hash.
///
/// Left and right are always 32 bytes, so the pre-image is always 64 bytes.
/// No length ambiguity, no domain separator required (leaves are already
/// distinguished by the `"LEAF:"` prefix).
#[inline]
fn hash_node(env: &Env, left: &BytesN<32>, right: &BytesN<32>) -> BytesN<32> {
    hash_pair_raw(env, &left.to_array(), &right.to_array())
}

/// Hash two raw 32-byte slices as a 64-byte pre-image.
#[inline]
fn hash_pair_raw(env: &Env, a: &[u8; 32], b: &[u8; 32]) -> BytesN<32> {
    let mut buf = [0u8; 64];
    buf[..32].copy_from_slice(a);
    buf[32..].copy_from_slice(b);
    env.crypto().sha256(&Bytes::from_slice(env, &buf)).into()
}

// ---------------------------------------------------------------------------
// Bit manipulation
// ---------------------------------------------------------------------------

/// Extract bit `bit_index` from `key`, treating the key as a big-endian
/// (MSB-first) bit string.
///
/// Returns `0` for bits beyond the length of `key` (implicit zero padding).
#[inline]
fn get_bit(key: &[u8], bit_index: usize) -> u8 {
    let byte_idx = bit_index / 8;
    if byte_idx >= key.len() {
        return 0;
    }
    // Within the byte: bit 0 of the index = MSB of the byte.
    let bit_pos = 7 - (bit_index % 8);
    (key[byte_idx] >> bit_pos) & 1
}

// ---------------------------------------------------------------------------
// Vec utilities (no std::collections)
// ---------------------------------------------------------------------------

/// Reverse the sub-slice of `v` starting at index `from` in-place,
/// returning a new [`soroban_sdk::Vec`].
///
/// Used to correct the ordering of default siblings that were pushed in
/// reverse during the `prove` walk.
fn reverse_suffix(
    env: &Env,
    v: soroban_sdk::Vec<BytesN<32>>,
    from: u32,
) -> soroban_sdk::Vec<BytesN<32>> {
    let len = v.len();
    if from >= len {
        return v;
    }
    let mut out: soroban_sdk::Vec<BytesN<32>> = soroban_sdk::Vec::new(env);
    // Copy prefix unchanged.
    for i in 0..from {
        if let Some(x) = v.get(i) {
            out.push_back(x);
        }
    }
    // Copy suffix in reverse.
    for i in (from..len).rev() {
        if let Some(x) = v.get(i) {
            out.push_back(x);
        }
    }
    out
}

/// Keep only the first `n` elements of a [`soroban_sdk::Vec`].
fn truncate_vec(
    env: &Env,
    v: soroban_sdk::Vec<BytesN<32>>,
    n: u32,
) -> soroban_sdk::Vec<BytesN<32>> {
    let mut out: soroban_sdk::Vec<BytesN<32>> = soroban_sdk::Vec::new(env);
    for i in 0..n.min(v.len()) {
        if let Some(x) = v.get(i) {
            out.push_back(x);
        }
    }
    out
}

/// Copy the first 32 bytes of a [`Bytes`] value into a `[u8; 32]` array.
///
/// Bytes beyond position 31 are silently dropped; positions not covered by
/// `b` are zero-filled.  Used to feed Soroban host `Bytes` values into
/// functions that expect a plain `&[u8]` (e.g. [`get_bit`]).
fn bytes_to_array_32(b: &Bytes) -> [u8; 32] {
    let mut arr = [0u8; 32];
    let len = (b.len() as usize).min(32);
    for i in 0..(len as u32) {
        if let Some(byte) = b.get(i) {
            arr[i as usize] = byte;
        }
    }
    arr
}

/// Normalise a [`Bytes`] field key into a `[u8; 32]` path array, zeroing
/// all bits beyond `field_depth` so that the key occupies only the first
/// `field_depth` bit-positions of the array.
fn bytes_to_key_array(key: &Bytes, field_depth: u32) -> [u8; 32] {
    let mut arr = bytes_to_array_32(key);
    let full_bytes = (field_depth / 8) as usize;
    let remainder_bits = (field_depth % 8) as usize;
    if remainder_bits > 0 && full_bytes < 32 {
        let mask: u8 = !((1u8 << (8 - remainder_bits)) - 1);
        arr[full_bytes] &= mask;
        for b in arr.iter_mut().skip(full_bytes + 1) {
            *b = 0;
        }
    } else if remainder_bits == 0 {
        for b in arr.iter_mut().skip(full_bytes) {
            *b = 0;
        }
    }
    arr
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(deprecated)] // env.budget() is deprecated in SDK v25; tests use it to
                      // disable metered limits for integration-style assertions.
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_empty_tree_root_is_null() {
        let env = Env::default();
        let tree = SparseMerkleTree::new(&env);
        assert_eq!(tree.root().to_array(), NULL_ROOT);
    }

    #[test]
    fn test_insert_changes_root() {
        let env = Env::default();
        let mut tree = SparseMerkleTree::new(&env);
        let key = [0x01u8; 32];
        let value = b"patient_record_v1";
        let root = tree.insert(&env, &key, value);
        assert_ne!(root.to_array(), NULL_ROOT);
    }

    #[test]
    fn test_insert_same_key_twice_same_root() {
        let env = Env::default();
        let mut tree = SparseMerkleTree::new(&env);
        let key = [0x02u8; 32];
        let value = b"record";

        let root1 = tree.insert(&env, &key, value);
        let root2 = tree.insert(&env, &key, value);
        assert_eq!(root1.to_array(), root2.to_array());
    }

    #[test]
    fn test_different_keys_different_roots() {
        let env = Env::default();
        let mut tree = SparseMerkleTree::new(&env);
        let key1 = [0x00u8; 32];
        let key2 = [0xFFu8; 32];
        let value = b"same_value";

        tree.insert(&env, &key1, value);
        let root1 = tree.root().clone();

        let mut tree2 = SparseMerkleTree::new(&env);
        tree2.insert(&env, &key2, value);
        let root2 = tree2.root().clone();

        assert_ne!(root1.to_array(), root2.to_array());
    }

    #[test]
    fn test_proof_verify_roundtrip() {
        let env = Env::default();
        let mut tree = SparseMerkleTree::new(&env);
        let key = [0xABu8; 32];
        let value = b"health_record";

        tree.insert(&env, &key, value);
        let root = tree.root().clone();
        let proof = tree.prove(&env, &key, value);

        assert!(SparseMerkleTree::verify(&env, &root, &key, value, &proof));
    }

    #[test]
    fn test_verify_wrong_value_fails() {
        let env = Env::default();
        let mut tree = SparseMerkleTree::new(&env);
        let key = [0xCDu8; 32];
        let value = b"correct_value";

        tree.insert(&env, &key, value);
        let root = tree.root().clone();
        let proof = tree.prove(&env, &key, value);

        // Mutate the value — proof should not verify.
        assert!(!SparseMerkleTree::verify(
            &env,
            &root,
            &key,
            b"wrong_value",
            &proof
        ));
    }

    #[test]
    fn test_get_bit_msb_first() {
        // Key = 0b10000000 = 0x80
        let key = [0x80u8];
        assert_eq!(get_bit(&key, 0), 1); // bit 0 = MSB = 1
        assert_eq!(get_bit(&key, 1), 0);
        assert_eq!(get_bit(&key, 7), 0);
    }

    #[test]
    fn test_get_bit_beyond_key_is_zero() {
        let key = [0xFFu8];
        assert_eq!(get_bit(&key, 8), 0); // beyond key length
    }

    #[test]
    fn test_multiple_inserts_proof_each() {
        let env = Env::default();
        // A 256-level SMT requires many SHA-256 ops per operation; disable
        // the metered budget for this unit test (budget limits are enforced
        // on-chain by the Soroban runtime, not here).
        env.budget().reset_unlimited();

        let mut tree = SparseMerkleTree::new(&env);

        let entries: &[([u8; 32], &[u8])] = &[
            ([0x01u8; 32], b"record_a"),
            ([0x02u8; 32], b"record_b"),
            ([0xFEu8; 32], b"record_c"),
        ];

        for (k, v) in entries {
            tree.insert(&env, k, v);
        }
        let root = tree.root().clone();

        // Every inserted key must verify against the final root.
        for (k, v) in entries {
            let proof = tree.prove(&env, k, v);
            assert!(
                SparseMerkleTree::verify(&env, &root, k, v, &proof),
                "proof failed for key {:?}",
                k
            );
        }
    }

    #[test]
    fn test_field_proof_roundtrip() {
        let env = Env::default();
        env.budget().reset_unlimited();

        let mut fields: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(&env);
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"name"),
            value: Bytes::from_slice(&env, b"Alice"),
        });
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"dob"),
            value: Bytes::from_slice(&env, b"1990-01-01"),
        });
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"blood_type"),
            value: Bytes::from_slice(&env, b"O+"),
        });

        // Prove the "dob" field without revealing "name" or "blood_type".
        let (_record_root, field_proof) =
            SparseMerkleTree::build_field_proof(&env, &fields, b"dob", 32);

        assert!(SparseMerkleTree::verify_field(&env, &field_proof));
    }

    #[test]
    fn test_field_proof_wrong_value_fails() {
        let env = Env::default();
        env.budget().reset_unlimited();

        let mut fields: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(&env);
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"name"),
            value: Bytes::from_slice(&env, b"Bob"),
        });

        let (_record_root, mut field_proof) =
            SparseMerkleTree::build_field_proof(&env, &fields, b"name", 32);

        // Tamper with the proven value — verify_field must reject it.
        field_proof.field_value = Bytes::from_slice(&env, b"Eve");
        assert!(!SparseMerkleTree::verify_field(&env, &field_proof));
    }

    #[test]
    fn test_state_roundtrip() {
        let env = Env::default();
        env.budget().reset_unlimited();

        let mut tree = SparseMerkleTree::new(&env);
        let key = [0x55u8; 32];
        let value = b"persist_me";

        tree.insert(&env, &key, value);
        let expected_root = tree.root().clone();

        // Serialise to TreeState and restore.
        let state = tree.into_state();
        let tree2 = SparseMerkleTree::from_state(state);
        assert_eq!(tree2.root().to_array(), expected_root.to_array());

        // A proof built on tree2 must still verify.
        let proof = tree2.prove(&env, &key, value);
        assert!(SparseMerkleTree::verify(
            &env,
            tree2.root(),
            &key,
            value,
            &proof
        ));
    }
}
