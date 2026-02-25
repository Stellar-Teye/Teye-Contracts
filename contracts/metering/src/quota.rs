//! Per-tenant quota definitions and enforcement.
//!
//! Quotas are defined at any level of the tenant hierarchy
//! (Organization → Clinic → Provider → Patient). When a quota is not set at a
//! lower level, the system walks up the hierarchy to find an inherited limit.
//!
//! ## Burst allowance
//! Each quota may specify a `burst_allowance` — extra capacity that may be
//! consumed beyond the base limit before the tenant is blocked. Burst usage is
//! tracked separately and replenished when a new billing cycle starts.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

// ── Storage key prefixes ─────────────────────────────────────────────────────

pub const QUOTA_KEY: Symbol = symbol_short!("QUOTA");
pub const BURST_KEY: Symbol = symbol_short!("BURST");

pub const TTL_THRESHOLD: u32 = 5_184_000;
pub const TTL_EXTEND_TO: u32 = 10_368_000;

// ── Types ─────────────────────────────────────────────────────────────────────

/// Quota limits for a single tenant address.
///
/// Gas units are abstract, contract-internal cost units defined in
/// [`crate::GasCosts`].  All limits are **per billing cycle**.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenantQuota {
    /// Maximum gas units for read operations per cycle.
    pub read_limit: u64,
    /// Maximum gas units for write operations per cycle.
    pub write_limit: u64,
    /// Maximum gas units for compute operations per cycle.
    pub compute_limit: u64,
    /// Maximum gas units for storage operations per cycle.
    pub storage_limit: u64,
    /// Total gas cap across all operation types per cycle.
    pub total_limit: u64,
    /// Additional gas units available beyond `total_limit` (burst).
    pub burst_allowance: u64,
    /// Whether quota enforcement is active for this tenant.
    pub enabled: bool,
}

/// Per-operation-type gas consumed by a tenant within the current cycle.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaUsage {
    pub read_used: u64,
    pub write_used: u64,
    pub compute_used: u64,
    pub storage_used: u64,
    /// Burst units consumed (drawn from `burst_allowance`).
    pub burst_used: u64,
}

