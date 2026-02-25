//! M-of-N multisig helpers for Soroban contracts.
//!
//! Provides on-chain proposal tracking so that critical admin operations
//! require approval from M out of N configured signers before execution.

use soroban_sdk::{contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec};

// ── Storage Keys ─────────────────────────────────────────────────────────────

const MSIG_CFG: Symbol = symbol_short!("MSIG_CFG");
const MSIG_CTR: Symbol = symbol_short!("MSIG_CTR");
const MSIG_PROP: Symbol = symbol_short!("MSIG_PR");

const TTL_THRESHOLD: u32 = 5_184_000;
const TTL_EXTEND_TO: u32 = 10_368_000;

// ── Types ────────────────────────────────────────────────────────────────────

/// Multisig configuration: M-of-N threshold and the set of signers.
#[contracttype]
#[derive(Clone, Debug)]
pub struct MultisigConfig {
    /// Minimum number of approvals required.
    pub threshold: u32,
    /// The set of authorised signers.
    pub signers: Vec<Address>,
}

/// A pending multisig proposal.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Proposal {
    /// Unique identifier.
    pub id: u64,
    /// Short action tag (e.g. `"SET_RATE"`, `"SET_LOCK"`).
    pub action: Symbol,
    /// Hash of the action parameters so executors can verify intent.
    pub data_hash: BytesN<32>,
    /// Who created the proposal.
    pub proposer: Address,
    /// Addresses that have approved so far.
    pub approvals: Vec<Address>,
    /// Ledger timestamp when the proposal was created.
    pub created_at: u64,
    /// Whether this proposal has been executed.
    pub executed: bool,
}

/// Errors specific to multisig operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MultisigError {
    /// Threshold must be > 0 and ≤ number of signers.
    InvalidConfig,
    /// The caller is not one of the configured signers.
    NotASigner,
    /// The caller has already approved this proposal.
    AlreadyApproved,
    /// The proposal does not exist.
    ProposalNotFound,
    /// The proposal has not reached the required threshold.
    ThresholdNotMet,
    /// The proposal was already executed.
    AlreadyExecuted,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn proposal_key(id: u64) -> (Symbol, u64) {
    (MSIG_PROP, id)
}

