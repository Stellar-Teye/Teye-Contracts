//! # Gas Metering & Cost Allocation Contract
//!
//! Tracks per-tenant resource consumption, enforces quotas, and enables fair
//! cost allocation across clinics/organizations sharing the contract infrastructure.
//!
//! ## Tenant hierarchy
//! ```text
//! Organization
//!   └── Clinic
//!         └── Provider
//!               └── Patient
//! ```
//! Gas consumed by a lower-level tenant is **rolled up** into every ancestor's
//! usage counters, enabling top-down quota enforcement and accurate billing.
//!
//! ## Operation types & costs
//! | Type    | Default cost (units) |
//! |---------|---------------------|
//! | Read    | 1                   |
//! | Write   | 5                   |
//! | Compute | 10                  |
//! | Storage | 3                   |
//!
//! ## Alert thresholds
//! A `QuotaAlertEvent` is emitted when a tenant crosses 80 % of their total
//! quota.  Operations are blocked once the quota **and** burst allowance are
//! exhausted.
#![no_std]
#![allow(clippy::too_many_arguments)]

pub mod billing;
pub mod events;
pub mod gas_token;
pub mod quota;

use billing::{BillingError, BillingModel, BillingReport, Invoice, TenantUsageRecord};
use gas_token::GasTokenError;
use quota::{QuotaError, QuotaUsage, TenantQuota};

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

// ── Storage keys ──────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");
const TENANT_KEY: Symbol = symbol_short!("TENANT");
const TENANT_LIST: Symbol = symbol_short!("TEN_LST");
const PARENT_KEY: Symbol = symbol_short!("PARENT");
const GAS_COSTS: Symbol = symbol_short!("GAS_CST");

/// Percentage of total quota consumed before a `QuotaAlertEvent` fires.
const ALERT_THRESHOLD_PCT: u64 = 80;

const TTL_THRESHOLD: u32 = 5_184_000;
const TTL_EXTEND_TO: u32 = 10_368_000;

// ── Types ─────────────────────────────────────────────────────────────────────

/// Level in the tenant hierarchy.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TenantLevel {
    Organization,
    Clinic,
    Provider,
    Patient,
}

/// Type of operation being metered.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationType {
    Read,
    Write,
    Compute,
    Storage,
}

/// Per-operation gas costs (in abstract units).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GasCosts {
    pub read_cost: u64,
    pub write_cost: u64,
    pub compute_cost: u64,
    pub storage_cost: u64,
}

impl GasCosts {
    pub fn default_costs() -> Self {
        GasCosts {
            read_cost: 1,
            write_cost: 5,
            compute_cost: 10,
            storage_cost: 3,
        }
    }

    pub fn cost_for(&self, op: &OperationType) -> u64 {
        match op {
            OperationType::Read => self.read_cost,
            OperationType::Write => self.write_cost,
            OperationType::Compute => self.compute_cost,
            OperationType::Storage => self.storage_cost,
        }
    }
}

/// Registration record for a tenant.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tenant {
    pub address: Address,
    pub level: TenantLevel,
    /// Immediate parent in the hierarchy (self == address for root orgs).
    pub parent: Address,
    pub registered_at: u64,
    pub active: bool,
}

// ── Contract errors ───────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MeteringError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    TenantNotFound = 4,
    TenantAlreadyExists = 5,
    TenantInactive = 6,
    QuotaExceeded = 7,
    InvalidInput = 8,
    CycleAlreadyOpen = 9,
    NoCycleOpen = 10,
    InvoiceNotFound = 11,
    AlreadySettled = 12,
    InsufficientPrepaidBalance = 13,
    GasTokenAccountFrozen = 14,
    GasTokenInsufficientBalance = 15,
    ZeroMintAmount = 16,
}

fn map_quota_error(_e: QuotaError) -> MeteringError {
    MeteringError::QuotaExceeded
}

fn map_billing_error(e: BillingError) -> MeteringError {
    match e {
        BillingError::CycleAlreadyOpen => MeteringError::CycleAlreadyOpen,
        BillingError::NoCycleOpen => MeteringError::NoCycleOpen,
        BillingError::InvoiceNotFound => MeteringError::InvoiceNotFound,
        BillingError::AlreadySettled => MeteringError::AlreadySettled,
        BillingError::InsufficientPrepaidBalance => MeteringError::InsufficientPrepaidBalance,
    }
}

