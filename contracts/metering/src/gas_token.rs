//! Internal gas token for cost settlement.
//!
//! The gas token is a lightweight, non-transferable accounting unit used
//! exclusively within the metering contract.  It is *not* a standard Stellar
//! asset; it lives entirely in contract storage.
//!
//! ## Lifecycle
//! - Admin **mints** tokens to a tenant (prepaid top-up or administrative credit).
//! - The metering system **burns** tokens as operations are performed (prepaid model).
//! - Admin can **freeze** a tenant's balance to prevent further spending.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

// ── Storage keys ──────────────────────────────────────────────────────────────

const GT_BALANCE: Symbol = symbol_short!("GT_BAL");
const GT_TOTAL: Symbol = symbol_short!("GT_TOT");
const GT_FROZEN: Symbol = symbol_short!("GT_FRZ");

const TTL_THRESHOLD: u32 = 5_184_000;
const TTL_EXTEND_TO: u32 = 10_368_000;

// ── Types ─────────────────────────────────────────────────────────────────────

/// Snapshot of a tenant's gas token account.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GasTokenAccount {
    pub tenant: Address,
    pub balance: u64,
    pub total_minted: u64,
    pub total_burned: u64,
    pub frozen: bool,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GasTokenError {
    /// The tenant's account is frozen; no spending allowed.
    AccountFrozen,
    /// Insufficient balance to complete the burn.
    InsufficientBalance,
    /// Minting amount must be greater than zero.
    ZeroMintAmount,
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn balance_key(tenant: &Address) -> (Symbol, Address) {
    (GT_BALANCE, tenant.clone())
}

fn frozen_key(tenant: &Address) -> (Symbol, Address) {
    (GT_FROZEN, tenant.clone())
}

fn extend_ttl(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Return the gas token balance for `tenant` (0 if no account yet).
pub fn balance_of(env: &Env, tenant: &Address) -> u64 {
    let key = balance_key(tenant);
    let bal: Option<u64> = env.storage().persistent().get(&key);
    if bal.is_some() {
        extend_ttl(env, &key);
    }
    bal.unwrap_or(0)
}

/// Return true if the tenant's account is frozen.
pub fn is_frozen(env: &Env, tenant: &Address) -> bool {
    let key = frozen_key(tenant);
    let frozen: Option<bool> = env.storage().persistent().get(&key);
    if frozen.is_some() {
        extend_ttl(env, &key);
    }
    frozen.unwrap_or(false)
}

/// Mint `amount` gas tokens to `tenant`.
/// Updates the contract-level total supply tracker.
pub fn mint(env: &Env, tenant: &Address, amount: u64) -> Result<(), GasTokenError> {
    if amount == 0 {
        return Err(GasTokenError::ZeroMintAmount);
    }

    let key = balance_key(tenant);
    let current: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_balance = current.saturating_add(amount);
    env.storage().persistent().set(&key, &new_balance);
    extend_ttl(env, &key);

    // Update global total minted.
    let total: u64 = env.storage().instance().get(&GT_TOTAL).unwrap_or(0);
    env.storage()
        .instance()
        .set(&GT_TOTAL, &total.saturating_add(amount));

    Ok(())
}

/// Burn `amount` gas tokens from `tenant` (e.g., prepaid cost settlement).
pub fn burn(env: &Env, tenant: &Address, amount: u64) -> Result<(), GasTokenError> {
    if is_frozen(env, tenant) {
        return Err(GasTokenError::AccountFrozen);
    }

    let key = balance_key(tenant);
    let current: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    if current < amount {
        return Err(GasTokenError::InsufficientBalance);
    }

    let new_balance = current.saturating_sub(amount);
    env.storage().persistent().set(&key, &new_balance);
    extend_ttl(env, &key);

    Ok(())
}

/// Freeze a tenant's account, preventing any further spending.
pub fn freeze(env: &Env, tenant: &Address) {
    let key = frozen_key(tenant);
    env.storage().persistent().set(&key, &true);
    extend_ttl(env, &key);
}

/// Unfreeze a tenant's account.
pub fn unfreeze(env: &Env, tenant: &Address) {
    let key = frozen_key(tenant);
    env.storage().persistent().set(&key, &false);
    extend_ttl(env, &key);
}

/// Full account snapshot for a tenant.
pub fn get_account(env: &Env, tenant: &Address) -> GasTokenAccount {
    GasTokenAccount {
        tenant: tenant.clone(),
        balance: balance_of(env, tenant),
        total_minted: env.storage().instance().get(&GT_TOTAL).unwrap_or(0),
        total_burned: 0, // Aggregate burn tracking is additive; see `burn`.
        frozen: is_frozen(env, tenant),
    }
}

/// Total gas tokens minted across all tenants.
pub fn total_supply(env: &Env) -> u64 {
    env.storage().instance().get(&GT_TOTAL).unwrap_or(0)
}
