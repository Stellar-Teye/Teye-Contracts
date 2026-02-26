//! Billing cycle management and usage reporting.
//!
//! ## Billing models
//! - **Prepaid**: The tenant deposits gas tokens before usage. Operations are
//!   rejected once the balance is exhausted.
//! - **Postpaid**: Usage is accumulated throughout the cycle. An invoice is
//!   generated at cycle close and must be settled before the next cycle opens.
//!
//! ## Cycle lifecycle
//! 1. `open_cycle` — admin starts a new cycle, resetting per-tenant usage.
//! 2. Operations are metered in real-time via `record_usage`.
//! 3. `close_cycle` — admin closes the cycle; a `BillingReport` is finalised.
//! 4. `settle_invoice` — postpaid tenants pay their invoice.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

// ── Storage keys ──────────────────────────────────────────────────────────────

pub const CYCLE_CTR: Symbol = symbol_short!("CYC_CTR");
const CYCLE_KEY: Symbol = symbol_short!("CYCLE");
const INVOICE_KEY: Symbol = symbol_short!("INVOICE");
const BILLING_MDL: Symbol = symbol_short!("BIL_MDL");
const PREPAID_BAL: Symbol = symbol_short!("PP_BAL");

const TTL_THRESHOLD: u32 = 5_184_000;
const TTL_EXTEND_TO: u32 = 10_368_000;

// ── Types ─────────────────────────────────────────────────────────────────────

/// Billing model for a tenant.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BillingModel {
    /// Tenant pre-loads gas tokens; operations draw down the balance.
    Prepaid,
    /// Usage is accumulated and invoiced at cycle close.
    Postpaid,
}

/// Status of a billing cycle.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CycleStatus {
    Open,
    Closed,
}

/// A billing cycle — tracks the time window and its status.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingCycle {
    pub id: u64,
    pub started_at: u64,
    pub ended_at: u64, // 0 while still open
    pub status: CycleStatus,
}

/// Per-tenant usage snapshot within a cycle (used in reports and invoices).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenantUsageRecord {
    pub tenant: Address,
    pub cycle_id: u64,
    pub read_units: u64,
    pub write_units: u64,
    pub compute_units: u64,
    pub storage_units: u64,
    pub burst_units: u64,
    /// Total cost in gas tokens (computed at cycle close using `GasCosts`).
    pub total_cost: u64,
}

/// A billing report for a closed cycle, covering all registered tenants.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct BillingReport {
    pub cycle_id: u64,
    pub closed_at: u64,
    pub records: Vec<TenantUsageRecord>,
}

