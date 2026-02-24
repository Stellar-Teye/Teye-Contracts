//! Shared error types for the Teye contract suite.
//!
//! All contracts should use [`CommonError`] for standardised error codes.
//! Contract-specific errors can extend the range starting at code **100** and
//! above, ensuring no collisions with the common set.

#![no_std]

use soroban_sdk::contracterror;

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