fn map_gas_token_error(e: GasTokenError) -> MeteringError {
    match e {
        GasTokenError::AccountFrozen => MeteringError::GasTokenAccountFrozen,
        GasTokenError::InsufficientBalance => MeteringError::GasTokenInsufficientBalance,
        GasTokenError::ZeroMintAmount => MeteringError::ZeroMintAmount,
    }
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn tenant_key(addr: &Address) -> (Symbol, Address) {
    (TENANT_KEY, addr.clone())
}

fn parent_key(addr: &Address) -> (Symbol, Address) {
    (PARENT_KEY, addr.clone())
}

fn extend_addr_ttl(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MeteringContract;

#[contractimpl]
impl MeteringContract {
    // ── Initialisation ────────────────────────────────────────────────────────

    /// Initialise the metering contract with an administrator address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), MeteringError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(MeteringError::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);

        // Persist default gas costs.
        env.storage()
            .instance()
            .set(&GAS_COSTS, &GasCosts::default_costs());

        Ok(())
    }

    // ── Admin helpers ─────────────────────────────────────────────────────────

    fn require_admin(env: &Env, caller: &Address) -> Result<(), MeteringError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(MeteringError::NotInitialized)?;
        if *caller != admin {
            return Err(MeteringError::Unauthorized);
        }
        Ok(())
    }

    fn require_initialized(env: &Env) -> Result<(), MeteringError> {
        if !env.storage().instance().has(&INITIALIZED) {
            return Err(MeteringError::NotInitialized);
        }
        Ok(())
    }

    /// Return the admin address.
    pub fn get_admin(env: Env) -> Result<Address, MeteringError> {
        env.storage()
            .instance()
            .get(&ADMIN)
            .ok_or(MeteringError::NotInitialized)
    }

    // ── Gas cost configuration ────────────────────────────────────────────────

    /// Update the per-operation gas costs. Admin only.
    pub fn set_gas_costs(env: Env, caller: Address, costs: GasCosts) -> Result<(), MeteringError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;
        env.storage().instance().set(&GAS_COSTS, &costs);
        Ok(())
    }

    /// Return the current per-operation gas costs.
    pub fn get_gas_costs(env: Env) -> GasCosts {
        env.storage()
            .instance()
            .get(&GAS_COSTS)
            .unwrap_or_else(GasCosts::default_costs)
    }

    // ── Tenant management ─────────────────────────────────────────────────────

    /// Register a new tenant at the specified hierarchy level.
    ///
    /// - Root organisations must pass their own address as `parent`.
    /// - Lower levels must pass the address of their direct parent, which must
    ///   already be registered.
    pub fn register_tenant(
        env: Env,
        caller: Address,
        tenant: Address,
        level: TenantLevel,
        parent: Address,
    ) -> Result<(), MeteringError> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;

        // Ensure not already registered.
        let key = tenant_key(&tenant);
        if env.storage().persistent().has(&key) {
            return Err(MeteringError::TenantAlreadyExists);
        }

        // For non-root tenants, verify parent exists.
        if level != TenantLevel::Organization {
            let parent_key_val = tenant_key(&parent);
            if !env.storage().persistent().has(&parent_key_val) {
                return Err(MeteringError::TenantNotFound);
            }
        }

        let record = Tenant {
            address: tenant.clone(),
            level: level.clone(),
            parent: parent.clone(),
            registered_at: env.ledger().timestamp(),
            active: true,
        };

        env.storage().persistent().set(&key, &record);
        extend_addr_ttl(&env, &key);

        // Store parent pointer separately for efficient rollup.
        let pk = parent_key(&tenant);
        env.storage().persistent().set(&pk, &parent);
        extend_addr_ttl(&env, &pk);

        // Append to global tenant list.
        let mut list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&TENANT_LIST)
            .unwrap_or(Vec::new(&env));
        list.push_back(tenant.clone());
        env.storage().persistent().set(&TENANT_LIST, &list);

        events::publish_tenant_registered(&env, tenant, level, parent);

        Ok(())
    }

    /// Deactivate a tenant, preventing further gas recording.
    pub fn deactivate_tenant(
        env: Env,
        caller: Address,
        tenant: Address,
    ) -> Result<(), MeteringError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        let key = tenant_key(&tenant);
        let mut record: Tenant = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(MeteringError::TenantNotFound)?;

        record.active = false;
        env.storage().persistent().set(&key, &record);
        extend_addr_ttl(&env, &key);

        Ok(())
    }

    /// Retrieve tenant registration record.
    pub fn get_tenant(env: Env, tenant: Address) -> Result<Tenant, MeteringError> {
        let key = tenant_key(&tenant);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(MeteringError::TenantNotFound)
    }

    // ── Quota configuration ───────────────────────────────────────────────────

    /// Set or update a quota for a tenant. Admin only.
    pub fn set_quota(
        env: Env,
        caller: Address,
        tenant: Address,
        quota: TenantQuota,
    ) -> Result<(), MeteringError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        // Tenant must be registered.
        let tenant_key_val = tenant_key(&tenant);
        if !env.storage().persistent().has(&tenant_key_val) {
            return Err(MeteringError::TenantNotFound);
        }

        quota::set_quota(&env, &tenant, quota);

        Ok(())
    }

    /// Remove the quota for a tenant (reverts to unlimited).
    pub fn remove_quota(env: Env, caller: Address, tenant: Address) -> Result<(), MeteringError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;
        quota::remove_quota(&env, &tenant);
        Ok(())
    }

    /// Return the quota for a tenant.
    pub fn get_quota(env: Env, tenant: Address) -> Option<TenantQuota> {
        quota::get_quota(&env, &tenant)
    }

    /// Return current usage counters for a tenant.
    pub fn get_usage(env: Env, tenant: Address) -> QuotaUsage {
        quota::get_usage(&env, &tenant)
    }

    // ── Gas recording ─────────────────────────────────────────────────────────

    /// Record gas consumption for a tenant and propagate up the hierarchy.
    ///
    /// - Validates the tenant is registered and active.
    /// - Enforces quota (with burst).
    /// - If prepaid: burns gas tokens.
    /// - Propagates usage to every ancestor.
    /// - Emits alert event when tenant crosses 80 % of total quota.
    pub fn record_gas(
        env: Env,
        caller: Address,
        tenant: Address,
        op_type: OperationType,
    ) -> Result<(), MeteringError> {
        caller.require_auth();
        Self::require_initialized(&env)?;

        // Validate tenant.
        let tenant_record: Tenant = env
            .storage()
            .persistent()
            .get(&tenant_key(&tenant))
            .ok_or(MeteringError::TenantNotFound)?;

        if !tenant_record.active {
            return Err(MeteringError::TenantInactive);
        }

        // Determine gas units for this operation type.
        let costs: GasCosts = env
            .storage()
            .instance()
            .get(&GAS_COSTS)
            .unwrap_or_else(GasCosts::default_costs);
        let units = costs.cost_for(&op_type);

        // Enforce quota for the direct tenant.
        quota::check_quota(&env, &tenant, &op_type, units).map_err(|e| {
            events::publish_quota_exceeded(&env, tenant.clone(), op_type.clone());
            map_quota_error(e)
        })?;

        // Prepaid: debit gas tokens.
        let model = billing::get_billing_model(&env, &tenant);
        if model == BillingModel::Prepaid {
            let current_balance = gas_token::balance_of(&env, &tenant);
            if gas_token::is_frozen(&env, &tenant) {
                return Err(MeteringError::GasTokenAccountFrozen);
            }
            if current_balance < units {
                return Err(MeteringError::InsufficientPrepaidBalance);
            }
            gas_token::burn(&env, &tenant, units).map_err(map_gas_token_error)?;
            let new_balance = gas_token::balance_of(&env, &tenant);
            events::publish_gas_token_burned(&env, tenant.clone(), units, new_balance);
        }

        // Commit usage for the direct tenant.
        quota::consume_quota(&env, &tenant, &op_type, units);

        // Get active cycle id (0 if none).
        let cycle_id = billing::current_cycle_id(&env);

        // Emit gas recorded event.
        events::publish_gas_recorded(&env, tenant.clone(), op_type.clone(), units, cycle_id);

        // Check and possibly emit alert.
        Self::maybe_emit_alert(&env, &tenant);

        // Propagate consumption up the hierarchy.
        Self::rollup_gas(&env, &tenant_record, &op_type, units, cycle_id);

        Ok(())
    }

    /// Walk up the tenant tree and apply usage to every ancestor.
    fn rollup_gas(env: &Env, child: &Tenant, op_type: &OperationType, units: u64, cycle_id: u64) {
        // Stop if child is an org (root) or parent == child.
        if child.level == TenantLevel::Organization || child.parent == child.address {
            return;
        }

        let pk = parent_key(&child.address);
        let parent_addr: Option<Address> = env.storage().persistent().get(&pk);
        let parent_addr = match parent_addr {
            Some(a) => a,
            None => return,
        };

        // Apply to parent's usage.
        quota::consume_quota(env, &parent_addr, op_type, units);
        events::publish_gas_recorded(env, parent_addr.clone(), op_type.clone(), units, cycle_id);
        Self::maybe_emit_alert(env, &parent_addr);

        // Recurse.
        let parent_record: Option<Tenant> =
            env.storage().persistent().get(&tenant_key(&parent_addr));
        if let Some(pr) = parent_record {
            Self::rollup_gas(env, &pr, op_type, units, cycle_id);
        }
    }

    /// Emit a `QuotaAlertEvent` if the tenant has crossed the alert threshold.
    fn maybe_emit_alert(env: &Env, tenant: &Address) {
        let quota = match quota::get_quota(env, tenant) {
            Some(q) if q.enabled && q.total_limit > 0 => q,
            _ => return,
        };

        let usage = quota::get_usage(env, tenant);
        let total_used = usage.total();

        // Compute percentage: (used * 100) / limit
        let pct = total_used
            .saturating_mul(100)
            .checked_div(quota.total_limit)
            .unwrap_or(0);

        if pct >= ALERT_THRESHOLD_PCT {
            events::publish_quota_alert(env, tenant.clone(), pct as u32);
        }
    }

    // ── Billing cycle management ──────────────────────────────────────────────

    /// Open a new billing cycle. Admin only.
    /// Resets usage counters for all registered tenants.
    pub fn open_billing_cycle(env: Env, caller: Address) -> Result<u64, MeteringError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        let cycle_id = billing::open_cycle(&env).map_err(map_billing_error)?;

        // Reset usage for all tenants.
        let list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&TENANT_LIST)
            .unwrap_or(Vec::new(&env));

        for i in 0..list.len() {
            if let Some(addr) = list.get(i) {
                quota::reset_usage(&env, &addr);
            }
        }

        events::publish_cycle_opened(&env, cycle_id);

        Ok(cycle_id)
    }

    /// Close the current billing cycle and generate invoices for postpaid tenants.
    /// Returns a `BillingReport` summarising usage and costs for all tenants.
    pub fn close_billing_cycle(env: Env, caller: Address) -> Result<BillingReport, MeteringError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        let cycle_id = billing::close_cycle(&env).map_err(map_billing_error)?;

        let list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&TENANT_LIST)
            .unwrap_or(Vec::new(&env));

        let mut records: Vec<TenantUsageRecord> = Vec::new(&env);

        for i in 0..list.len() {
            if let Some(addr) = list.get(i) {
                let usage = quota::get_usage(&env, &addr);

                let total_cost = usage
                    .read_used
                    .saturating_add(usage.write_used)
                    .saturating_add(usage.compute_used)
                    .saturating_add(usage.storage_used);

                let record = TenantUsageRecord {
                    tenant: addr.clone(),
                    cycle_id,
                    read_units: usage.read_used,
                    write_units: usage.write_used,
                    compute_units: usage.compute_used,
                    storage_units: usage.storage_used,
                    burst_units: usage.burst_used,
                    total_cost,
                };

                // Issue invoice for postpaid tenants with non-zero cost.
                let model = billing::get_billing_model(&env, &addr);
                if model == BillingModel::Postpaid && total_cost > 0 {
                    billing::create_invoice(&env, &addr, cycle_id, total_cost);
                    events::publish_invoice_issued(&env, addr.clone(), cycle_id, total_cost);
                }

                records.push_back(record);
            }
        }

        events::publish_cycle_closed(&env, cycle_id);

        Ok(BillingReport {
            cycle_id,
            closed_at: env.ledger().timestamp(),
            records,
        })
    }

    /// Return the current active cycle id (0 if no cycle has been opened).
    pub fn current_cycle_id(env: Env) -> u64 {
        billing::current_cycle_id(&env)
    }

    /// Return a billing cycle by id.
    pub fn get_billing_cycle(env: Env, cycle_id: u64) -> Option<billing::BillingCycle> {
        billing::get_cycle(&env, cycle_id)
    }

    // ── Billing model helpers ─────────────────────────────────────────────────

    /// Set the billing model for a tenant. Admin only.
    pub fn set_billing_model(
        env: Env,
        caller: Address,
        tenant: Address,
        model: BillingModel,
    ) -> Result<(), MeteringError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;
        billing::set_billing_model(&env, &tenant, model);
        Ok(())
    }

    /// Return the billing model for a tenant.
    pub fn get_billing_model(env: Env, tenant: Address) -> BillingModel {
        billing::get_billing_model(&env, &tenant)
    }

    // ── Invoice management ────────────────────────────────────────────────────

    /// Return an invoice for a tenant / cycle pair.
    pub fn get_invoice(env: Env, tenant: Address, cycle_id: u64) -> Option<Invoice> {
        billing::get_invoice(&env, &tenant, cycle_id)
    }

    /// Settle a postpaid invoice.  The tenant must call this themselves.
    pub fn settle_invoice(env: Env, caller: Address, cycle_id: u64) -> Result<(), MeteringError> {
        caller.require_auth();
        billing::settle_invoice(&env, &caller, cycle_id).map_err(map_billing_error)?;
        events::publish_invoice_settled(&env, caller, cycle_id);
        Ok(())
    }

    // ── Gas token management ──────────────────────────────────────────────────

    /// Mint gas tokens to a tenant (prepaid top-up). Admin only.
    pub fn mint_gas_tokens(
        env: Env,
        caller: Address,
        tenant: Address,
        amount: u64,
    ) -> Result<(), MeteringError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        gas_token::mint(&env, &tenant, amount).map_err(map_gas_token_error)?;

        // Also credit the prepaid balance tracker in billing.
        billing::credit_prepaid(&env, &tenant, amount);

        let new_balance = gas_token::balance_of(&env, &tenant);
        events::publish_gas_token_minted(&env, tenant, amount, new_balance);

        Ok(())
    }

    /// Return the gas token balance for a tenant.
    pub fn gas_token_balance(env: Env, tenant: Address) -> u64 {
        gas_token::balance_of(&env, &tenant)
    }

    /// Freeze a tenant's gas token account. Admin only.
    pub fn freeze_gas_token_account(
        env: Env,
        caller: Address,
        tenant: Address,
    ) -> Result<(), MeteringError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;
        gas_token::freeze(&env, &tenant);
        Ok(())
    }

    /// Unfreeze a tenant's gas token account. Admin only.
    pub fn unfreeze_gas_token_account(
        env: Env,
        caller: Address,
        tenant: Address,
    ) -> Result<(), MeteringError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;
        gas_token::unfreeze(&env, &tenant);
        Ok(())
    }

    /// Return the full gas token account for a tenant.
    pub fn get_gas_token_account(env: Env, tenant: Address) -> gas_token::GasTokenAccount {
        gas_token::get_account(&env, &tenant)
    }

    /// Return the total gas tokens minted across all tenants.
    pub fn total_gas_token_supply(env: Env) -> u64 {
        gas_token::total_supply(&env)
    }

    // ── Query helpers ─────────────────────────────────────────────────────────

    /// Return the list of all registered tenant addresses.
    pub fn list_tenants(env: Env) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&TENANT_LIST)
            .unwrap_or(Vec::new(&env))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test;