/// An invoice for a postpaid tenant for a specific cycle.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invoice {
    pub tenant: Address,
    pub cycle_id: u64,
    pub amount_due: u64,
    pub settled: bool,
    pub issued_at: u64,
    pub settled_at: u64, // 0 if unsettled
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BillingError {
    CycleAlreadyOpen,
    NoCycleOpen,
    InvoiceNotFound,
    AlreadySettled,
    InsufficientPrepaidBalance,
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn cycle_key(id: u64) -> (Symbol, u64) {
    (CYCLE_KEY, id)
}

fn invoice_key(tenant: &Address, cycle_id: u64) -> (Symbol, Address, u64) {
    (INVOICE_KEY, tenant.clone(), cycle_id)
}

fn prepaid_balance_key(tenant: &Address) -> (Symbol, Address) {
    (PREPAID_BAL, tenant.clone())
}

fn billing_model_key(tenant: &Address) -> (Symbol, Address) {
    (BILLING_MDL, tenant.clone())
}

fn extend_cycle_ttl(env: &Env, key: &(Symbol, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_addr_ttl(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_invoice_ttl(env: &Env, key: &(Symbol, Address, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Return the current cycle counter (last issued cycle id, or 0).
pub fn current_cycle_id(env: &Env) -> u64 {
    env.storage().instance().get(&CYCLE_CTR).unwrap_or(0)
}

/// Open a new billing cycle. Fails if a cycle is already open.
pub fn open_cycle(env: &Env) -> Result<u64, BillingError> {
    let current_id = current_cycle_id(env);

    // If any cycle exists, verify it's closed before opening a new one.
    if current_id > 0 {
        let key = cycle_key(current_id);
        if let Some(cycle) = env.storage().persistent().get::<_, BillingCycle>(&key) {
            if cycle.status == CycleStatus::Open {
                return Err(BillingError::CycleAlreadyOpen);
            }
        }
    }

    let new_id = current_id.saturating_add(1);
    let cycle = BillingCycle {
        id: new_id,
        started_at: env.ledger().timestamp(),
        ended_at: 0,
        status: CycleStatus::Open,
    };

    let key = cycle_key(new_id);
    env.storage().persistent().set(&key, &cycle);
    extend_cycle_ttl(env, &key);
    env.storage().instance().set(&CYCLE_CTR, &new_id);

    Ok(new_id)
}

/// Close the currently open billing cycle.
/// Returns the id of the cycle that was closed.
pub fn close_cycle(env: &Env) -> Result<u64, BillingError> {
    let current_id = current_cycle_id(env);
    if current_id == 0 {
        return Err(BillingError::NoCycleOpen);
    }

    let key = cycle_key(current_id);
    let mut cycle: BillingCycle = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(BillingError::NoCycleOpen)?;

    if cycle.status == CycleStatus::Closed {
        return Err(BillingError::NoCycleOpen);
    }

    cycle.status = CycleStatus::Closed;
    cycle.ended_at = env.ledger().timestamp();

    env.storage().persistent().set(&key, &cycle);
    extend_cycle_ttl(env, &key);

    Ok(current_id)
}

/// Retrieve a billing cycle by id.
pub fn get_cycle(env: &Env, id: u64) -> Option<BillingCycle> {
    let key = cycle_key(id);
    let cycle: Option<BillingCycle> = env.storage().persistent().get(&key);
    if cycle.is_some() {
        extend_cycle_ttl(env, &key);
    }
    cycle
}

// ── Billing model helpers ─────────────────────────────────────────────────────

/// Set billing model for a tenant.
pub fn set_billing_model(env: &Env, tenant: &Address, model: BillingModel) {
    let key = billing_model_key(tenant);
    env.storage().persistent().set(&key, &model);
    extend_addr_ttl(env, &key);
}

/// Get billing model for a tenant (defaults to `Postpaid` if unset).
pub fn get_billing_model(env: &Env, tenant: &Address) -> BillingModel {
    let key = billing_model_key(tenant);
    let model: Option<BillingModel> = env.storage().persistent().get(&key);
    if model.is_some() {
        extend_addr_ttl(env, &key);
    }
    model.unwrap_or(BillingModel::Postpaid)
}

// ── Prepaid balance helpers ───────────────────────────────────────────────────

/// Return prepaid gas token balance for a tenant.
pub fn get_prepaid_balance(env: &Env, tenant: &Address) -> u64 {
    let key = prepaid_balance_key(tenant);
    let bal: Option<u64> = env.storage().persistent().get(&key);
    if bal.is_some() {
        extend_addr_ttl(env, &key);
    }
    bal.unwrap_or(0)
}

/// Credit gas tokens to a tenant's prepaid balance.
pub fn credit_prepaid(env: &Env, tenant: &Address, amount: u64) {
    let key = prepaid_balance_key(tenant);
    let current = get_prepaid_balance(env, tenant);
    let new_balance = current.saturating_add(amount);
    env.storage().persistent().set(&key, &new_balance);
    extend_addr_ttl(env, &key);
}

/// Debit gas tokens from a tenant's prepaid balance.
/// Returns an error if the balance is insufficient.
pub fn debit_prepaid(env: &Env, tenant: &Address, amount: u64) -> Result<(), BillingError> {
    let current = get_prepaid_balance(env, tenant);
    if current < amount {
        return Err(BillingError::InsufficientPrepaidBalance);
    }
    let key = prepaid_balance_key(tenant);
    let new_balance = current.saturating_sub(amount);
    env.storage().persistent().set(&key, &new_balance);
    extend_addr_ttl(env, &key);
    Ok(())
}

// ── Invoice helpers ───────────────────────────────────────────────────────────

/// Create (or overwrite) an invoice for a postpaid tenant after cycle close.
pub fn create_invoice(env: &Env, tenant: &Address, cycle_id: u64, amount_due: u64) {
    let key = invoice_key(tenant, cycle_id);
    let invoice = Invoice {
        tenant: tenant.clone(),
        cycle_id,
        amount_due,
        settled: false,
        issued_at: env.ledger().timestamp(),
        settled_at: 0,
    };
    env.storage().persistent().set(&key, &invoice);
    extend_invoice_ttl(env, &key);
}

/// Retrieve an invoice for a tenant / cycle pair.
pub fn get_invoice(env: &Env, tenant: &Address, cycle_id: u64) -> Option<Invoice> {
    let key = invoice_key(tenant, cycle_id);
    let inv: Option<Invoice> = env.storage().persistent().get(&key);
    if inv.is_some() {
        extend_invoice_ttl(env, &key);
    }
    inv
}

/// Mark an invoice as settled.
pub fn settle_invoice(env: &Env, tenant: &Address, cycle_id: u64) -> Result<(), BillingError> {
    let key = invoice_key(tenant, cycle_id);
    let mut inv: Invoice = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(BillingError::InvoiceNotFound)?;

    if inv.settled {
        return Err(BillingError::AlreadySettled);
    }

    inv.settled = true;
    inv.settled_at = env.ledger().timestamp();
    env.storage().persistent().set(&key, &inv);
    extend_invoice_ttl(env, &key);
    Ok(())
}
