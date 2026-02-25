//! Shared utilities and error types for the Teye contract suite.
//!
//! This crate provides:
//! - [`CommonError`] — standardised error codes for all contracts.
//! - Consent, key-management, and multisig helpers (requires `std` feature).
//! - On-chain whitelist, meta-transaction, and rate-limiting utilities.
//!
//! Contract-specific errors can extend the range starting at code **100** and
//! above, ensuring no collisions with the common set.

#![cfg_attr(not(feature = "std"), no_std)]

use soroban_sdk::contracterror;

// ── Modules ──────────────────────────────────────────────────────────────────

pub mod admin_tiers;
#[cfg(feature = "std")]
pub mod consent;
pub mod keys;
pub mod meta_tx;
#[cfg(feature = "std")]
pub mod multisig;
pub mod rate_limit;
pub mod whitelist;

pub use admin_tiers::*;
#[cfg(feature = "std")]
pub use consent::*;
pub use keys::*;
pub use meta_tx::*;
#[cfg(feature = "std")]
pub use multisig::*;
pub use rate_limit::*;
pub use whitelist::*;

// ── Shared error enum ────────────────────────────────────────────────────────

/// Standardised error codes shared by every Teye contract.
#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum CommonError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    AccessDenied = 10,
    UserNotFound = 20,
    RecordNotFound = 21,
    InvalidInput = 30,
    Paused = 40,
    InvalidChannelState = 50,
    InvalidSignature = 51,
    InvalidTransition = 52,
    ChallengePeriodActive = 53,
    AlreadySettled = 54,
}

/// Shared channel status for lifespan tracking
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ChannelStatus {
    Open = 0,
    Closing = 1,
    Closed = 2,
    Settled = 3,
    Disputed = 4,
}
