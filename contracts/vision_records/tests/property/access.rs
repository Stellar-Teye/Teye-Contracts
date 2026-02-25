#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]
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

        client.revoke_access(&patient, &patient, &grantee);
        prop_assert_eq!(client.check_access(&patient, &grantee), AccessLevel::None);
    }

    /// Revoking access that was never granted must not panic (returns Ok).
    #[test]
    fn prop_idempotent_revoke_never_panics(_seed in 0u8..=255u8) {
        let (env, client) = setup();
        let patient = Address::generate(&env);
        let grantee = Address::generate(&env);

        // No grant was ever issued — revoke should be a no-op
        client.revoke_access(&patient, &patient, &grantee);
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
        client.revoke_access(&patient, &patient, &grantee_a);
        prop_assert_eq!(client.check_access(&patient, &grantee_a), AccessLevel::None);
        prop_assert_eq!(client.check_access(&patient, &grantee_b), level_b);
    }

    /// Time-restricted access: policies should only allow access during specified hours
    #[test]
    fn prop_time_restricted_access(
        hour_seed in 0u8..=23u8,
        business_hours_only in bool::ANY,
    ) {
        let (env, client) = setup();
        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        let researcher = Address::generate(&env);

        // Register users
        client.register_user(&admin, &provider, &vision_records::Role::Optometrist, &String::from_str(&env, "Provider"));
        client.register_user(&admin, &researcher, &vision_records::Role::Staff, &String::from_str(&env, "Researcher"));

        // Create a time-restricted policy
        let time_restriction = if business_hours_only {
            vision_records::TimeRestriction::BusinessHours
        } else {
            vision_records::TimeRestriction::HourRange(hour_seed, (hour_seed + 1) % 24)
        };

        client.create_access_policy(
            &admin,
            &String::from_str(&env, "time_restricted_policy"),
            &String::from_str(&env, "Time Restricted Access"),
            Some(vision_records::Role::Researcher),
            time_restriction,
            vision_records::CredentialType::ResearchCredentials,
            vision_records::SensitivityLevel::Standard,
            false,
        );

        // Set researcher credentials
        client.set_user_credential(&admin, &researcher, vision_records::CredentialType::ResearchCredentials);

        // Create a record
        let record_id = client.add_record(
            &provider,
            &patient,
            &provider,
            &vision_records::RecordType::Examination,
            &String::from_str(&env, "data_hash"),
        ).unwrap();

        // Set record sensitivity
        client.set_record_sensitivity(&provider, &record_id, vision_records::SensitivityLevel::Standard);

        // Test access based on time
        let current_hour = (env.ledger().timestamp() / 3600) % 24;
        let should_allow = if business_hours_only {
            current_hour >= 9 && current_hour <= 17
        } else {
            current_hour == hour_seed as u64 || current_hour == ((hour_seed + 1) % 24) as u64
        };

        let access_result = client.check_record_access(&researcher, &record_id).unwrap();

        if should_allow {
            prop_assert_ne!(access_result, vision_records::AccessLevel::None);
        } else {
            prop_assert_eq!(access_result, vision_records::AccessLevel::None);
        }
    }

    /// Consent-gated access: policies should require active consent
    #[test]
    fn prop_consent_gated_access(
        consent_granted in bool::ANY,
        consent_expired in bool::ANY,
    ) {
        let (env, client) = setup();
        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        let researcher = Address::generate(&env);

        // Register users
        client.register_user(&admin, &provider, &vision_records::Role::Optometrist, &String::from_str(&env, "Provider"));
        client.register_user(&admin, &researcher, &vision_records::Role::Staff, &String::from_str(&env, "Researcher"));

        // Create a consent-required policy
        client.create_access_policy(
            &admin,
            &String::from_str(&env, "consent_required_policy"),
            &String::from_str(&env, "Consent Required Access"),
            Some(vision_records::Role::Researcher),
            vision_records::TimeRestriction::None,
            vision_records::CredentialType::ResearchCredentials,
            vision_records::SensitivityLevel::Standard,
            true, // consent_required
        );

        // Set researcher credentials
        client.set_user_credential(&admin, &researcher, vision_records::CredentialType::ResearchCredentials);

        // Create a record
        let record_id = client.add_record(
            &provider,
            &patient,
            &provider,
            &vision_records::RecordType::Examination,
            &String::from_str(&env, "data_hash"),
        ).unwrap();

        // Set record sensitivity
        client.set_record_sensitivity(&provider, &record_id, vision_records::SensitivityLevel::Standard);

        // Grant consent if required
        if consent_granted {
            let duration = if consent_expired { 1 } else { 86400 }; // 1 second if expired, 1 day if not
            client.grant_consent(
                &patient,
                &researcher,
                &vision_records::ConsentType::Research,
                duration,
            ).unwrap();
        }

        // Test access
        let access_result = client.check_record_access(&researcher, &record_id).unwrap();

        let should_allow = consent_granted && !consent_expired;
        if should_allow {
            prop_assert_ne!(access_result, vision_records::AccessLevel::None);
        } else {
            prop_assert_eq!(access_result, vision_records::AccessLevel::None);
        }
    }

    /// Multi-attribute policies: access should satisfy all conditions
    #[test]
    fn prop_multi_attribute_policies(
        role_match in bool::ANY,
        credential_match in bool::ANY,
        sensitivity_match in bool::ANY,
        consent_match in bool::ANY,
    ) {
        let (env, client) = setup();
        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        let user = Address::generate(&env);

        // Register user with role based on test
        let user_role = if role_match {
            vision_records::Role::Optometrist
        } else {
            vision_records::Role::Staff
        };
        client.register_user(&admin, &user, &user_role, &String::from_str(&env, "User"));

        // Create a multi-attribute policy
        client.create_access_policy(
            &admin,
            &String::from_str(&env, "multi_attribute_policy"),
            &String::from_str(&env, "Multi Attribute Access"),
            Some(vision_records::Role::Optometrist), // Requires Optometrist role
            vision_records::TimeRestriction::BusinessHours, // Requires business hours
            vision_records::CredentialType::MedicalLicense, // Requires medical license
            vision_records::SensitivityLevel::Confidential, // Allows confidential and above
            true, // consent_required
        );

        // Set credentials based on test
        let credential = if credential_match {
            vision_records::CredentialType::MedicalLicense
        } else {
            vision_records::CredentialType::None
        };
        client.set_user_credential(&admin, &user, credential);

        // Create a record
        let record_id = client.add_record(
            &provider,
            &patient,
            &provider,
            &vision_records::RecordType::Examination,
            &String::from_str(&env, "data_hash"),
        ).unwrap();

        // Set record sensitivity based on test
        let sensitivity = if sensitivity_match {
            vision_records::SensitivityLevel::Confidential
        } else {
            vision_records::SensitivityLevel::Restricted
        };
        client.set_record_sensitivity(&provider, &record_id, sensitivity);

        // Grant consent if required
        if consent_match {
            client.grant_consent(
                &patient,
                &user,
                &vision_records::ConsentType::Treatment,
                86400,
            ).unwrap();
        }

        // Test access - all conditions must be met
        let current_hour = (env.ledger().timestamp() / 3600) % 24;
        let is_business_hours = current_hour >= 9 && current_hour <= 17;

        let should_allow = role_match && credential_match && sensitivity_match && consent_match && is_business_hours;

        let access_result = client.check_record_access(&user, &record_id).unwrap();

        if should_allow {
            prop_assert_ne!(access_result, vision_records::AccessLevel::None);
        } else {
            prop_assert_eq!(access_result, vision_records::AccessLevel::None);
        }
    }
}
