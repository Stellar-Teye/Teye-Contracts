//! Shared metering types and hook utilities used across multiple contracts.
//!
//! Contracts that want to report gas consumption to the `metering` contract
//! use [`MeteringHook`] which stores a configured metering contract address
//! and provides a single [`MeteringHook::record`] call per operation.
//!
//! If no metering contract is configured (i.e. the hook address is absent)
//! the call is a no-op, preserving backward compatibility.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

// ── Storage key ───────────────────────────────────────────────────────────────

/// Instance-storage key for the optional metering contract address.
pub const METERING_CONTRACT: Symbol = symbol_short!("MTR_CTR");

// ── Operation type (mirrors metering::OperationType) ─────────────────────────

/// The type of operation being tracked.
///
/// Must stay in sync with `metering::OperationType`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MeteringOpType {
    Read,
    Write,
    Compute,
    Storage,
}

// ── Hook ─────────────────────────────────────────────────────────────────────

/// Lightweight helper that records gas usage against the configured metering
/// contract without blocking the call on error (best-effort).
///
/// ## Usage
/// ```ignore
/// let hook = MeteringHook::load(&env);
/// hook.record(&env, &caller, MeteringOpType::Write);
/// ```
pub struct MeteringHook {
    pub contract: Option<Address>,
}

impl MeteringHook {
    /// Load the hook from instance storage.
    pub fn load(env: &Env) -> Self {
        let contract: Option<Address> = env.storage().instance().get(&METERING_CONTRACT);
        MeteringHook { contract }
    }

    /// Store (or clear) the metering contract address in instance storage.
    pub fn configure(env: &Env, address: Option<Address>) {
        match address {
            Some(addr) => env.storage().instance().set(&METERING_CONTRACT, &addr),
            None => {
                env.storage().instance().remove(&METERING_CONTRACT);
            }
        }
    }

    /// Whether a metering contract has been configured.
    pub fn is_configured(&self) -> bool {
        self.contract.is_some()
    }
}
