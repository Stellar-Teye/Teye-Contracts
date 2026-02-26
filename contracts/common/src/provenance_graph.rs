//! # Provenance Graph — high-level traversal, access control, and export
//!
//! Builds on [`crate::lineage`] to provide:
//!
//! * **Ancestor / descendant traversal** — BFS with configurable depth bound.
//! * **Origin tracing** — walk the DAG to the genesis node.
//! * **Lineage-based access control** — if a caller has access to a *derived*
//!   record, they also have access to its *source* records (transitively).
//! * **Lineage verification gateway** — wraps [`lineage::verify_node_integrity`]
//!   for public consumption.
//! * **DAG export** — serialises the visible subgraph as a deterministic JSON-
//!   compatible [`ProvenanceExport`] that can be consumed by off-chain tooling.
//!
//! ## Algorithm notes
//!
//! All graph algorithms are iterative (no recursion) and operate within
//! Soroban's instruction budget by enforcing a `max_depth` / `max_nodes` cap.
//! The visited-set is implemented as a sorted `Vec<u64>` with binary-search
//! insertion, giving O(n log n) deduplication without a hash map (no `std`).

use soroban_sdk::{contracttype, Address, Env, String, Vec};

use crate::lineage::{
    get_in_edges, get_node, get_out_edges, verify_node_integrity, LineageEdge, LineageNode,
    RelationshipKind, TraversalNode, TraversalResult, VerificationResult,
};

// ── Constants ───────────────────────────────────────────────────────────────

/// Hard cap on BFS nodes to protect the instruction budget.
pub const MAX_BFS_NODES: u32 = 128;
/// Hard cap on BFS depth.
pub const MAX_BFS_DEPTH: u32 = 32;

// ── Traversal result types ──────────────────────────────────────────────────


/// Compact DAG export suitable for off-chain visualisation tools.
/// Edges are encoded as `(source_id, target_id, kind_u32, edge_id)` tuples.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvenanceExport {
    /// Root record for this export.
    pub root_record_id: u64,
    /// All node IDs included in the export.
    pub node_ids: Vec<u64>,
    /// All edges: each entry is `(edge_id, source_id, target_id, kind_discriminant)`.
    pub edges: Vec<(u64, u64, u64, u32)>,
    /// Depth reached by the export traversal.
    pub depth_reached: u32,
}

/// Result of a lineage-based access check.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LineageAccessResult {
    /// Access granted because the record is the requested one.
    DirectMatch,
    /// Access granted because the caller has access to a derived descendant.
    InheritedFromDescendant(u64),
    /// Access denied — no lineage path found with access.
    Denied,
}

// ── Internal visited-set (no_std BTreeSet replacement) ─────────────────────

/// Inserts `id` into `sorted_vec` if not already present.  Returns `true` if
/// it was newly inserted.  O(log n) search + O(n) shift in worst case, but
/// n ≤ MAX_BFS_NODES so the constant factor is tiny.
#[inline]
fn visit(sorted_vec: &mut Vec<u64>, id: u64) -> bool {
    // Linear scan is fine up to MAX_BFS_NODES = 128.
    for i in 0..sorted_vec.len() {
        if sorted_vec.get(i) == Some(id) {
            return false; // already visited
        }
    }
    sorted_vec.push_back(id);
    true
}

// ── Ancestor traversal (walk towards roots) ─────────────────────────────────

