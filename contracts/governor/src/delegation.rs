//! Voting power delegation with revocation support.
//!
//! A voter may delegate their vote weight to a representative for all
//! active proposals.  Delegation is revocable at any time; revocation
//! takes effect on proposals that have not yet entered the Voting phase.
//!
//! ## Rules
//! - A voter can delegate to exactly **one** representative at a time.
//! - A representative can **not** re-delegate received power (no chains).
//! - Self-delegation is a no-op (treated as no delegation).
//! - Revoking non-existent delegation is a no-op.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

// ── Storage key prefixes ─────────────────────────────────────────────────────

/// Maps voter → delegate address.
const DELEGATE_TO: Symbol = symbol_short!("DEL_TO");
/// Maps delegate → total delegated raw stake (for display only; vote power is
/// recomputed per proposal using each delegator's own stake age).
const DELEGATE_FROM_CNT: Symbol = symbol_short!("DEL_CNT");

const TTL_THRESHOLD: u32 = 1_036_800;
const TTL_EXTEND_TO: u32 = 2_073_600;

// ── Types ─────────────────────────────────────────────────────────────────────

/// A delegation record stored under the delegator's address.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Delegation {
    /// The address that will cast votes on behalf of the delegator.
    pub delegate: Address,
    /// Ledger timestamp when the delegation was created.
    pub since: u64,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn del_to_key(voter: &Address) -> (Symbol, Address) {
    (DELEGATE_TO, voter.clone())
}

fn del_cnt_key(delegate: &Address) -> (Symbol, Address) {
    (DELEGATE_FROM_CNT, delegate.clone())
}

fn extend(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Set or update the delegation for `voter` → `delegate`.
///
/// Returns `false` if `voter == delegate` (self-delegation ignored).
pub fn set_delegation(env: &Env, voter: &Address, delegate: &Address) -> bool {
    if voter == delegate {
        return false;
    }

    // If there was a previous delegate, decrement their count.
    if let Some(prev) = get_delegation(env, voter) {
        decrement_count(env, &prev.delegate);
    }

    let record = Delegation {
        delegate: delegate.clone(),
        since: env.ledger().timestamp(),
    };

    let key = del_to_key(voter);
    env.storage().persistent().set(&key, &record);
    extend(env, &key);

    increment_count(env, delegate);
    true
}

/// Remove any active delegation for `voter`.
pub fn revoke_delegation(env: &Env, voter: &Address) {
    let key = del_to_key(voter);
    if let Some(prev) = env.storage().persistent().get::<_, Delegation>(&key) {
        decrement_count(env, &prev.delegate);
        env.storage().persistent().remove(&key);
    }
}

/// Return the active delegation for `voter`, if any.
pub fn get_delegation(env: &Env, voter: &Address) -> Option<Delegation> {
    env.storage().persistent().get(&del_to_key(voter))
}

/// Return the number of addresses that have delegated to `delegate`.
pub fn delegation_count(env: &Env, delegate: &Address) -> u32 {
    env.storage()
        .instance()
        .get(&del_cnt_key(delegate))
        .unwrap_or(0u32)
}

/// Check whether `voter` has delegated their vote (to anyone).
pub fn has_delegated(env: &Env, voter: &Address) -> bool {
    env.storage().persistent().has(&del_to_key(voter))
}

/// Check whether `delegate` is voting on behalf of `voter`.
pub fn is_delegate_of(env: &Env, delegate: &Address, voter: &Address) -> bool {
    get_delegation(env, voter)
        .map(|d| d.delegate == *delegate)
        .unwrap_or(false)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn increment_count(env: &Env, delegate: &Address) {
    let key = del_cnt_key(delegate);
    let n: u32 = env.storage().instance().get(&key).unwrap_or(0u32);
    env.storage().instance().set(&key, &n.saturating_add(1));
}

fn decrement_count(env: &Env, delegate: &Address) {
    let key = del_cnt_key(delegate);
    let n: u32 = env.storage().instance().get(&key).unwrap_or(0u32);
    env.storage()
        .instance()
        .set(&key, &n.saturating_sub(1));
}
