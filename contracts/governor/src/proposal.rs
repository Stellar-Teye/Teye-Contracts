//! Proposal types, storage, and lifecycle management for the Governor DAO.

use soroban_sdk::{contracttype, symbol_short, Address, BytesN, Env, String, Symbol, Vec};

// ── Storage key prefixes ─────────────────────────────────────────────────────

pub(crate) const PROPOSAL_CTR: Symbol = symbol_short!("PROP_CTR");
pub(crate) const PROPOSAL: Symbol = symbol_short!("PROP");

// TTL: ~60 days at 5s/ledger
const TTL_THRESHOLD: u32 = 1_036_800;
const TTL_EXTEND_TO: u32 = 2_073_600;

// ── Proposal type ─────────────────────────────────────────────────────────────

/// The kind of change a proposal enacts.
///
/// Each type carries its own quorum requirement and execution path.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalType {
    /// Upgrade a contract to a new WASM hash.
    ContractUpgrade,
    /// Change a numeric or flag parameter in a contract.
    ParameterChange,
    /// Modify an access-control or compliance policy.
    PolicyModification,
    /// Fast-path action for emergencies (reduced timelock).
    EmergencyAction,
    /// Authorise a treasury spend to a target address.
    TreasurySpend,
}

/// Phase of the proposal lifecycle.
///
/// ```text
/// Draft ──► Discussion ──► Voting ──► Timelock ──► Execution
///                                          │
///                                          └──► Rejected  (veto threshold met)
///                                          └──► Completed (executed)
///                                          └──► Expired   (voting period elapsed, quorum not met)
/// ```
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalPhase {
    Draft,
    Discussion,
    Voting,
    Timelock,
    Execution,
    Completed,
    Rejected,
    Expired,
}

/// A single action within a batched proposal.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ProposalAction {
    /// Target contract address.
    pub target: Address,
    /// Short tag identifying the function to call (e.g. `symbol_short!("UPGRADE")`).
    pub function: Symbol,
    /// SHA-256 hash of the encoded call parameters so voters can verify intent.
    pub params_hash: BytesN<32>,
}

/// The full on-chain proposal record.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Proposal {
    pub id: u64,
    pub proposal_type: ProposalType,
    pub phase: ProposalPhase,
    pub proposer: Address,
    /// Human-readable summary stored as a short on-chain string.
    pub title: String,
    /// One or more actions executed atomically when the proposal passes.
    pub actions: Vec<ProposalAction>,
    pub created_at: u64,
    /// Timestamp when Discussion phase ends and Voting begins.
    pub discussion_ends: u64,
    /// Timestamp when Voting phase ends.
    pub voting_ends: u64,
    /// Timestamp when Timelock expires and Execution is allowed.
    pub timelock_ends: u64,
    /// Total quadratic vote weight in favour.
    pub votes_for: i128,
    /// Total quadratic vote weight against.
    pub votes_against: i128,
    /// Total quadratic weight of veto votes (counted against optimistic execution).
    pub votes_veto: i128,
    /// Commit-reveal: hash of committed votes not yet revealed.
    pub commit_count: u32,
    /// Number of revealed votes (for + against + veto).
    pub reveal_count: u32,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

pub(crate) fn next_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&PROPOSAL_CTR)
        .unwrap_or(0u64)
        .saturating_add(1);
    env.storage().instance().set(&PROPOSAL_CTR, &id);
    id
}

pub(crate) fn proposal_key(id: u64) -> (Symbol, u64) {
    (PROPOSAL, id)
}

pub(crate) fn store(env: &Env, proposal: &Proposal) {
    let key = proposal_key(proposal.id);
    env.storage().persistent().set(&key, proposal);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

pub(crate) fn load(env: &Env, id: u64) -> Option<Proposal> {
    env.storage().persistent().get(&proposal_key(id))
}

// ── Quorum constants by proposal type ────────────────────────────────────────

/// Minimum fraction of total vote supply (in basis points, 10 000 = 100 %)
/// that must participate for a proposal to be valid.
pub fn quorum_bps(proposal_type: &ProposalType) -> u32 {
    match proposal_type {
        ProposalType::TreasurySpend => 1_000,      // 10 %
        ProposalType::ParameterChange => 1_500,     // 15 %
        ProposalType::PolicyModification => 2_000,  // 20 %
        ProposalType::ContractUpgrade => 3_000,     // 30 %
        ProposalType::EmergencyAction => 500,       // 5 %  — fast path
    }
}

/// Minimum fraction of revealed votes that must be FOR (in basis points).
pub fn pass_threshold_bps(_proposal_type: &ProposalType) -> u32 {
    5_100 // simple majority for all types (51 %)
}

/// Fraction of total vote supply (bps) needed to trigger an optimistic-veto.
pub fn veto_threshold_bps(proposal_type: &ProposalType) -> u32 {
    match proposal_type {
        ProposalType::EmergencyAction => 3_000, // 30 % can veto emergency
        _ => 3_300,                             // 33 % for everything else
    }
}
