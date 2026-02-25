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

#[allow(clippy::enum_variant_names)]
pub mod admin_tiers;
pub mod concurrency;
pub mod conflict_resolver;
#[cfg(feature = "std")]
pub mod consent;
pub mod keys;
/// On-chain DAG provenance data model and storage primitives.
pub mod lineage;
pub mod meta_tx;
pub mod metering;
pub mod migration;
pub mod multisig;
pub mod operational_transform;
pub mod pausable;
pub mod policy_dsl;
pub mod policy_engine;
pub mod progressive_auth;
/// High-level provenance graph traversal, access control, and export.
pub mod provenance_graph;
pub mod rate_limit;
pub mod reentrancy_guard;
pub mod risk_engine;
pub mod session;
pub mod vector_clock;
pub mod versioned_storage;
pub mod whitelist;

pub use admin_tiers::*;
pub use concurrency::{
    compare_and_swap, get_pending_conflicts, get_record_conflicts, get_resolution_strategy as get_occ_strategy,
    resolve_conflict, set_resolution_strategy as set_occ_strategy, ConflictEntry,
    ResolutionStrategy as OCCStrategy, UpdateOutcome, VersionStamp,
};
#[cfg(feature = "std")]
pub use consent::*;
pub use keys::*;
pub use lineage::{
    LineageEdge, LineageNode, LineageSummary, RelationshipKind, TraversalNode, TraversalResult,
    VerificationResult,
};
pub use meta_tx::*;
pub use metering::*;
pub use migration::*;
pub use multisig::*;
pub use operational_transform::*;
pub use pausable::*;
pub use policy_dsl::*;
pub use policy_engine::{
    evaluate, evaluate_cached, evaluate_rule, get_resolution_strategy, set_resolution_strategy,
    simulate,
};
pub use conflict_resolver::ResolutionStrategy as PolicyStrategy;
pub use progressive_auth::*;
pub use provenance_graph::{LineageAccessResult, ProvenanceExport};
pub use rate_limit::*;
pub use reentrancy_guard::*;
pub use risk_engine::*;
pub use session::*;
pub use vector_clock::*;
pub use versioned_storage::*;
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

    // ── Lineage / Provenance (60–69) ─────────────────────────────────────
    /// The requested lineage node does not exist.
    LineageNodeNotFound = 60,

    /// The requested lineage edge does not exist.
    LineageEdgeNotFound = 61,

    /// A required ancestor node is missing from the provenance chain.
    LineageAncestorMissing = 62,

    /// A commitment mismatch was detected — the lineage has been tampered with.
    LineageTampered = 63,

    /// The lineage graph would form a cycle; DAG invariant must be preserved.
    LineageCycleDetected = 64,

    /// The caller does not have lineage-based access to the requested record.
    LineageAccessDenied = 65,
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
        // Lineage range.
        assert_eq!(CommonError::LineageNodeNotFound as u32, 60);
        assert_eq!(CommonError::LineageEdgeNotFound as u32, 61);
        assert_eq!(CommonError::LineageAncestorMissing as u32, 62);
        assert_eq!(CommonError::LineageTampered as u32, 63);
        assert_eq!(CommonError::LineageCycleDetected as u32, 64);
        assert_eq!(CommonError::LineageAccessDenied as u32, 65);
    }
}