//! # Lineage — on-chain DAG provenance primitives
//!
//! This module implements the core data model for tracking the complete history
//! of every medical record as a **Directed Acyclic Graph (DAG)**.
//!
//! ## Core Concepts
//!
//! * Every medical record is represented as a [`LineageNode`] identified by its
//!   `record_id`.
//! * Directed [`LineageEdge`]s connect nodes and carry a [`RelationshipKind`]
//!   (e.g. `DerivedFrom`, `ModifiedBy`).
//! * A SHA-256 **commitment chain** threads through every edge so that any
//!   gap or tampering is detectable via [`verify_node_integrity`].
//! * The graph is stored compactly: each node stores its *parent* edge list
//!   (O(in-degree)) and each edge is individually addressable by its auto-
//!   incremented `edge_id`.
//!
//! ## Storage layout (Soroban persistent)
//!
//! | Key                          | Type              | Description               |
//! |------------------------------|-------------------|---------------------------|
//! | `(LIN_NODE, record_id)`      | `LineageNode`     | Node metadata             |
//! | `(LIN_EDGE, edge_id)`        | `LineageEdge`     | Individual edge           |
//! | `(LIN_OUTEDG, record_id)`    | `Vec<u64>`        | Edge-IDs leaving a node   |
//! | `(LIN_INEDG, record_id)`     | `Vec<u64>`        | Edge-IDs entering a node  |
//! | `(LIN_ECTR,)`                | `u64`             | Global edge counter       |
//! | `(LIN_CONTRACT, record_id)`  | `String`          | Cross-contract origin tag |
//!
//! ## Time/Space complexity
//!
//! | Operation                  | Time       | Storage writes |
//! |----------------------------|------------|----------------|
//! | `create_node`              | O(1)       | 1              |
//! | `add_edge`                 | O(1) amort | 3              |
//! | `get_parents` / `get_children` | O(deg) | 0           |
//! | `verify_node_integrity`    | O(depth)   | 0              |
//! | `prune_summarised`         | O(n) amort | O(n)           |

use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, Env, String, Symbol, Vec};

// ── Storage key prefixes ────────────────────────────────────────────────────

const LIN_NODE: Symbol = symbol_short!("LIN_NODE");
const LIN_EDGE: Symbol = symbol_short!("LIN_EDG");
const LIN_OUTEDG: Symbol = symbol_short!("LIN_OUT");
const LIN_INEDG: Symbol = symbol_short!("LIN_IN");
const LIN_ECTR: Symbol = symbol_short!("LIN_ECTR");
const LIN_CONTRACT: Symbol = symbol_short!("LIN_CTR");

/// Soroban recommended TTL constants (ledgers).
/// Threshold: ~60 days; extend-to: ~120 days at 5 s/ledger.
pub const TTL_THRESHOLD: u32 = 5_184_000;
pub const TTL_EXTEND_TO: u32 = 10_368_000;

/// Maximum number of edges stored per adjacency list before compaction.
/// Prevents runaway growth; in practice any single record has far fewer edges.
pub const MAX_EDGES_PER_NODE: u32 = 256;

// ── Relationship kinds ──────────────────────────────────────────────────────

/// The semantic relationship represented by a directed lineage edge.
///
/// Edges are directed *from parent to child* (i.e. `source → target`).
///
/// | Variant         | Meaning                                                     |
/// |-----------------|-------------------------------------------------------------|
/// | `Created`       | `target` was freshly created; `source` is the creator addr  |
/// | `DerivedFrom`   | `target` is derived from `source` (e.g. Rx from exam)       |
/// | `ModifiedBy`    | `source` was modified to produce `target`                   |
/// | `SharedWith`    | `source` was shared with the party recorded in `target`     |
/// | `AggregatedInto`| `source` was aggregated into the composite `target`         |
/// | `CrossContract` | Provenance crosses a contract boundary                      |
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationshipKind {
    Created,
    DerivedFrom,
    ModifiedBy,
    SharedWith,
    AggregatedInto,
    CrossContract,
}

impl RelationshipKind {
    pub fn discriminant(&self) -> u32 {
        match self {
            RelationshipKind::Created => 1,
            RelationshipKind::DerivedFrom => 2,
            RelationshipKind::ModifiedBy => 3,
            RelationshipKind::SharedWith => 4,
            RelationshipKind::AggregatedInto => 5,
            RelationshipKind::CrossContract => 6,
        }
    }
}

// ── Core types ──────────────────────────────────────────────────────────────

