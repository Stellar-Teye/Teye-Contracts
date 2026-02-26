use soroban_sdk::xdr::ToXdr;
/// # Lineage — on-chain DAG provenance primitives
extern crate alloc;

use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, Env, String, Symbol, Vec};

// ── Storage key prefixes ────────────────────────────────────────────────────

const LIN_NODE: Symbol = symbol_short!("LIN_NODE");
const LIN_EDGE: Symbol = symbol_short!("LIN_EDG");
const LIN_OUTEDG: Symbol = symbol_short!("LIN_OUT");
const LIN_INEDG: Symbol = symbol_short!("LIN_IN");
const LIN_ECTR: Symbol = symbol_short!("LIN_ECTR");
const LIN_CONTRACT: Symbol = symbol_short!("LIN_CTR");

pub const TTL_THRESHOLD: u32 = 5_184_000;
pub const TTL_EXTEND_TO: u32 = 10_368_000;
pub const MAX_EDGES_PER_NODE: u32 = 256;

// ── Relationship kinds ──────────────────────────────────────────────────────

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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageNode {
    pub record_id: u64,
    pub creator: Address,
    pub created_at: u64,
    pub record_type_tag: String,
    pub commitment: BytesN<32>,
    pub pruned: bool,
    pub summary_commitment: Option<BytesN<32>>,
    pub origin_contract: Option<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageEdge {
    pub edge_id: u64,
    pub source_id: u64,
    pub target_id: u64,
    pub kind: u32,
    pub actor: Address,
    pub timestamp: u64,
    pub metadata_hash: Option<BytesN<32>>,
    pub edge_commitment: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageSummary {
    pub record_id: u64,
    pub summarised_depth: u32,
    pub summary_commitment: BytesN<32>,
    pub window_start: u64,
    pub window_end: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraversalNode {
    pub node: LineageNode,
    pub depth: u32,
    pub via_edge: Vec<LineageEdge>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraversalResult {
    pub nodes: Vec<TraversalNode>,
    pub truncated: bool,
    pub total_visited: u32,
}

// ── Commitment helpers ──────────────────────────────────────────────────────

pub fn genesis_commitment(
    env: &Env,
    record_id: u64,
    creator: &Address,
    created_at: u64,
    tag: &str,
) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&Bytes::from_slice(env, &record_id.to_be_bytes()));
    
    // FIX: Clone the creator so to_xdr can consume it
    buf.append(&creator.clone().to_xdr(env));

    buf.append(&Bytes::from_slice(env, &created_at.to_be_bytes()));
    buf.append(&Bytes::from_slice(env, tag.as_bytes()));
    buf.append(&Bytes::from_slice(env, &[0u8; 32])); 
    env.crypto().sha256(&buf).into()
}

pub fn extend_commitment(
    env: &Env,
    node: &LineageNode,
    parent_commitment: &BytesN<32>,
) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&Bytes::from_slice(env, &node.record_id.to_be_bytes()));
    
    // FIX: Clone node.creator to avoid moving out of shared reference
    buf.append(&node.creator.clone().to_xdr(env));
    buf.append(&Bytes::from_slice(env, &node.created_at.to_be_bytes()));
    
    let tag_bytes: Bytes = node.record_type_tag.clone().into();
    buf.append(&tag_bytes);

    buf.append(&parent_commitment.clone().into());

    env.crypto().sha256(&buf).into()
}

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
    
    // FIX: Clone actor to avoid moving out of reference
    buf.append(&actor.clone().to_xdr(env));
    
    buf.append(&Bytes::from_slice(env, &timestamp.to_be_bytes()));
    buf.append(&Bytes::from_slice(env, &edge_id.to_be_bytes()));
    env.crypto().sha256(&buf).into()
}

// ── Storage helpers ─────────────────────────────────────────────────────────

fn node_key(record_id: u64) -> (Symbol, u64) { (LIN_NODE, record_id) }
fn edge_key(edge_id: u64) -> (Symbol, u64) { (LIN_EDGE, edge_id) }
fn out_edges_key(record_id: u64) -> (Symbol, u64) { (LIN_OUTEDG, record_id) }
fn in_edges_key(record_id: u64) -> (Symbol, u64) { (LIN_INEDG, record_id) }