/// Traverses ancestor nodes of `start_id` using BFS, moving along *in-edges*
/// (from child → parent direction).
///
/// Returns all ancestors reachable within `max_depth` hops and `MAX_BFS_NODES`
/// total nodes.
///
/// # Complexity
/// - Time: O(min(N, MAX_BFS_NODES) × avg_in_degree).
/// - Space: O(frontier + visited) = O(MAX_BFS_NODES).
pub fn trace_ancestors(env: &Env, start_id: u64, max_depth: u32) -> TraversalResult {
    let depth_cap = max_depth.min(MAX_BFS_DEPTH);
    let mut result_nodes: Vec<TraversalNode> = Vec::new(env);
    let mut visited: Vec<u64> = Vec::new(env);
    let mut truncated = false;

    // BFS frontier: (record_id, depth, optional_via_edge)
    // We simulate a queue using two Vecs (current level / next level).
    let mut current_frontier: Vec<(u64, u32, Option<LineageEdge>)> = Vec::new(env);
    current_frontier.push_back((start_id, 0, None));

    while current_frontier.len() > 0 {
        let mut next_frontier: Vec<(u64, u32, Option<LineageEdge>)> = Vec::new(env);

        for i in 0..current_frontier.len() {
            let (rec_id, depth, via_edge) = current_frontier.get(i).unwrap();

            if !visit(&mut visited, rec_id) {
                continue;
            }

            if result_nodes.len() >= MAX_BFS_NODES {
                truncated = true;
                break;
            }

            if let Some(node) = get_node(env, rec_id) {
                let mut via_vec = Vec::new(env);
                if let Some(e) = via_edge {
                    via_vec.push_back(e);
                }
                result_nodes.push_back(TraversalNode {
                    node,
                    depth,
                    via_edge: via_vec,
                });
            }

            if depth < depth_cap {
                let in_edges = get_in_edges(env, rec_id);
                for j in 0..in_edges.len() {
                    let edge = in_edges.get(j).unwrap();
                    next_frontier.push_back((edge.source_id, depth + 1, Some(edge)));
                }
            }
        }

        if truncated {
            break;
        }

        current_frontier = next_frontier;
    }

    let total = result_nodes.len();
    TraversalResult {
        nodes: result_nodes,
        truncated,
        total_visited: total,
    }
}

/// Traces all descendants of `start_id` using out-edges (parent → child).
///
/// # Complexity: same as [`trace_ancestors`].
pub fn trace_descendants(env: &Env, start_id: u64, max_depth: u32) -> TraversalResult {
    let depth_cap = max_depth.min(MAX_BFS_DEPTH);
    let mut result_nodes: Vec<TraversalNode> = Vec::new(env);
    let mut visited: Vec<u64> = Vec::new(env);
    let mut truncated = false;

    let mut current_frontier: Vec<(u64, u32, Option<LineageEdge>)> = Vec::new(env);
    current_frontier.push_back((start_id, 0, None));

    while current_frontier.len() > 0 {
        let mut next_frontier: Vec<(u64, u32, Option<LineageEdge>)> = Vec::new(env);

        for i in 0..current_frontier.len() {
            let (rec_id, depth, via_edge) = current_frontier.get(i).unwrap();

            if !visit(&mut visited, rec_id) {
                continue;
            }

            if result_nodes.len() >= MAX_BFS_NODES {
                truncated = true;
                break;
            }

            if let Some(node) = get_node(env, rec_id) {
                let mut via_vec = Vec::new(env);
                if let Some(e) = via_edge {
                    via_vec.push_back(e);
                }
                result_nodes.push_back(TraversalNode {
                    node,
                    depth,
                    via_edge: via_vec,
                });
            }

            if depth < depth_cap {
                let out_edges = get_out_edges(env, rec_id);
                for j in 0..out_edges.len() {
                    let edge = out_edges.get(j).unwrap();
                    next_frontier.push_back((edge.target_id, depth + 1, Some(edge)));
                }
            }
        }

        if truncated {
            break;
        }

        current_frontier = next_frontier;
    }

    let total = result_nodes.len();
    TraversalResult {
        nodes: result_nodes,
        truncated,
        total_visited: total,
    }
}

// ── Origin tracing ──────────────────────────────────────────────────────────

