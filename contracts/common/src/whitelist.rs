use soroban_sdk::{symbol_short, Address, Env, Symbol};

const WL_ENABLED: Symbol = symbol_short!("WL_EN");
const WL_ADDR: Symbol = symbol_short!("WL_ADR");
const WL_TTL_THRESHOLD: u32 = 5_184_000; // 5,184,000 ledgers ~= 300 days (@ ~5s/ledger)
const WL_TTL_EXTEND_TO: u32 = 10_368_000; // 10,368,000 ledgers ~= 600 days (@ ~5s/ledger)

fn extend_whitelist_ttl(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, WL_TTL_THRESHOLD, WL_TTL_EXTEND_TO);
}

fn extend_whitelist_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(WL_TTL_THRESHOLD, WL_TTL_EXTEND_TO);
}

/// Enables or disables whitelist enforcement globally for the contract.
pub fn set_whitelist_enabled(env: &Env, enabled: bool) {
    env.storage().instance().set(&WL_ENABLED, &enabled);
    extend_whitelist_instance_ttl(env);
}

/// Returns whether whitelist enforcement is globally enabled.
pub fn is_whitelist_enabled(env: &Env) -> bool {
    let enabled = env.storage().instance().get(&WL_ENABLED).unwrap_or(false);
    if enabled {
        extend_whitelist_instance_ttl(env);
    }
    enabled
}

/// Adds an address to the whitelist.
pub fn add_to_whitelist(env: &Env, address: &Address) {
    let key = (WL_ADDR, address.clone());
    env.storage().persistent().set(&key, &true);
    extend_whitelist_ttl(env, &key);
}

/// Removes an address from the whitelist.
pub fn remove_from_whitelist(env: &Env, address: &Address) {
    env.storage()
        .persistent()
        .remove(&(WL_ADDR, address.clone()));
}

/// Returns whether an address is in the whitelist.
pub fn is_whitelisted(env: &Env, address: &Address) -> bool {
    let key = (WL_ADDR, address.clone());
    let is_whitelisted = env.storage().persistent().get(&key).unwrap_or(false);
    if is_whitelisted {
        extend_whitelist_ttl(env, &key);
    }
    is_whitelisted
}

/// Returns whether the address is allowed to call guarded functions.
///
/// When whitelist enforcement is disabled, all addresses are allowed.
pub fn check_whitelist_access(env: &Env, address: &Address) -> bool {
    !is_whitelist_enabled(env) || is_whitelisted(env, address)
}