impl QuotaUsage {
    /// Total gas across all non-burst buckets.
    pub fn total(&self) -> u64 {
        self.read_used
            .saturating_add(self.write_used)
            .saturating_add(self.compute_used)
            .saturating_add(self.storage_used)
    }
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn quota_key(tenant: &Address) -> (Symbol, Address) {
    (QUOTA_KEY, tenant.clone())
}

fn usage_key(tenant: &Address) -> (Symbol, Address) {
    (symbol_short!("QUSAGE"), tenant.clone())
}

fn extend_ttl(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Persist a quota configuration for a tenant.
pub fn set_quota(env: &Env, tenant: &Address, quota: TenantQuota) {
    let key = quota_key(tenant);
    env.storage().persistent().set(&key, &quota);
    extend_ttl(env, &key);
}

/// Retrieve the quota for a tenant, if one has been configured.
pub fn get_quota(env: &Env, tenant: &Address) -> Option<TenantQuota> {
    let key = quota_key(tenant);
    let quota: Option<TenantQuota> = env.storage().persistent().get(&key);
    if quota.is_some() {
        extend_ttl(env, &key);
    }
    quota
}

/// Remove the quota for a tenant (reverts to unlimited / inherited).
pub fn remove_quota(env: &Env, tenant: &Address) {
    env.storage().persistent().remove(&quota_key(tenant));
}

/// Read current usage for a tenant (returns zeroed struct if no data yet).
pub fn get_usage(env: &Env, tenant: &Address) -> QuotaUsage {
    let key = usage_key(tenant);
    let usage: Option<QuotaUsage> = env.storage().persistent().get(&key);
    if let Some(u) = usage {
        extend_ttl(env, &key);
        u
    } else {
        QuotaUsage {
            read_used: 0,
            write_used: 0,
            compute_used: 0,
            storage_used: 0,
            burst_used: 0,
        }
    }
}

/// Persist updated usage for a tenant.
pub fn set_usage(env: &Env, tenant: &Address, usage: &QuotaUsage) {
    let key = usage_key(tenant);
    env.storage().persistent().set(&key, usage);
    extend_ttl(env, &key);
}

/// Reset usage counters for a tenant (called at the start of each billing cycle).
pub fn reset_usage(env: &Env, tenant: &Address) {
    let key = usage_key(tenant);
    let zeroed = QuotaUsage {
        read_used: 0,
        write_used: 0,
        compute_used: 0,
        storage_used: 0,
        burst_used: 0,
    };
    env.storage().persistent().set(&key, &zeroed);
    extend_ttl(env, &key);
}

/// Check whether adding `delta` units to a specific bucket would breach
/// the configured quota.
///
/// Returns `Ok(())` if within limits (including burst), or
/// `Err(QuotaError)` if the operation would exceed both the hard limit and
/// the burst allowance.
pub fn check_quota(
    env: &Env,
    tenant: &Address,
    op_type: &super::OperationType,
    delta: u64,
) -> Result<(), QuotaError> {
    let quota = match get_quota(env, tenant) {
        Some(q) if q.enabled => q,
        // No quota configured, or quota disabled → allow all.
        _ => return Ok(()),
    };

    let usage = get_usage(env, tenant);

    // 1. Per-type bucket check.
    let (bucket_used, bucket_limit) = match op_type {
        super::OperationType::Read => (usage.read_used, quota.read_limit),
        super::OperationType::Write => (usage.write_used, quota.write_limit),
        super::OperationType::Compute => (usage.compute_used, quota.compute_limit),
        super::OperationType::Storage => (usage.storage_used, quota.storage_limit),
    };

    // 2. Total cap check.
    let total_after = usage.total().saturating_add(delta);

    if bucket_used.saturating_add(delta) <= bucket_limit && total_after <= quota.total_limit {
        return Ok(());
    }

    // 3. Try to draw from burst allowance.
    let burst_remaining = quota.burst_allowance.saturating_sub(usage.burst_used);
    if burst_remaining >= delta {
        return Ok(()); // Will be drawn from burst when usage is committed.
    }

    Err(QuotaError::QuotaExceeded)
}

/// Apply gas consumption to a tenant's usage counters.
/// If the regular bucket is exhausted, overflow spills into `burst_used`.
///
/// # Panics
/// This function does not panic; overflows are capped with saturating arithmetic.
pub fn consume_quota(env: &Env, tenant: &Address, op_type: &super::OperationType, delta: u64) {
    let quota_opt = get_quota(env, tenant);
    let mut usage = get_usage(env, tenant);

    let burst_draw = if let Some(ref quota) = quota_opt {
        let (bucket_used, bucket_limit) = match op_type {
            super::OperationType::Read => (usage.read_used, quota.read_limit),
            super::OperationType::Write => (usage.write_used, quota.write_limit),
            super::OperationType::Compute => (usage.compute_used, quota.compute_limit),
            super::OperationType::Storage => (usage.storage_used, quota.storage_limit),
        };

        // How much fits in the regular bucket?
        let headroom = bucket_limit.saturating_sub(bucket_used);
        if headroom >= delta {
            0u64 // Fits entirely in the regular bucket.
        } else {
            delta.saturating_sub(headroom) // Remainder goes to burst.
        }
    } else {
        0u64
    };

    // Commit to the appropriate bucket.
    match op_type {
        super::OperationType::Read => usage.read_used = usage.read_used.saturating_add(delta),
        super::OperationType::Write => usage.write_used = usage.write_used.saturating_add(delta),
        super::OperationType::Compute => {
            usage.compute_used = usage.compute_used.saturating_add(delta)
        }
        super::OperationType::Storage => {
            usage.storage_used = usage.storage_used.saturating_add(delta)
        }
    }
    usage.burst_used = usage.burst_used.saturating_add(burst_draw);

    set_usage(env, tenant, &usage);
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuotaError {
    /// The tenant has exhausted their quota and burst allowance.
    QuotaExceeded,
}
