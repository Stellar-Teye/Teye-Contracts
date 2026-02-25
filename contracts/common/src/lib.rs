//! Shared utilities and error types for the Teye contract suite.
//!
//! This crate provides:
//! - [`CommonError`] — standardised error codes for all contracts.
//! - Consent and key-management helpers (requires `std` feature).
//! - On-chain multisig, whitelist, meta-transaction, and rate-limiting utilities.
//! - [`migration`] — contract upgrade migration framework with data versioning
//!   and rollback support.
//! - [`versioned_storage`] — lazy-migration storage layer built on top of
//!   the migration framework.
//!
//! Contract-specific errors can extend the range starting at code **100** and
//! above, ensuring no collisions with the common set.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::arithmetic_side_effects)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

use soroban_sdk::contracterror;

// ── Modules ──────────────────────────────────────────────────────────────────

pub mod admin_tiers;
#[allow(clippy::enum_variant_names)]
pub mod admin_tiers;
pub mod concurrency;
pub mod conflict_resolver;
#[cfg(feature = "std")]
pub mod consent;
pub mod keys;
pub mod meta_tx;
pub mod metering;
pub mod multisig;
pub mod nonce;
pub mod rate_limit;
pub mod reentrancy_guard;
pub mod session;
pub mod risk_engine;
pub mod vector_clock;
pub mod whitelist;
pub mod transaction;

pub use admin_tiers::*;
pub use concurrency::*;
#[cfg(feature = "std")]
pub use consent::*;
pub use keys::*;
pub use meta_tx::*;
pub use metering::*;
pub use multisig::*;
pub use nonce::*;
pub use rate_limit::*;
pub use reentrancy_guard::*;
pub use session::*;
pub use risk_engine::*;
pub use vector_clock::*;
pub use whitelist::*;

// ── Shared error enum ────────────────────────────────────────────────────────

/// Standardised error codes shared by every Teye contract.
///
/// # Code ranges
/// | Range   | Purpose                       |
/// |---------|-------------------------------|
/// | 1 – 9   | Lifecycle / initialisation    |
/// | 10 – 19 | Authentication & authorisation|
/// | 20 – 29 | Resource not found            |
/// | 30 – 39 | Validation / input            |
/// | 40 – 49 | Contract state                |
/// | 50 – 59 | Migration & versioning        |
/// | 100+    | Reserved for contract-specific |
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
    /// Nonce does not match the expected value (replay or out-of-order).
    InvalidNonce = 31,
    /// Nonce counter would exceed u64::MAX.
    NonceOverflow = 32,
    // ── Contract state (40–49) ───────────────────────────────
    /// The contract is currently paused and cannot process requests.
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
#[cfg(test)]
mod tests {
    use super::CommonError;

    #[test]
    fn common_error_discriminants_are_stable() {
        assert_eq!(CommonError::NotInitialized as u32, 1);
        assert_eq!(CommonError::AlreadyInitialized as u32, 2);
        assert_eq!(CommonError::AccessDenied as u32, 10);
        assert_eq!(CommonError::UserNotFound as u32, 20);
        assert_eq!(CommonError::RecordNotFound as u32, 21);
        assert_eq!(CommonError::InvalidInput as u32, 30);
        assert_eq!(CommonError::InvalidNonce as u32, 31);
        assert_eq!(CommonError::NonceOverflow as u32, 32);
        assert_eq!(CommonError::Paused as u32, 40);
    }
}