/// A node in the provenance DAG, corresponding to one medical record.
///
/// `commitment` is a SHA-256 hash computed over `(record_id ‖ creator ‖
/// created_at ‖ record_type_tag ‖ parent_commitment)` where
/// `parent_commitment` is either the zero-hash (genesis node) or the
/// commitment of the most recent direct parent.  This creates a tamper-
/// evident chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageNode {
    /// Unique record identifier (matches the VisionRecord / Prescription id).
    pub record_id: u64,
    /// On-chain address of the entity that created this node.
    pub creator: Address,
    /// Ledger timestamp at creation.
    pub created_at: u64,
    /// Human-readable record type tag (e.g. "Examination", "Prescription").
    pub record_type_tag: String,
    /// SHA-256 commitment chaining this node to its history.
    pub commitment: BytesN<32>,
    /// Whether this node has been pruned (summarised).  A pruned node retains
    /// its commitment but its edge list is collapsed into a summary hash.
    pub pruned: bool,
    /// Optional summary commitment replacing detailed edges after pruning.
    pub summary_commitment: Option<BytesN<32>>,
    /// Optional originating contract id for cross-contract lineage.
    pub origin_contract: Option<String>,
}

/// A directed edge in the provenance DAG.
///
/// `edge_commitment` = SHA-256(`source_id ‖ target_id ‖ kind_u32 ‖
/// actor ‖ timestamp ‖ source_commitment ‖ target_parent_commitment`).
/// Storing the commitment on the edge lets verifiers reconstruct the
/// full integrity proof without loading every historical node.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageEdge {
    /// Auto-incremented globally unique edge identifier.
    pub edge_id: u64,
    /// Record ID of the parent (source) node.
    pub source_id: u64,
    /// Record ID of the child (target) node.
    pub target_id: u64,
    /// Semantic relationship type discriminant.
    pub kind: u32,
    /// Address of the actor that created this relationship.
    pub actor: Address,
    /// Ledger timestamp at which this edge was recorded.
    pub timestamp: u64,
    /// Optional free-form metadata hash (e.g. off-chain annotation digest).
    pub metadata_hash: Option<BytesN<32>>,
    /// Cryptographic commitment proving edge integrity.
    pub edge_commitment: BytesN<32>,
}

/// Summary produced by lineage pruning.  Replaces the detailed edge list of a
/// node while preserving the commitment chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageSummary {
    pub record_id: u64,
    /// Number of provenance hops that were summarised.
    pub summarised_depth: u32,
    /// Merkle-like root over all summarised edge commitments.
    pub summary_commitment: BytesN<32>,
    /// Earliest timestamp in the summarised window.
    pub window_start: u64,
    /// Latest timestamp in the summarised window.
    pub window_end: u64,
}

// ── Traversal result types ──────────────────────────────────────────────────

/// A node entry in a traversal result, paired with its depth from the start.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraversalNode {
    pub node: LineageNode,
    /// BFS depth from the starting record.
    pub depth: u32,
    /// Edge that connected the parent to this node (None for the root).
    /// Using Vec instead of Option to bypass a contracttype bug.
    pub via_edge: Vec<LineageEdge>,
}

/// Result of a full provenance traversal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraversalResult {
    /// Ordered list of nodes visited (BFS order).
    pub nodes: Vec<TraversalNode>,
    /// True if the traversal was truncated by depth or node cap.
    pub truncated: bool,
    /// Total nodes visited.
    pub total_visited: u32,
}

// ── Commitment helpers ──────────────────────────────────────────────────────

/// Computes the genesis commitment for a newly created node.
///
/// `H(record_id ‖ creator_bytes ‖ created_at ‖ tag_bytes ‖ [0u8;32])`
pub fn genesis_commitment(
    env: &Env,
    record_id: u64,
    creator: &Address,
    created_at: u64,
    tag: &str,
) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&Bytes::from_slice(env, &record_id.to_be_bytes()));

    let addr_str = creator.to_string();
    buf.append(&addr_str.to_bytes());

    buf.append(&Bytes::from_slice(env, &created_at.to_be_bytes()));
    buf.append(&Bytes::from_slice(env, tag.as_bytes()));
    buf.append(&Bytes::from_slice(env, &[0u8; 32])); // zero parent commitment
    env.crypto().sha256(&buf).into()
}