/// Walks the primary ancestor chain (always following the *first* in-edge)
/// until a genesis node (no parents) is reached.
///
/// Returns the genesis [`LineageNode`] and the hop count.
///
/// # Complexity
/// - Time: O(chain depth).
/// - Terminates after `MAX_BFS_DEPTH` hops even on unexpectedly long chains.
pub fn find_origin(env: &Env, start_id: u64) -> Option<(LineageNode, u32)> {
    let mut current_id = start_id;
    let mut depth = 0u32;

    loop {
        if depth >= MAX_BFS_DEPTH {
            // Return current as best-effort origin.
            return get_node(env, current_id).map(|n| (n, depth));
        }

        let node = get_node(env, current_id)?;
        let in_edges = get_in_edges(env, current_id);

        if in_edges.is_empty() {
            return Some((node, depth));
        }

        let parent_edge = in_edges.get(0).unwrap();
        current_id = parent_edge.source_id;
        depth += 1;
    }
}

// ── Lineage-based access control ────────────────────────────────────────────

/// Determines whether `requester` has lineage-based access to `record_id`.
///
/// The rule: **access to a derived record implies access to all its ancestor
/// source records.**  The implementation walks *descendants* from `record_id`
/// to check whether any descendant node was created by or explicitly shared
/// with `requester`.
///
/// `has_direct_access_fn` is a caller-supplied closure (represented as a
/// simple record-id → bool lookup through contract storage) that checks
/// direct ownership or consent; it is applied to each node in the subgraph.
/// Because Soroban contracts cannot take function pointers, callers must
/// implement the check inline — this function provides the `TraversalResult`
/// so the caller can inspect the node list and apply their own logic.
///
/// # Returns
/// - `DirectMatch` — `record_id` itself matches `requester`'s direct access.
/// - `InheritedFromDescendant(desc_id)` — a descendant grants access.
/// - `Denied` — no access found.
///
/// # Complexity
/// O(descendants × avg_degree) bounded by MAX_BFS_NODES.
pub fn check_lineage_access(
    env: &Env,
    record_id: u64,
    requester: &Address,
    max_depth: u32,
) -> (LineageAccessResult, TraversalResult) {
    // First, check if the node itself belongs to the requester.
    if let Some(node) = get_node(env, record_id) {
        if node.creator == *requester {
            let dummy = TraversalResult {
                nodes: Vec::new(env),
                truncated: false,
                total_visited: 0,
            };
            return (LineageAccessResult::DirectMatch, dummy);
        }
    }

    // Walk descendants to find a node the requester created or was shared with.
    let descendants = trace_descendants(env, record_id, max_depth);

    for i in 0..descendants.nodes.len() {
        let tn = descendants.nodes.get(i).unwrap();
        // Check if requester is the creator of a descendant.
        if tn.node.creator == *requester {
            let desc_id = tn.node.record_id;
            return (
                LineageAccessResult::InheritedFromDescendant(desc_id),
                descendants,
            );
        }
        // Check if any SharedWith edge targets the requester's record.
        if let Some(ref via) = tn.via_edge.get(0) {
            if via.kind == RelationshipKind::SharedWith.discriminant() && via.actor == *requester {
                let desc_id = tn.node.record_id;
                return (
                    LineageAccessResult::InheritedFromDescendant(desc_id),
                    descendants,
                );
            }
        }
    }

    (LineageAccessResult::Denied, descendants)
}

// ── Lineage verification gateway ────────────────────────────────────────────

/// Public gateway for integrity verification, wrapping
/// [`lineage::verify_node_integrity`] and surfacing a friendly result.
pub fn verify_provenance(env: &Env, record_id: u64, max_depth: u32) -> VerificationResult {
    verify_node_integrity(env, record_id, max_depth)
}

// ── DAG export ──────────────────────────────────────────────────────────────

