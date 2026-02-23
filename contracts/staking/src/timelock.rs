use soroban_sdk::{contracttype, symbol_short, Address, Env};

// ── Storage key constants ───────────────────────────────────────────────────

/// Counter for generating monotonic unstake request IDs.
const UNSTK_CTR: soroban_sdk::Symbol = symbol_short!("USTK_CTR");

// ── Types ───────────────────────────────────────────────────────────────────

/// An in-flight unstake request sitting in the timelock queue.
///
/// Once `unlock_at` has passed, the staker may call `withdraw` to receive
/// their tokens.  `withdrawn` is set to `true` after a successful withdrawal
/// so that double-withdrawals are rejected without removing the audit record.
#[contracttype]
#[derive(Clone, Debug)]
pub struct UnstakeRequest {
    /// Auto-incremented unique identifier.
    pub id: u64,
    /// The address that initiated the unstake.
    pub staker: Address,
    /// Number of tokens to be returned.
    pub amount: i128,
    /// Ledger timestamp after which withdrawal is permitted.
    pub unlock_at: u64,
    /// Becomes `true` once tokens have been returned to the staker.
    pub withdrawn: bool,
}

// ── Storage helpers ─────────────────────────────────────────────────────────

fn request_key(id: u64) -> (soroban_sdk::Symbol, u64) {
    (symbol_short!("USTK_REQ"), id)
}

/// Persist an `UnstakeRequest`.
pub fn store_request(env: &Env, request: &UnstakeRequest) {
    env.storage()
        .persistent()
        .set(&request_key(request.id), request);
}

/// Retrieve an `UnstakeRequest` by ID, returning `None` when not found.
pub fn get_request(env: &Env, id: u64) -> Option<UnstakeRequest> {
    env.storage().persistent().get(&request_key(id))
}

/// Allocate and return the next request ID (1-based, monotonically increasing).
pub fn next_request_id(env: &Env) -> u64 {
    let current: u64 = env.storage().instance().get(&UNSTK_CTR).unwrap_or(0u64);
    let next = current.saturating_add(1);
    env.storage().instance().set(&UNSTK_CTR, &next);
    next
}