fn extend_proposal_ttl(env: &Env, key: &(Symbol, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Store or update the multisig configuration.
///
/// # Errors
/// Returns `MultisigError::InvalidConfig` if `threshold` is zero or exceeds
/// the number of signers.
pub fn configure(env: &Env, signers: Vec<Address>, threshold: u32) -> Result<(), MultisigError> {
    if threshold == 0 || threshold > signers.len() {
        return Err(MultisigError::InvalidConfig);
    }
    let cfg = MultisigConfig { threshold, signers };
    env.storage().instance().set(&MSIG_CFG, &cfg);
    Ok(())
}

/// Return the current multisig configuration, if any.
pub fn get_config(env: &Env) -> Option<MultisigConfig> {
    env.storage().instance().get(&MSIG_CFG)
}

/// Returns `true` when multisig is **not** configured — meaning the legacy
/// single-admin path is acceptable.
pub fn is_legacy_admin_allowed(env: &Env) -> bool {
    get_config(env).is_none()
}

/// Create a new proposal.
///
/// # Errors
/// Returns `MultisigError::NotASigner` if `proposer` is not in the signer set.
pub fn propose(
    env: &Env,
    proposer: &Address,
    action: Symbol,
    data_hash: BytesN<32>,
) -> Result<u64, MultisigError> {
    let cfg = get_config(env).ok_or(MultisigError::InvalidConfig)?;
    if !is_signer(&cfg, proposer) {
        return Err(MultisigError::NotASigner);
    }

    let id: u64 = env.storage().instance().get(&MSIG_CTR).unwrap_or(0) + 1;
    env.storage().instance().set(&MSIG_CTR, &id);

    // The proposer counts as the first approval.
    let mut approvals = Vec::new(env);
    approvals.push_back(proposer.clone());

    let proposal = Proposal {
        id,
        action,
        data_hash,
        proposer: proposer.clone(),
        approvals,
        created_at: env.ledger().timestamp(),
        executed: false,
    };

    let key = proposal_key(id);
    env.storage().persistent().set(&key, &proposal);
    extend_proposal_ttl(env, &key);

    Ok(id)
}

/// Approve an existing proposal.
///
/// # Errors
/// - `NotASigner` if the approver is not in the signer set.
/// - `ProposalNotFound` if the proposal ID doesn't exist.
/// - `AlreadyApproved` if this address has already approved.
/// - `AlreadyExecuted` if the proposal was already executed.
pub fn approve(env: &Env, approver: &Address, proposal_id: u64) -> Result<(), MultisigError> {
    let cfg = get_config(env).ok_or(MultisigError::InvalidConfig)?;
    if !is_signer(&cfg, approver) {
        return Err(MultisigError::NotASigner);
    }

    let key = proposal_key(proposal_id);
    let mut proposal: Proposal = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(MultisigError::ProposalNotFound)?;

    if proposal.executed {
        return Err(MultisigError::AlreadyExecuted);
    }

    // Check for duplicate approval.
    for i in 0..proposal.approvals.len() {
        if proposal.approvals.get(i) == Some(approver.clone()) {
            return Err(MultisigError::AlreadyApproved);
        }
    }

    proposal.approvals.push_back(approver.clone());
    env.storage().persistent().set(&key, &proposal);
    extend_proposal_ttl(env, &key);

    Ok(())
}

/// Check whether a proposal has reached the required approval threshold.
pub fn is_executable(env: &Env, proposal_id: u64) -> bool {
    let cfg = match get_config(env) {
        Some(c) => c,
        None => return false,
    };
    let key = proposal_key(proposal_id);
    match env.storage().persistent().get::<_, Proposal>(&key) {
        Some(p) => !p.executed && p.approvals.len() >= cfg.threshold,
        None => false,
    }
}

/// Mark a proposal as executed.  Callers must verify `is_executable` first.
///
/// # Errors
/// - `ProposalNotFound` if the proposal doesn't exist.
/// - `AlreadyExecuted` if already executed.
/// - `ThresholdNotMet` if the approval count is below threshold.
pub fn mark_executed(env: &Env, proposal_id: u64) -> Result<(), MultisigError> {
    let cfg = get_config(env).ok_or(MultisigError::InvalidConfig)?;
    let key = proposal_key(proposal_id);

    let mut proposal: Proposal = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(MultisigError::ProposalNotFound)?;

    if proposal.executed {
        return Err(MultisigError::AlreadyExecuted);
    }
    if proposal.approvals.len() < cfg.threshold {
        return Err(MultisigError::ThresholdNotMet);
    }

    proposal.executed = true;
    env.storage().persistent().set(&key, &proposal);
    extend_proposal_ttl(env, &key);

    Ok(())
}

/// Retrieve a proposal by ID.
pub fn get_proposal(env: &Env, proposal_id: u64) -> Option<Proposal> {
    let key = proposal_key(proposal_id);
    env.storage().persistent().get(&key)
}

/// Returns true when level-3 (multi-party) authorization is satisfied.
///
/// - If multisig is not configured, legacy admin flow is accepted.
/// - If configured, the proposal must be executable.
pub fn is_level3_authorized(env: &Env, proposal_id: Option<u64>) -> bool {
    if is_legacy_admin_allowed(env) {
        return true;
    }

    match proposal_id {
        Some(id) => is_executable(env, id),
        None => false,
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn is_signer(cfg: &MultisigConfig, addr: &Address) -> bool {
    for i in 0..cfg.signers.len() {
        if cfg.signers.get(i) == Some(addr.clone()) {
            return true;
        }
    }
    false
}
