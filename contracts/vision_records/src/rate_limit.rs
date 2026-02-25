use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol, Vec};

// ── Storage keys ──────────────────────────────────────────────
pub(crate) const RATE_LIMIT_CONFIG: Symbol = symbol_short!("RL_CFG");
pub(crate) const RATE_LIMIT_WINDOW: Symbol = symbol_short!("RL_WIN");
pub(crate) const RATE_LIMIT_COUNT: Symbol = symbol_short!("RL_CNT");
pub(crate) const RATE_LIMIT_BYPASS: Symbol = symbol_short!("RL_BYP");


const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

/// Extends the time-to-live (TTL) for rate limit storage keys.
fn extend_ttl_config_key(env: &Env, key: &(Symbol, String)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_ttl_window_key(env: &Env, key: &(Symbol, Address, String)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_ttl_count_key(env: &Env, key: &(Symbol, Address, String)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_ttl_bypass_key(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Types ─────────────────────────────────────────────────────

/// Rate limit configuration for an operation type
#[contracttype]
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    pub max_requests: u32,   // Maximum requests allowed in the window
    pub window_seconds: u64, // Time window in seconds
    pub operation: String,   // Operation name (e.g., "add_record", "get_record")
}

/// Rate limit status for an address
#[contracttype]
#[derive(Clone, Debug)]
pub struct RateLimitStatus {
    pub address: Address,
    pub operation: String,
    pub current_count: u32,
    pub max_requests: u32,
    pub window_seconds: u64,
    pub window_start: u64,
    pub window_end: u64,
    pub reset_at: u64,
}

/// Rate limit statistics for dashboard
#[contracttype]
#[derive(Clone, Debug)]
pub struct RateLimitStats {
    pub total_requests: u64,
    pub rate_limited_requests: u64,
    pub unique_addresses: u64,
    pub top_rate_limited_operations: Vec<String>,
}

// ── Storage Functions ────────────────────────────────────────

/// Gets rate limit configuration for an operation
pub fn get_rate_limit_config(env: &Env, operation: &String) -> Option<RateLimitConfig> {
    let key = (RATE_LIMIT_CONFIG, operation.clone());
    env.storage().persistent().get(&key)
}

/// Sets rate limit configuration for an operation
pub fn set_rate_limit_config(env: &Env, config: &RateLimitConfig) {
    let key = (RATE_LIMIT_CONFIG, config.operation.clone());
    env.storage().persistent().set(&key, config);
    extend_ttl_config_key(env, &key);
}

/// Gets the current rate limit window start time for an address and operation
pub fn get_rate_limit_window(env: &Env, address: &Address, operation: &String) -> u64 {
    let key = (RATE_LIMIT_WINDOW, address.clone(), operation.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(env.ledger().timestamp())
}

/// Sets the rate limit window start time for an address and operation
pub fn set_rate_limit_window(env: &Env, address: &Address, operation: &String, window_start: u64) {
    let key = (RATE_LIMIT_WINDOW, address.clone(), operation.clone());
    env.storage().persistent().set(&key, &window_start);
    extend_ttl_window_key(env, &key);
}

/// Gets the current request count for an address and operation in the current window
pub fn get_rate_limit_count(env: &Env, address: &Address, operation: &String) -> u32 {
    let key = (RATE_LIMIT_COUNT, address.clone(), operation.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Increments the rate limit count for an address and operation
pub fn increment_rate_limit_count(env: &Env, address: &Address, operation: &String) -> u32 {
    let key = (RATE_LIMIT_COUNT, address.clone(), operation.clone());
    let current: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_count = current + 1;
    env.storage().persistent().set(&key, &new_count);
    extend_ttl_count_key(env, &key);
    new_count
}

/// Resets the rate limit count for an address and operation
pub fn reset_rate_limit_count(env: &Env, address: &Address, operation: &String) {
    let key = (RATE_LIMIT_COUNT, address.clone(), operation.clone());
    env.storage().persistent().set(&key, &0u32);
    extend_ttl_count_key(env, &key);
}

/// Checks if an address has rate limit bypass (e.g., verified providers)
pub fn has_rate_limit_bypass(env: &Env, address: &Address) -> bool {
    let key = (RATE_LIMIT_BYPASS, address.clone());
    env.storage().persistent().get(&key).unwrap_or(false)
}

/// Sets rate limit bypass for an address
pub fn set_rate_limit_bypass(env: &Env, address: &Address, bypass: bool) {
    let key = (RATE_LIMIT_BYPASS, address.clone());
    if bypass {
        env.storage().persistent().set(&key, &true);
        extend_ttl_bypass_key(env, &key);
    } else {
        env.storage().persistent().remove(&key);
    }
}

// ── Rate Limit Logic ────────────────────────────────────────

/// Checks if a request should be rate limited
/// Returns (is_allowed, current_count, max_requests, reset_at)
pub fn check_rate_limit(env: &Env, address: &Address, operation: &String) -> (bool, u32, u32, u64) {
    // Check if address has bypass
    if has_rate_limit_bypass(env, address) {
        return (true, 0, 0, 0);
    }

    // Get rate limit configuration
    let config = match get_rate_limit_config(env, operation) {
        Some(cfg) => cfg,
        None => {
            // No rate limit configured for this operation - allow
            return (true, 0, 0, 0);
        }
    };

    let current_time = env.ledger().timestamp();
    let window_start = get_rate_limit_window(env, address, operation);
    let window_end = window_start + config.window_seconds;

    // Check if current window has expired
    if current_time >= window_end {
        // Reset window and count
        set_rate_limit_window(env, address, operation, current_time);
        reset_rate_limit_count(env, address, operation);
        let new_count = increment_rate_limit_count(env, address, operation);
        let new_window_end = current_time + config.window_seconds;
        return (true, new_count, config.max_requests, new_window_end);
    }

    // Window is still active, check count
    let current_count = get_rate_limit_count(env, address, operation);

    // If this is the first request in a window (count is 0), store the window_start
    if current_count == 0 {
        set_rate_limit_window(env, address, operation, window_start);
    }

    if current_count >= config.max_requests {
        // Rate limit exceeded
        return (false, current_count, config.max_requests, window_end);
    }

    // Increment count and allow
    let new_count = increment_rate_limit_count(env, address, operation);
    (true, new_count, config.max_requests, window_end)
}

/// Gets rate limit status for an address and operation
pub fn get_rate_limit_status(
    env: &Env,
    address: &Address,
    operation: &String,
) -> Option<RateLimitStatus> {
    let config = get_rate_limit_config(env, operation)?;
    let window_start = get_rate_limit_window(env, address, operation);
    let current_count = get_rate_limit_count(env, address, operation);
    let window_end = window_start + config.window_seconds;

    Some(RateLimitStatus {
        address: address.clone(),
        operation: operation.clone(),
        current_count,
        max_requests: config.max_requests,
        window_seconds: config.window_seconds,
        window_start,
        window_end,
        reset_at: window_end,
    })
}

/// Gets all rate limit configurations
pub fn get_all_rate_limit_configs(env: &Env) -> Vec<RateLimitConfig> {
    // Note: This is a simplified implementation
    // In a production system, you might want to maintain an index of all operations
    let mut configs = Vec::new(env);

    // Common operations that might have rate limits
    let mut operations = Vec::new(env);
    operations.push_back(String::from_str(env, "add_record"));
    operations.push_back(String::from_str(env, "get_record"));
    operations.push_back(String::from_str(env, "grant_access"));
    operations.push_back(String::from_str(env, "register_user"));

    for i in 0..operations.len() {
        if let Some(op) = operations.get(i) {
            if let Some(config) = get_rate_limit_config(env, &op) {
                configs.push_back(config);
            }
        }
    }

    configs
}

/// Gets addresses that have rate limit bypass
pub fn get_rate_limit_bypass_addresses(env: &Env) -> Vec<Address> {
    // Note: This is a simplified implementation
    // In production, you might want to maintain an index
    // This would need to be implemented with an index in a real system
    // For now, return empty vector
    Vec::new(env)
}
