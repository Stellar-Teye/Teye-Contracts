//! Events emitted by the metering contract.

use soroban_sdk::{symbol_short, Address, Env};

use crate::{OperationType, TenantLevel};

// ── Internal helper ───────────────────────────────────────────────────────────

fn emit<T: soroban_sdk::TryIntoVal<Env, soroban_sdk::Val>>(env: &Env, topic: &str, data: T) {
    #[allow(deprecated)]
    env.events()
        .publish((symbol_short!("METER"), soroban_sdk::Symbol::new(env, topic)), data);
}

// ── Event structs ─────────────────────────────────────────────────────────────

#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenantRegisteredEvent {
    pub tenant: Address,
    pub level: TenantLevel,
    pub parent: Address,
    pub timestamp: u64,
}

#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GasRecordedEvent {
    pub tenant: Address,
    pub op_type: OperationType,
    pub units: u64,
    pub cycle_id: u64,
    pub timestamp: u64,
}

#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaAlertEvent {
    pub tenant: Address,
    pub percent_used: u32,
    pub timestamp: u64,
}

#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaExceededEvent {
    pub tenant: Address,
    pub op_type: OperationType,
    pub timestamp: u64,
}

#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CycleOpenedEvent {
    pub cycle_id: u64,
    pub timestamp: u64,
}

#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CycleClosedEvent {
    pub cycle_id: u64,
    pub timestamp: u64,
}

#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceIssuedEvent {
    pub tenant: Address,
    pub cycle_id: u64,
    pub amount_due: u64,
    pub timestamp: u64,
}

#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceSettledEvent {
    pub tenant: Address,
    pub cycle_id: u64,
    pub timestamp: u64,
}

#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GasTokenMintedEvent {
    pub tenant: Address,
    pub amount: u64,
    pub new_balance: u64,
    pub timestamp: u64,
}

#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GasTokenBurnedEvent {
    pub tenant: Address,
    pub amount: u64,
    pub remaining: u64,
    pub timestamp: u64,
}

// ── Publishers ────────────────────────────────────────────────────────────────

pub fn publish_tenant_registered(env: &Env, tenant: Address, level: TenantLevel, parent: Address) {
    emit(
        env,
        "TenantReg",
        TenantRegisteredEvent {
            tenant,
            level,
            parent,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_gas_recorded(
    env: &Env,
    tenant: Address,
    op_type: OperationType,
    units: u64,
    cycle_id: u64,
) {
    emit(
        env,
        "GasRec",
        GasRecordedEvent {
            tenant,
            op_type,
            units,
            cycle_id,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_quota_alert(env: &Env, tenant: Address, percent_used: u32) {
    emit(
        env,
        "QuotaAlert",
        QuotaAlertEvent {
            tenant,
            percent_used,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_quota_exceeded(env: &Env, tenant: Address, op_type: OperationType) {
    emit(
        env,
        "QuotaExcd",
        QuotaExceededEvent {
            tenant,
            op_type,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_cycle_opened(env: &Env, cycle_id: u64) {
    emit(
        env,
        "CycleOpen",
        CycleOpenedEvent {
            cycle_id,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_cycle_closed(env: &Env, cycle_id: u64) {
    emit(
        env,
        "CycleClose",
        CycleClosedEvent {
            cycle_id,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_invoice_issued(env: &Env, tenant: Address, cycle_id: u64, amount_due: u64) {
    emit(
        env,
        "InvIssued",
        InvoiceIssuedEvent {
            tenant,
            cycle_id,
            amount_due,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_invoice_settled(env: &Env, tenant: Address, cycle_id: u64) {
    emit(
        env,
        "InvSettled",
        InvoiceSettledEvent {
            tenant,
            cycle_id,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_gas_token_minted(env: &Env, tenant: Address, amount: u64, new_balance: u64) {
    emit(
        env,
        "GTMinted",
        GasTokenMintedEvent {
            tenant,
            amount,
            new_balance,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn publish_gas_token_burned(env: &Env, tenant: Address, amount: u64, remaining: u64) {
    emit(
        env,
        "GTBurned",
        GasTokenBurnedEvent {
            tenant,
            amount,
            remaining,
            timestamp: env.ledger().timestamp(),
        },
    );
}
