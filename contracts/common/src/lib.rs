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

#[cfg(feature = "std")]
pub mod consent;
#[cfg(feature = "std")]
pub mod keys;
pub mod meta_tx;
#[cfg(feature = "std")]
pub mod multisig;
pub mod rate_limit;
pub mod whitelist;

pub use admin_tiers::*;
#[cfg(feature = "std")]
pub use consent::*;
#[cfg(feature = "std")]
pub use keys::*;
pub use meta_tx::*;
#[cfg(feature = "std")]
pub use multisig::*;
pub use rate_limit::*;
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
/// | 100+    | Reserved for contract-specific |
#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum CommonError {
    // ── Lifecycle (1–9) ──────────────────────────────────────
    /// The contract has not been initialised yet.
    /// Returned when a function requires prior initialisation.
    NotInitialized = 1,

    /// The contract has already been initialised.
    /// Returned when `initialize` is called more than once.
    AlreadyInitialized = 2,

    // ── Auth (10–19) ─────────────────────────────────────────
    /// The caller lacks the required role or permission to perform
    /// the requested operation (e.g. not an admin, not the record owner).
    AccessDenied = 10,

    // ── Not-found (20–29) ────────────────────────────────────
    /// The requested user does not exist in contract storage.
    UserNotFound = 20,

    /// The requested record does not exist in contract storage.
    RecordNotFound = 21,

    // ── Validation (30–39) ───────────────────────────────────
    /// One or more input parameters are invalid (e.g. empty list,
    /// zero duration, malformed hash).
    InvalidInput = 30,

    // ── Contract state (40–49) ───────────────────────────────
    /// The contract is currently paused and cannot process requests.
    Paused = 40,
}

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
        assert_eq!(CommonError::Paused as u32, 40);
    }
}