/// Exports the provenance DAG rooted at `record_id` as a
/// [`ProvenanceExport`] suitable for off-chain DAG renderers (e.g. Graphviz,
/// D3, or the accompanying `visualize_lineage.sh` script).
///
/// The export includes both ancestors **and** descendants up to `max_depth`
/// hops, combined into one subgraph view.
///
/// # Complexity
/// O(MAX_BFS_NODES × avg_degree) — bounded.
pub fn export_dag(env: &Env, record_id: u64, max_depth: u32) -> ProvenanceExport {
    let depth_cap = max_depth.min(MAX_BFS_DEPTH);

    // Collect ancestors + descendants.
    let ancestors = trace_ancestors(env, record_id, depth_cap);
    let descendants = trace_descendants(env, record_id, depth_cap);

    let mut node_ids: Vec<u64> = Vec::new(env);
    let mut visited_ids: Vec<u64> = Vec::new(env);

    // Merge both traversal results.
    for i in 0..ancestors.nodes.len() {
        let tn = ancestors.nodes.get(i).unwrap();
        if visit(&mut visited_ids, tn.node.record_id) {
            node_ids.push_back(tn.node.record_id);
        }
    }
    for i in 0..descendants.nodes.len() {
        let tn = descendants.nodes.get(i).unwrap();
        if visit(&mut visited_ids, tn.node.record_id) {
            node_ids.push_back(tn.node.record_id);
        }
    }

    // Collect all edges within the subgraph.
    let mut edges: Vec<(u64, u64, u64, u32)> = Vec::new(env);
    let mut visited_edge_ids: Vec<u64> = Vec::new(env);

    for i in 0..node_ids.len() {
        let nid = node_ids.get(i).unwrap();
        let out_edges = get_out_edges(env, nid);
        for j in 0..out_edges.len() {
            let e = out_edges.get(j).unwrap();
            // Only include edges where both endpoints are in the subgraph.
            let target_in_graph = {
                let mut found = false;
                for k in 0..node_ids.len() {
                    if node_ids.get(k) == Some(e.target_id) {
                        found = true;
                        break;
                    }
                }
                found
            };

            if target_in_graph && visit(&mut visited_edge_ids, e.edge_id) {
                edges.push_back((e.edge_id, e.source_id, e.target_id, e.kind));
            }
        }
    }

    let depth_reached = ancestors.total_visited.max(descendants.total_visited);

    ProvenanceExport {
        root_record_id: record_id,
        node_ids,
        edges,
        depth_reached,
    }
}

/// Collects all unique actors (addresses) that appear on lineage edges within
/// `max_depth` hops of `record_id`.  Useful for lineage-based access control
/// auditing.
pub fn collect_lineage_actors(env: &Env, record_id: u64, max_depth: u32) -> Vec<Address> {
    let ancestors = trace_ancestors(env, record_id, max_depth);
    let mut actors: Vec<Address> = Vec::new(env);

    for i in 0..ancestors.nodes.len() {
        let tn = ancestors.nodes.get(i).unwrap();
        if let Some(ref edge) = tn.via_edge.get(0) {
            // Add actor if not already present (linear scan, bounded by MAX_BFS_NODES).
            let mut already = false;
            for j in 0..actors.len() {
                if actors.get(j) == Some(edge.actor.clone()) {
                    already = true;
                    break;
                }
            }
            if !already {
                actors.push_back(edge.actor.clone());
            }
        }
    }

    actors
}

/// Returns a human-readable description of a [`RelationshipKind`] as a
/// Soroban `String`, suitable for event payloads and export metadata.
pub fn relationship_kind_label(env: &Env, kind: &RelationshipKind) -> String {
    match kind {
        RelationshipKind::Created => String::from_str(env, "created"),
        RelationshipKind::DerivedFrom => String::from_str(env, "derived_from"),
        RelationshipKind::ModifiedBy => String::from_str(env, "modified_by"),
        RelationshipKind::SharedWith => String::from_str(env, "shared_with"),
        RelationshipKind::AggregatedInto => String::from_str(env, "aggregated_into"),
        RelationshipKind::CrossContract => String::from_str(env, "cross_contract"),
    }
}