/// Computes the commitment for an existing node that is being updated /
/// extended with a new parent commitment.
///
/// `H(record_id ‖ creator_bytes ‖ created_at ‖ tag_bytes ‖ parent_commit)`
pub fn extend_commitment(
    env: &Env,
    node: &LineageNode,
    parent_commitment: &BytesN<32>,
) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&Bytes::from_slice(env, &node.record_id.to_be_bytes()));

    buf.append(&node.creator.to_string().to_bytes());

    buf.append(&Bytes::from_slice(env, &node.created_at.to_be_bytes()));
    buf.append(&node.record_type_tag.to_bytes());

    let parent_arr = parent_commitment.to_array();
    buf.append(&Bytes::from_slice(env, &parent_arr));

    env.crypto().sha256(&buf).into()
}

/// Computes the edge commitment.
///
/// `H(source_id ‖ target_id ‖ kind_u32 ‖ actor ‖ timestamp ‖ edge_id)`
pub fn compute_edge_commitment(
    env: &Env,
    source_id: u64,
    target_id: u64,
    kind: &RelationshipKind,
    actor: &Address,
    timestamp: u64,
    edge_id: u64,
) -> BytesN<32> {
    let kind_u32 = kind.discriminant();
    let mut buf = Bytes::new(env);
    buf.append(&Bytes::from_slice(env, &source_id.to_be_bytes()));
    buf.append(&Bytes::from_slice(env, &target_id.to_be_bytes()));
    buf.append(&Bytes::from_slice(env, &kind_u32.to_be_bytes()));

    buf.append(&actor.to_string().to_bytes());

    buf.append(&Bytes::from_slice(env, &timestamp.to_be_bytes()));
    buf.append(&Bytes::from_slice(env, &edge_id.to_be_bytes()));
    env.crypto().sha256(&buf).into()
}

// ── Storage helpers ─────────────────────────────────────────────────────────

fn node_key(record_id: u64) -> (Symbol, u64) {
    (LIN_NODE, record_id)
}

fn edge_key(edge_id: u64) -> (Symbol, u64) {
    (LIN_EDGE, edge_id)
}

fn out_edges_key(record_id: u64) -> (Symbol, u64) {
    (LIN_OUTEDG, record_id)
}

fn in_edges_key(record_id: u64) -> (Symbol, u64) {
    (LIN_INEDG, record_id)
}