fn extend_node_ttl(env: &Env, record_id: u64) {
    env.storage().persistent().extend_ttl(&node_key(record_id), TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Node/Edge CRUD ──────────────────────────────────────────────────────────

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

pub fn get_node(env: &Env, record_id: u64) -> Option<LineageNode> {
    let key = node_key(record_id);
    let node: Option<LineageNode> = env.storage().persistent().get(&key);
    if node.is_some() { extend_node_ttl(env, record_id); }
    node
}

pub fn save_node(env: &Env, node: &LineageNode) {
    env.storage().persistent().set(&node_key(node.record_id), node);
    extend_node_ttl(env, node.record_id);
}

fn next_edge_id(env: &Env) -> u64 {
    let current: u64 = env.storage().persistent().get(&LIN_ECTR).unwrap_or(0);
    let next = current.saturating_add(1);
    env.storage().persistent().set(&LIN_ECTR, &next);
    next
}

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
    let edge_commitment = compute_edge_commitment(env, source_id, target_id, &kind, &actor, now, edge_id);

    let edge = LineageEdge {
        edge_id, source_id, target_id, kind: kind.discriminant(),
        actor, timestamp: now, metadata_hash, edge_commitment,
    };

    env.storage().persistent().set(&edge_key(edge_id), &edge);
    
    let out_key = out_edges_key(source_id);
    let mut out_list: Vec<u64> = env.storage().persistent().get(&out_key).unwrap_or_else(|| Vec::new(env));
    if out_list.len() < MAX_EDGES_PER_NODE {
        out_list.push_back(edge_id);
        env.storage().persistent().set(&out_key, &out_list);
    }

    let in_key = in_edges_key(target_id);
    let mut in_list: Vec<u64> = env.storage().persistent().get(&in_key).unwrap_or_else(|| Vec::new(env));
    if in_list.len() < MAX_EDGES_PER_NODE {
        in_list.push_back(edge_id);
        env.storage().persistent().set(&in_key, &in_list);
    }

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

pub fn get_edge(env: &Env, edge_id: u64) -> Option<LineageEdge> {
    env.storage().persistent().get(&edge_key(edge_id))
}

pub fn get_in_edges(env: &Env, record_id: u64) -> Vec<LineageEdge> {
    let ids: Vec<u64> = env.storage().persistent().get(&in_edges_key(record_id)).unwrap_or_else(|| Vec::new(env));
    let mut edges = Vec::new(env);
    for id in ids.iter() {
        if let Some(e) = get_edge(env, id) { edges.push_back(e); }
    }
    edges
}

// ── Integrity verification ──────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationResult {
    Valid,
    Tampered(u64),
    MissingAncestor(u64),
}

pub fn verify_node_integrity(env: &Env, record_id: u64, max_depth: u32) -> VerificationResult {
    let mut current_id = record_id;
    let mut depth = 0u32;

    loop {
        if depth >= max_depth { return VerificationResult::Valid; }
        let node = match get_node(env, current_id) {
            Some(n) => n,
            None => return VerificationResult::MissingAncestor(current_id),
        };

        let in_edges = get_in_edges(env, current_id);
        if in_edges.is_empty() {
            let mut buf = alloc::vec::Vec::with_capacity(node.record_type_tag.len() as usize);
            buf.resize(node.record_type_tag.len() as usize, 0);
            node.record_type_tag.copy_into_slice(&mut buf);
            let tag_str = core::str::from_utf8(&buf).unwrap_or("");

            // FIX: Clone creator for to_xdr inside genesis_commitment helper
            let expected = genesis_commitment(env, node.record_id, &node.creator, node.created_at, tag_str);
            return if expected == node.commitment { VerificationResult::Valid } else { VerificationResult::Tampered(current_id) };
        }

        let parent_edge = in_edges.get(0).unwrap();
        let parent_node = match get_node(env, parent_edge.source_id) {
            Some(n) => n,
            None => return VerificationResult::MissingAncestor(parent_edge.source_id),
        };

        let expected = extend_commitment(env, &node, &parent_node.commitment);
        if expected != node.commitment { return VerificationResult::Tampered(current_id); }

        current_id = parent_edge.source_id;
        depth += 1;
    }
}

// ── Lineage pruning ──────────────────────────────────────────────────────────

pub fn prune_summarise(env: &Env, record_id: u64, depth: u32) -> Option<LineageSummary> {
    let mut node = get_node(env, record_id)?;
    if node.pruned {
        return Some(LineageSummary {
            record_id,
            summarised_depth: depth,
            summary_commitment: node.summary_commitment.clone().unwrap_or_else(|| node.commitment.clone()),
            window_start: node.created_at,
            window_end: node.created_at,
        });
    }

    let mut accumulated = Bytes::new(env);
    let mut window_start = u64::MAX;
    let mut window_end = 0u64;
    let mut current_id = record_id;

    for _ in 0..depth {
        let in_edges = get_in_edges(env, current_id);
        if in_edges.is_empty() { break; }
        let edge = in_edges.get(0).unwrap();
        accumulated.append(&edge.edge_commitment.clone().into());

        if edge.timestamp < window_start { window_start = edge.timestamp; }
        if edge.timestamp > window_end { window_end = edge.timestamp; }
        current_id = edge.source_id;
    }

    let summary_commitment: BytesN<32> = if !accumulated.is_empty() {
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
        window_start: if window_start == u64::MAX { node.created_at } else { window_start },
        window_end: if window_end == 0 { node.created_at } else { window_end },
    })
}