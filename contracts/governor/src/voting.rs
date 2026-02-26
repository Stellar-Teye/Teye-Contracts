//! Quadratic voting with commit-reveal scheme.
//!
//! ## Vote power formula
//! ```text
//! raw_power     = sqrt(staked_tokens)
//! loyalty_mult  = 1.0 + min(stake_age_days / 365, 1.0)   // up to 2×
//! final_power   = raw_power × loyalty_mult                 // scaled ×1000
//! ```
//! All values are integer-scaled by SCALE (1 000) to avoid floating point.

use soroban_sdk::{contracttype, symbol_short, Address, BytesN, Env, Symbol};

// ── Storage key prefixes ─────────────────────────────────────────────────────

const COMMIT: Symbol = symbol_short!("COMMIT");
const VOTED: Symbol = symbol_short!("VOTED");

// TTL: ~30 days
const TTL_THRESHOLD: u32 = 518_400;
const TTL_EXTEND_TO: u32 = 1_036_800;

/// Integer scale factor applied to vote power.
pub const SCALE: i128 = 1_000;
/// Seconds in one day.
pub const SECS_PER_DAY: u64 = 86_400;
/// Maximum loyalty bonus: 100 % (doubles raw power after 1 year).
pub const MAX_LOYALTY_DAYS: u64 = 365;

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoteChoice {
    For,
    Against,
    Veto,
}

/// A committed (unrevealed) vote.
#[contracttype]
#[derive(Clone, Debug)]
pub struct VoteCommit {
    /// SHA-256( proposal_id || voter || choice || salt )
    pub commitment: BytesN<32>,
    pub committed_at: u64,
}

/// A fully revealed vote record kept to prevent double-voting.
#[contracttype]
#[derive(Clone, Debug)]
pub struct VoteRecord {
    pub voter: Address,
    pub choice: VoteChoice,
    pub vote_power: i128,
    pub revealed_at: u64,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn commit_key(proposal_id: u64, voter: &Address) -> (Symbol, u64, Address) {
    (COMMIT, proposal_id, voter.clone())
}

fn voted_key(proposal_id: u64, voter: &Address) -> (Symbol, u64, Address) {
    (VOTED, proposal_id, voter.clone())
}

pub(crate) fn store_commit(env: &Env, proposal_id: u64, voter: &Address, commit: &VoteCommit) {
    let key = commit_key(proposal_id, voter);
    env.storage().persistent().set(&key, commit);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

pub(crate) fn load_commit(env: &Env, proposal_id: u64, voter: &Address) -> Option<VoteCommit> {
    env.storage().persistent().get(&commit_key(proposal_id, voter))
}

pub(crate) fn store_vote(env: &Env, proposal_id: u64, voter: &Address, record: &VoteRecord) {
    let key = voted_key(proposal_id, voter);
    env.storage().persistent().set(&key, record);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

pub(crate) fn has_voted(env: &Env, proposal_id: u64, voter: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&voted_key(proposal_id, voter))
}

pub(crate) fn has_committed(env: &Env, proposal_id: u64, voter: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&commit_key(proposal_id, voter))
}

// ── Vote power computation ────────────────────────────────────────────────────

/// Integer square-root via Newton's method (no_std compatible).
///
/// Returns floor(sqrt(n)) for n >= 0.
pub fn isqrt(n: i128) -> i128 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Compute the loyalty multiplier (scaled by SCALE).
///
/// Multiplier grows linearly from 1× to 2× over `MAX_LOYALTY_DAYS` days.
///
/// ```text
/// loyalty_scaled = SCALE + min(stake_age_days, 365) * SCALE / 365
/// ```
pub fn loyalty_multiplier_scaled(stake_age_secs: u64) -> i128 {
    let days = (stake_age_secs / SECS_PER_DAY) as i128;
    let capped = days.min(MAX_LOYALTY_DAYS as i128);
    SCALE + capped * SCALE / (MAX_LOYALTY_DAYS as i128)
}

/// Compute final vote power for a voter.
///
/// `staked`         — raw token balance (e.g. stroops).
/// `stake_age_secs` — seconds since the voter first staked.
///
/// Returns an integer scaled by SCALE² to preserve precision through
/// multiplication; callers should use this value directly for tallying
/// as long as they compare apples-to-apples.
pub fn compute_vote_power(staked: i128, stake_age_secs: u64) -> i128 {
    let raw = isqrt(staked); // sqrt(tokens)
    let loyalty = loyalty_multiplier_scaled(stake_age_secs); // SCALE-scaled
    // raw × loyalty / SCALE  — keeps result in "SCALE-scaled sqrt-token" units
    raw.saturating_mul(loyalty) / SCALE
}