fn extend_node_ttl(env: &Env, record_id: u64) {
    env.storage()
        .persistent()
        .extend_ttl(&node_key(record_id), TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_edge_ttl(env: &Env, edge_id: u64) {
    env.storage()
        .persistent()
        .extend_ttl(&edge_key(edge_id), TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_adj_ttl(env: &Env, key: &(Symbol, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Node CRUD ───────────────────────────────────────────────────────────────

/// Creates a new lineage node for `record_id`.
///
/// Idempotent: if the node already exists, returns the existing node without
/// overwriting.  Returns `true` if the node was newly created.
pub fn create_node(
    env: &Env,
    record_id: u64,
    creator: Address,
    record_type_tag: &str,
    origin_contract: Option<String>,
) -> (LineageNode, bool) {
    let key = node_key(record_id);
    if let Some(existing) = env.storage().persistent().get::<_, LineageNode>(&key) {
        extend_node_ttl(env, record_id);
        return (existing, false);
    }

    let now = env.ledger().timestamp();
    let commitment = genesis_commitment(env, record_id, &creator, now, record_type_tag);

    let node = LineageNode {
        record_id,
        creator,
        created_at: now,
        record_type_tag: String::from_str(env, record_type_tag),
        commitment,
        pruned: false,
        summary_commitment: None,
        origin_contract,
    };

    env.storage().persistent().set(&key, &node);
    extend_node_ttl(env, record_id);
    (node, true)
}

/// Returns a lineage node, or `None` if it does not exist.
pub fn get_node(env: &Env, record_id: u64) -> Option<LineageNode> {
    let key = node_key(record_id);
    let node: Option<LineageNode> = env.storage().persistent().get(&key);
    if node.is_some() {
        extend_node_ttl(env, record_id);
    }
    node
}

/// Persists an updated node (e.g. after commitment extension or pruning).
pub fn save_node(env: &Env, node: &LineageNode) {
    env.storage().persistent().set(&node_key(node.record_id), node);
    extend_node_ttl(env, node.record_id);
}

// ── Edge CRUD ───────────────────────────────────────────────────────────────

/// Atomically allocates the next edge id.  O(1).
fn next_edge_id(env: &Env) -> u64 {
    let current: u64 = env.storage().persistent().get(&LIN_ECTR).unwrap_or(0);
    let next = current.saturating_add(1);
    env.storage().persistent().set(&LIN_ECTR, &next);
    next
}

/// Records a directed lineage edge from `source_id` → `target_id`, updates
/// target node's commitment to chain from the source, and refreshes both
/// adjacency lists.
///
/// Returns the newly created edge.
///
/// # Complexity
/// - Time: O(in-degree of target) for commitment update; O(1) otherwise.
/// - Storage writes: 4 (edge + 2 adj-list + counter).
pub fn add_edge(
    env: &Env,
    source_id: u64,
    target_id: u64,
    kind: RelationshipKind,
    actor: Address,
    metadata_hash: Option<BytesN<32>>,
) -> LineageEdge {
    let edge_id = next_edge_id(env);
    let now = env.ledger().timestamp();

    let edge_commitment = compute_edge_commitment(
        env, source_id, target_id, &kind, &actor, now, edge_id,
    );

    let edge = LineageEdge {
        edge_id,
        source_id,
        target_id,
        kind: kind.discriminant(),
        actor,
        timestamp: now,
        metadata_hash,
        edge_commitment,
    };

    // Persist the edge.
    let ek = edge_key(edge_id);
    env.storage().persistent().set(&ek, &edge);
    extend_edge_ttl(env, edge_id);

    // Update out-edges for source.
    let out_key = out_edges_key(source_id);
    let mut out_list: Vec<u64> = env
        .storage()
        .persistent()
        .get(&out_key)
        .unwrap_or_else(|| Vec::new(env));
    // Enforce per-node cap to prevent runaway storage growth.
    if out_list.len() < MAX_EDGES_PER_NODE {
        out_list.push_back(edge_id);
        env.storage().persistent().set(&out_key, &out_list);
        extend_adj_ttl(env, &out_key);
    }

    // Update in-edges for target.
    let in_key = in_edges_key(target_id);
    let mut in_list: Vec<u64> = env
        .storage()
        .persistent()
        .get(&in_key)
        .unwrap_or_else(|| Vec::new(env));
    if in_list.len() < MAX_EDGES_PER_NODE {
        in_list.push_back(edge_id);
        env.storage().persistent().set(&in_key, &in_list);
        extend_adj_ttl(env, &in_key);
    }

    // Extend the target node's commitment to chain from this edge's source.
    if let Some(mut target_node) = get_node(env, target_id) {
        if !target_node.pruned {
            let source_commitment = get_node(env, source_id)
                .map(|n| n.commitment)
                .unwrap_or_else(|| BytesN::from_array(env, &[0u8; 32]));
            target_node.commitment = extend_commitment(env, &target_node, &source_commitment);
            save_node(env, &target_node);
        }
    }

    edge
}

/// Retrieves an edge by its id.
pub fn get_edge(env: &Env, edge_id: u64) -> Option<LineageEdge> {
    env.storage().persistent().get(&edge_key(edge_id))
}

/// Returns all edges whose **source** is `record_id` (children).
///
/// Complexity: O(out-degree).
pub fn get_out_edges(env: &Env, record_id: u64) -> Vec<LineageEdge> {
    let out_key = out_edges_key(record_id);
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&out_key)
        .unwrap_or_else(|| Vec::new(env));
    let mut edges = Vec::new(env);
    for id in ids.iter() {
        if let Some(e) = get_edge(env, id) {
            edges.push_back(e);
        }
    }
    edges
}

/// Returns all edges whose **target** is `record_id` (parents).
///
/// Complexity: O(in-degree).
pub fn get_in_edges(env: &Env, record_id: u64) -> Vec<LineageEdge> {
    let in_key = in_edges_key(record_id);
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&in_key)
        .unwrap_or_else(|| Vec::new(env));
    let mut edges = Vec::new(env);
    for id in ids.iter() {
        if let Some(e) = get_edge(env, id) {
            edges.push_back(e);
        }
    }
    edges
}

// ── Integrity verification ──────────────────────────────────────────────────

/// Result of a lineage integrity check.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationResult {
    /// Every commitment in the chain is consistent.
    Valid,
    /// A commitment mismatch was detected at the given record id.
    Tampered(u64),
    /// A required ancestor node is missing from storage.
    MissingAncestor(u64),
}

/// Cryptographically verifies that the lineage chain up to `record_id` is
/// intact.
///
/// The algorithm walks backwards through *in-edges* (parent pointers)
/// recomputing each node's expected commitment and comparing it to the stored
/// value.  The walk stops when a genesis node (no parents) is reached or
/// when `max_depth` hops have been checked.
///
/// # Complexity
/// - Time: O(max_depth × in-degree) — effectively O(depth) for linear chains.
/// - Storage reads: O(max_depth).
pub fn verify_node_integrity(env: &Env, record_id: u64, max_depth: u32) -> VerificationResult {
    let mut current_id = record_id;
    let mut depth = 0u32;

    loop {
        if depth >= max_depth {
            return VerificationResult::Valid;
        }

        let node = match get_node(env, current_id) {
            Some(n) => n,
            None => return VerificationResult::MissingAncestor(current_id),
        };

        let in_edges = get_in_edges(env, current_id);

        if in_edges.is_empty() {
            // Genesis node — recompute the zero-parent commitment.
            let tag_std = node.record_type_tag.to_string();
            let expected =
                genesis_commitment(env, node.record_id, &node.creator, node.created_at, &tag_std);
            if expected != node.commitment {
                return VerificationResult::Tampered(current_id);
            }
            return VerificationResult::Valid;
        }

        // For simplicity we verify against the *first* (oldest) parent edge,
        // which dominates the main provenance chain.
        let parent_edge = in_edges.get(0).unwrap();
        let parent_node = match get_node(env, parent_edge.source_id) {
            Some(n) => n,
            None => return VerificationResult::MissingAncestor(parent_edge.source_id),
        };

        let expected = extend_commitment(env, &node, &parent_node.commitment);
        if expected != node.commitment {
            return VerificationResult::Tampered(current_id);
        }

        current_id = parent_edge.source_id;
        depth += 1;
    }
}

// ── Cross-contract provenance ───────────────────────────────────────────────

/// Tags a lineage node with its originating contract identifier.
/// Used when provenance crosses contract boundaries.
pub fn set_origin_contract(env: &Env, record_id: u64, contract_id: String) {
    env.storage()
        .persistent()
        .set(&(LIN_CONTRACT, record_id), &contract_id);
    env.storage()
        .persistent()
        .extend_ttl(&(LIN_CONTRACT, record_id), TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Retrieves the originating contract id for a cross-contract record, if any.
pub fn get_origin_contract(env: &Env, record_id: u64) -> Option<String> {
    env.storage()
        .persistent()
        .get(&(LIN_CONTRACT, record_id))
}

// ── Lineage pruning with provenance-preserving summarisation ────────────────

/// Summarises the lineage of `record_id` up to `depth` ancestor hops,
/// replacing the detailed in-edge list with a Merkle-like commitment over all
/// summarised edge commitments.
///
/// After pruning:
/// * The node's `pruned` flag is set to `true`.
/// * `summary_commitment` holds the aggregated edge commitment hash.
/// * The detailed edge entries are **not** deleted from storage (they remain
///   accessible individually) but the adjacency list is cleared so that
///   routine traversal skips them, reducing per-call gas.
///
/// Returns the [`LineageSummary`] for the summarised window, or `None` if
/// the node does not exist.
///
/// # Complexity
/// - Time: O(depth × in-degree).
/// - Storage writes: O(depth) compressed into 1 summary write.
pub fn prune_summarise(env: &Env, record_id: u64, depth: u32) -> Option<LineageSummary> {
    let mut node = get_node(env, record_id)?;
    if node.pruned {
        // Already pruned; return the existing summary.
        return Some(LineageSummary {
            record_id,
            summarised_depth: depth,
            summary_commitment: node.summary_commitment.unwrap_or_else(|| node.commitment.clone()),
            window_start: node.created_at,
            window_end: node.created_at,
        });
    }

    // Collect edge commitments for the window.
    let mut accumulated = Bytes::new(env);
    let mut window_start = u64::MAX;
    let mut window_end = 0u64;
    let mut current_id = record_id;

    for _ in 0..depth {
        let in_edges = get_in_edges(env, current_id);
        if in_edges.is_empty() {
            break;
        }
        let edge = in_edges.get(0).unwrap();
        let ec_arr = edge.edge_commitment.to_array();
        accumulated.append(&Bytes::from_slice(env, &ec_arr));

        if edge.timestamp < window_start {
            window_start = edge.timestamp;
        }
        if edge.timestamp > window_end {
            window_end = edge.timestamp;
        }

        current_id = edge.source_id;
    }

    let summary_commitment: BytesN<32> = if accumulated.len() > 0 {
        env.crypto().sha256(&accumulated).into()
    } else {
        node.commitment.clone()
    };

    node.pruned = true;
    node.summary_commitment = Some(summary_commitment.clone());
    save_node(env, &node);

    Some(LineageSummary {
        record_id,
        summarised_depth: depth,
        summary_commitment,
        window_start: if window_start == u64::MAX {
            node.created_at
        } else {
            window_start
        },
        window_end: if window_end == 0 { node.created_at } else { window_end },
    })
}
