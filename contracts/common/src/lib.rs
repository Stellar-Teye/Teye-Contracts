//! Shared utilities and error types for the Teye contract suite.
//!
//! This crate provides:
//! - [`CommonError`] — standardised error codes for all contracts.
//! - On-chain multisig, whitelist, meta-transaction, and rate-limiting utilities.
//! - [`migration`] — contract upgrade migration framework with data versioning
//!   and rollback support.
//! - [`versioned_storage`] — lazy-migration storage layer built on top of
//!   the migration framework.

#![no_std] // <--- FIX: Changed from conditional to absolute no_std
#![allow(clippy::arithmetic_side_effects)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

use soroban_sdk::contracterror;

// ── Modules ──────────────────────────────────────────────────────────────────

#[allow(clippy::enum_variant_names)]
pub mod admin_tiers;
pub mod concurrency;
pub mod conflict_resolver;
// Removed #[cfg(feature = "std")] mod consent - WASM contracts cannot use std-only modules
pub mod keys;
/// On-chain DAG provenance data model and storage primitives.
pub mod lineage;
pub mod meta_tx;
pub mod metering;
pub mod migration;
pub mod multisig;
pub mod nonce;
pub mod pausable;
pub mod policy_dsl;
pub mod progressive_auth;
pub mod rate_limit;
pub mod reentrancy_guard;
pub mod risk_engine;
pub mod session;
pub mod transaction;
pub mod vector_clock;
pub mod versioned_storage;
pub mod whitelist;

pub mod credential_types;

pub use admin_tiers::*;
pub use concurrency::{
    compare_and_swap, get_pending_conflicts, get_record_conflicts,
    get_resolution_strategy as get_occ_strategy, resolve_conflict,
    set_resolution_strategy as set_occ_strategy, ConflictEntry, ResolutionStrategy as OCCStrategy,
    UpdateOutcome, VersionStamp,
};
pub use credential_types::*;
pub use keys::*;
pub use lineage::{
    LineageEdge, LineageNode, LineageSummary, RelationshipKind, TraversalNode, TraversalResult,
    VerificationResult,
};
pub use meta_tx::*;
pub use metering::*;
pub use migration::*;
pub use multisig::*;
pub use nonce::*;
pub use rate_limit::*;
pub use reentrancy_guard::*;
pub use risk_engine::*;
pub use session::*;
pub use vector_clock::*;
pub use versioned_storage::*;
pub use whitelist::*;

// ── Shared error enum ────────────────────────────────────────────────────────

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
    InvalidNonce = 31,
    NonceOverflow = 32,
    Paused = 40,
    InvalidChannelState = 50,
    InvalidSignature = 51,
    InvalidTransition = 52,
    ChallengePeriodActive = 53,
    AlreadySettled = 54,
}

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

#[cfg(test)]
mod tests {
    use super::CommonError;
    #[test]
    fn common_error_discriminants_are_stable() {
        assert_eq!(CommonError::NotInitialized as u32, 1);
        assert_eq!(CommonError::AccessDenied as u32, 10);
    }
}