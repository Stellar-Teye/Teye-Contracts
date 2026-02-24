#![allow(clippy::unwrap_used, clippy::expect_used, clippy::arithmetic_side_effects)]
//! Property-based tests for the access-control layer.
//!
//! Invariants tested:
//! - Access is always `None` before any grant has been made
//! - `grant_access(level)` → `check_access()` always returns that exact level
//! - `grant_access` → `revoke_access` → `check_access` always returns `None`
//! - Revoking access that was never granted does not panic

use proptest::prelude::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};
use vision_records::{AccessLevel, VisionRecordsContract, VisionRecordsContractClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, VisionRecordsContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client)
}

/// Convert a u8 seed to one of the non-None `AccessLevel` variants.
fn access_level_from_u8(n: u8) -> AccessLevel {
    match n % 3 {
        0 => AccessLevel::Read,
        1 => AccessLevel::Write,
        _ => AccessLevel::Full,
    }
}

// ── proptest! blocks ──────────────────────────────────────────────────────────

proptest! {
    /// For any patient/grantee pair, access is always `None` before any grant is issued.
    #[test]
    fn prop_no_access_before_grant(_seed in 0u8..=255u8) {
        let (env, client) = setup();
        let patient = Address::generate(&env);
        let grantee = Address::generate(&env);

        prop_assert_eq!(client.check_access(&patient, &grantee), AccessLevel::None);
    }

    /// Granting an access level and then checking must return exactly that level.
    #[test]
    fn prop_grant_then_check_matches_level(
        level_seed in 0u8..=255u8,
        duration in 3600u64..=86400u64,
    ) {
        let (env, client) = setup();
        let patient = Address::generate(&env);
        let grantee = Address::generate(&env);
        let level = access_level_from_u8(level_seed);

        client.grant_access(&patient, &patient, &grantee, &level, &duration);
        prop_assert_eq!(client.check_access(&patient, &grantee), level);
    }

    /// Grant followed immediately by revoke must always result in `None`.
    #[test]
    fn prop_grant_then_revoke_returns_none(
        level_seed in 0u8..=255u8,
        duration in 3600u64..=86400u64,
    ) {
        let (env, client) = setup();
        let patient = Address::generate(&env);
        let grantee = Address::generate(&env);
        let level = access_level_from_u8(level_seed);

        client.grant_access(&patient, &patient, &grantee, &level, &duration);
        prop_assert_ne!(client.check_access(&patient, &grantee), AccessLevel::None);

        client.revoke_access(&patient, &grantee);
        prop_assert_eq!(client.check_access(&patient, &grantee), AccessLevel::None);
    }

    /// Revoking access that was never granted must not panic (returns Ok).
    #[test]
    fn prop_idempotent_revoke_never_panics(_seed in 0u8..=255u8) {
        let (env, client) = setup();
        let patient = Address::generate(&env);
        let grantee = Address::generate(&env);

        // No grant was ever issued — revoke should be a no-op
        client.revoke_access(&patient, &grantee);
        prop_assert_eq!(client.check_access(&patient, &grantee), AccessLevel::None);
    }

    /// A re-grant with a different level must always overwrite the previous one.
    #[test]
    fn prop_regrant_overwrites_previous(
        first_seed in 0u8..=1u8,   // Read or Write
        second_seed in 2u8..=2u8,  // Full
        duration in 3600u64..=86400u64,
    ) {
        let (env, client) = setup();
        let patient = Address::generate(&env);
        let grantee = Address::generate(&env);

        let first_level = access_level_from_u8(first_seed);
        let second_level = access_level_from_u8(second_seed);

        client.grant_access(&patient, &patient, &grantee, &first_level, &duration);
        prop_assert_eq!(client.check_access(&patient, &grantee), first_level.clone());

        // Overwrite with second level
        client.grant_access(&patient, &patient, &grantee, &second_level, &duration);
        prop_assert_eq!(client.check_access(&patient, &grantee), second_level);
    }

    /// Granting access to multiple grantees must not interfere with each other.
    #[test]
    fn prop_grants_are_isolated(
        level_a_seed in 0u8..=0u8,  // Read
        level_b_seed in 2u8..=2u8,  // Full
        duration in 3600u64..=86400u64,
    ) {
        let (env, client) = setup();
        let patient = Address::generate(&env);
        let grantee_a = Address::generate(&env);
        let grantee_b = Address::generate(&env);

        let level_a = access_level_from_u8(level_a_seed);
        let level_b = access_level_from_u8(level_b_seed);

        client.grant_access(&patient, &patient, &grantee_a, &level_a, &duration);
        client.grant_access(&patient, &patient, &grantee_b, &level_b, &duration);

        prop_assert_eq!(client.check_access(&patient, &grantee_a), level_a);
        prop_assert_eq!(client.check_access(&patient, &grantee_b), level_b.clone());

        // Revoking grantee_a must not affect grantee_b
        client.revoke_access(&patient, &grantee_a);
        prop_assert_eq!(client.check_access(&patient, &grantee_a), AccessLevel::None);
        prop_assert_eq!(client.check_access(&patient, &grantee_b), level_b);
    }
}
