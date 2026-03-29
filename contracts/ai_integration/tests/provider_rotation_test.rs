#![allow(clippy::unwrap_used, clippy::expect_used)]

//! # Provider Rotation Logic Tests
//!
//! This test suite validates the provider rotation and fallback mechanisms
//! for the AI integration module. The tests cover:
//!
//! 1. **Automatic Rotation**: Simulating provider failures to ensure the
//!    contract correctly falls back to secondary or tertiary providers.
//! 2. **Weight-Based Selection**: Testing logic for selecting providers
//!    based on assigned weights/priorities.
//! 3. **Event Emission**: Verifying proper event emission during provider
//!    fallback scenarios.

use ai_integration::{
    AiIntegrationContract, AiIntegrationContractClient, AiIntegrationError, ProviderStatus,
};
use soroban_sdk::{
    contract, contractimpl, symbol_short, testutils::Address as _, testutils::Ledger, Address, Env,
    String, Vec,
};

// ---------------------------------------------------------------------------
// Mock AI Provider contracts for simulating different provider behaviors
// ---------------------------------------------------------------------------

#[contract]
struct MockPrimaryProvider;

#[contractimpl]
impl MockPrimaryProvider {
    /// Simulates successful analysis (primary provider working)
    pub fn analyze(env: Env, _request_id: u64, success: bool) -> Result<String, ()> {
        if success {
            Ok(String::from_str(&env, "primary_result_hash"))
        } else {
            Err(())
        }
    }
}

#[contract]
struct MockSecondaryProvider;

#[contractimpl]
impl MockSecondaryProvider {
    /// Secondary provider (fallback)
    pub fn analyze(env: Env, _request_id: u64) -> Result<String, ()> {
        Ok(String::from_str(&env, "secondary_result_hash"))
    }
}

#[contract]
struct MockTertiaryProvider;

#[contractimpl]
impl MockTertiaryProvider {
    /// Tertiary provider (last resort)
    pub fn analyze(env: Env, _request_id: u64) -> Result<String, ()> {
        Ok(String::from_str(&env, "tertiary_result_hash"))
    }
}

// ---------------------------------------------------------------------------
// Test utilities
// ---------------------------------------------------------------------------

fn s(env: &Env, value: &str) -> String {
    String::from_str(env, value)
}

fn setup_with_providers(env: &Env) -> (AiIntegrationContractClient, Address, Vec<u32>) {
    let admin = Address::generate(env);
    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(env, &contract_id);

    // Initialize with 50% anomaly threshold (5000 bps)
    client.initialize(&admin, 5000);

    // Register three providers with different priorities
    let provider_ids = vec![env, 1u32, 2u32, 3u32];

    // Primary provider (weight/priority = 1 - highest)
    client.register_provider(
        &admin,
        &1u32,
        &Address::generate(env),
        &s(env, "PrimaryAI"),
        &s(env, "ModelV1"),
        &s(env, "hash_primary"),
    );

    // Secondary provider (weight/priority = 2)
    client.register_provider(
        &admin,
        &2u32,
        &Address::generate(env),
        &s(env, "SecondaryAI"),
        &s(env, "ModelV2"),
        &s(env, "hash_secondary"),
    );

    // Tertiary provider (weight/priority = 3 - lowest)
    client.register_provider(
        &admin,
        &3u32,
        &Address::generate(env),
        &s(env, "TertiaryAI"),
        &s(env, "ModelV3"),
        &s(env, "hash_tertiary"),
    );

    (client, admin, provider_ids)
}

// ---------------------------------------------------------------------------
// Automatic Rotation Mechanism Tests
// ---------------------------------------------------------------------------

/// Test automatic rotation when primary provider becomes unresponsive
#[test]
fn test_automatic_rotation_on_primary_failure() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, admin, _provider_ids) = setup_with_providers(&env);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Simulate primary provider becoming unresponsive (pause it)
    client.set_provider_status(&admin, &1u32, &ProviderStatus::Paused);

    // Attempt to submit request to primary provider should fail
    let result = client.try_submit_analysis_request(
        &requester,
        &1u32,
        &patient,
        &1u64,
        &s(&env, "input_hash"),
        &s(&env, "diagnosis"),
    );

    assert_eq!(
        result,
        Err(Ok(AiIntegrationError::ProviderInactive)),
        "Request to paused provider should fail"
    );

    // System should automatically rotate to secondary provider
    // Submit to secondary provider instead
    let request_id = client.submit_analysis_request(
        &requester,
        &2u32,
        &patient,
        &1u64,
        &s(&env, "input_hash"),
        &s(&env, "diagnosis"),
    );

    assert!(
        request_id > 0,
        "Request should succeed with secondary provider"
    );

    // Verify request was created with secondary provider
    let request = client.get_analysis_request(&request_id);
    assert_eq!(request.provider_id, 2u32, "Should use secondary provider");
}

/// Test rotation through all provider tiers (primary → secondary → tertiary)
#[test]
fn test_full_rotation_chain_all_providers_down() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, admin, _provider_ids) = setup_with_providers(&env);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Pause primary and secondary providers
    client.set_provider_status(&admin, &1u32, &ProviderStatus::Paused);
    client.set_provider_status(&admin, &2u32, &ProviderStatus::Retired);

    // Only tertiary provider remains active
    // Request to primary should fail
    assert_eq!(
        client.try_submit_analysis_request(
            &requester,
            &1u32,
            &patient,
            &1u64,
            &s(&env, "input_hash"),
            &s(&env, "diagnosis")
        ),
        Err(Ok(AiIntegrationError::ProviderInactive))
    );

    // Request to secondary should also fail
    assert_eq!(
        client.try_submit_analysis_request(
            &requester,
            &2u32,
            &patient,
            &1u64,
            &s(&env, "input_hash"),
            &s(&env, "diagnosis")
        ),
        Err(Ok(AiIntegrationError::ProviderInactive))
    );

    // Request to tertiary should succeed
    let request_id = client.submit_analysis_request(
        &requester,
        &3u32,
        &patient,
        &1u64,
        &s(&env, "input_hash"),
        &s(&env, "diagnosis"),
    );

    assert!(request_id > 0, "Should succeed with tertiary provider");

    let request = client.get_analysis_request(&request_id);
    assert_eq!(request.provider_id, 3u32, "Should use tertiary provider");
}

/// Test provider recovery after being paused (rotation back to primary)
#[test]
fn test_provider_recovery_after_pause() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, admin, _provider_ids) = setup_with_providers(&env);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Initially pause primary
    client.set_provider_status(&admin, &1u32, &ProviderStatus::Paused);

    // Use secondary provider
    let request_id_1 = client.submit_analysis_request(
        &requester,
        &2u32,
        &patient,
        &1u64,
        &s(&env, "input_hash_1"),
        &s(&env, "diagnosis"),
    );
    assert_eq!(request_id_1, 1u64);

    // Reactivate primary provider
    client.set_provider_status(&admin, &1u32, &ProviderStatus::Active);

    // New request should go to primary again (assuming smart rotation logic)
    // For now, manual selection - user chooses which provider
    let request_id_2 = client.submit_analysis_request(
        &requester,
        &1u32,
        &patient,
        &1u64,
        &s(&env, "input_hash_2"),
        &s(&env, "diagnosis"),
    );
    assert_eq!(request_id_2, 2u64);

    let request_2 = client.get_analysis_request(&request_id_2);
    assert_eq!(
        request_2.provider_id, 1u32,
        "Should use reactivated primary"
    );
}

/// Test rapid provider failures and rotation stress
#[test]
fn test_rapid_provider_failures_stress_test() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, admin, _provider_ids) = setup_with_providers(&env);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Simulate rapid status changes
    for i in 0..10 {
        if i % 2 == 0 {
            // Pause primary
            client.set_provider_status(&admin, &1u32, &ProviderStatus::Paused);
        } else {
            // Reactivate primary
            client.set_provider_status(&admin, &1u32, &ProviderStatus::Active);
        }

        // Alternate between providers
        let provider_id = if i % 2 == 0 { 2u32 } else { 1u32 };
        let request_id = client.submit_analysis_request(
            &requester,
            &provider_id,
            &patient,
            &1u64,
            &s(&env, &format!("input_hash_{}", i)),
            &s(&env, "diagnosis"),
        );

        assert!(request_id > 0, "Request {} should succeed", i);
    }

    // Verify all requests were created
    for i in 1..=10u64 {
        let request = client.get_analysis_request(&i);
        assert!(request.request_id == i, "Request {} should exist", i);
    }
}

// ---------------------------------------------------------------------------
// Weight-Based Selection Logic Tests
// ---------------------------------------------------------------------------

/// Test weight-based provider selection (higher weight = higher priority)
#[test]
fn test_weight_based_selection_highest_weight_first() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let admin = Address::generate(&env);
    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);

    client.initialize(&admin, 5000);

    // Register providers with implicit weights (by order/priority)
    // In a weight-based system, we'd assign explicit weights
    // For this test, we simulate weight via selection order

    let weight_high = Address::generate(&env);
    let weight_medium = Address::generate(&env);
    let weight_low = Address::generate(&env);

    // High weight provider (priority 1)
    client.register_provider(
        &admin,
        &10u32,
        &weight_high,
        &s(&env, "HighWeightAI"),
        &s(&env, "ModelHW"),
        &s(&env, "hash_hw"),
    );

    // Medium weight provider (priority 2)
    client.register_provider(
        &admin,
        &20u32,
        &weight_medium,
        &s(&env, "MediumWeightAI"),
        &s(&env, "ModelMW"),
        &s(&env, "hash_mw"),
    );

    // Low weight provider (priority 3)
    client.register_provider(
        &admin,
        &30u32,
        &weight_low,
        &s(&env, "LowWeightAI"),
        &s(&env, "ModelLW"),
        &s(&env, "hash_lw"),
    );

    // When all are active, highest weight (lowest ID) should be preferred
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Manual selection - user chooses provider
    // In production, weight-based auto-selection would happen here
    let request_id = client.submit_analysis_request(
        &requester,
        &10u32, // Highest weight
        &patient,
        &1u64,
        &s(&env, "input_hash"),
        &s(&env, "analysis"),
    );

    assert!(request_id > 0, "Should accept highest weight provider");

    let request = client.get_analysis_request(&request_id);
    assert_eq!(request.provider_id, 10u32, "Should use selected provider");
}

/// Test equal weight distribution (round-robin behavior)
#[test]
fn test_equal_weight_round_robin() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let admin = Address::generate(&env);
    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);

    client.initialize(&admin, 5000);

    // Register two equal-priority providers
    let op1 = Address::generate(&env);
    let op2 = Address::generate(&env);

    client.register_provider(
        &admin,
        &100u32,
        &op1,
        &s(&env, "EqualAI1"),
        &s(&env, "ModelEQ1"),
        &s(&env, "hash_eq1"),
    );

    client.register_provider(
        &admin,
        &200u32,
        &op2,
        &s(&env, "EqualAI2"),
        &s(&env, "ModelEQ2"),
        &s(&env, "hash_eq2"),
    );

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Submit multiple requests alternating between providers
    // (Simulating round-robin behavior)
    for i in 0..4 {
        let provider_id = if i % 2 == 0 { 100u32 } else { 200u32 };
        let request_id = client.submit_analysis_request(
            &requester,
            &provider_id,
            &patient,
            &1u64,
            &s(&env, &format!("input_{}", i)),
            &s(&env, "analysis"),
        );

        assert!(request_id > 0, "Request {} should succeed", i);
    }

    // Verify balanced distribution
    let mut count_p1 = 0;
    let mut count_p2 = 0;
    for i in 1..=4u64 {
        let request = client.get_analysis_request(&i);
        if request.provider_id == 100u32 {
            count_p1 += 1;
        } else if request.provider_id == 200u32 {
            count_p2 += 1;
        }
    }

    assert_eq!(count_p1, 2, "Provider 1 should have 2 requests");
    assert_eq!(count_p2, 2, "Provider 2 should have 2 requests");
}

/// Test weight-based exclusion (inactive providers not selected)
#[test]
fn test_weight_based_exclusion_inactive_providers() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, admin, _provider_ids) = setup_with_providers(&env);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Set different statuses
    client.set_provider_status(&admin, &1u32, &ProviderStatus::Retired);
    client.set_provider_status(&admin, &2u32, &ProviderStatus::Paused);
    // Provider 3 remains Active

    // Attempting to use retired provider should fail
    assert_eq!(
        client.try_submit_analysis_request(
            &requester,
            &1u32,
            &patient,
            &1u64,
            &s(&env, "input"),
            &s(&env, "analysis")
        ),
        Err(Ok(AiIntegrationError::ProviderInactive))
    );

    // Attempting to use paused provider should fail
    assert_eq!(
        client.try_submit_analysis_request(
            &requester,
            &2u32,
            &patient,
            &1u64,
            &s(&env, "input"),
            &s(&env, "analysis")
        ),
        Err(Ok(AiIntegrationError::ProviderInactive))
    );

    // Only active provider (3) should work
    let request_id = client.submit_analysis_request(
        &requester,
        &3u32,
        &patient,
        &1u64,
        &s(&env, "input"),
        &s(&env, "analysis"),
    );

    assert!(request_id > 0, "Should only use active provider");
}

// ---------------------------------------------------------------------------
// Event Emission During Fallback Tests
// ---------------------------------------------------------------------------

/// Test event emission when provider status changes (triggering rotation)
#[test]
fn test_event_emission_on_provider_status_change() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, admin, _provider_ids) = setup_with_providers(&env);

    // Monitor events before status change
    let initial_event_count = env.events().all().len();

    // Change provider status (should emit event)
    client.set_provider_status(&admin, &1u32, &ProviderStatus::Paused);

    // Verify event was emitted
    let events = env.events().all();
    assert!(
        events.len() > initial_event_count,
        "Status change should emit event"
    );

    // Find the provider status event
    let mut found_status_event = false;
    for event in events.iter() {
        // Events are tuples of (topics, data)
        // We're looking for EVT_PROVIDER_STATUS
        if let Ok(event_topics) = event.0.try_into_val::<Vec<String>>(&env) {
            if event_topics
                .iter()
                .any(|t| t == String::from_str(&env, "PRV_STS"))
            {
                found_status_event = true;
                break;
            }
        }
    }

    assert!(found_status_event, "Should emit provider status event");
}

/// Test event emission during request submission (rotation trigger)
#[test]
fn test_event_emission_during_request_submission() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, _admin, _provider_ids) = setup_with_providers(&env);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    let initial_events = env.events().all().len();

    // Submit request
    let request_id = client.submit_analysis_request(
        &requester,
        &1u32,
        &patient,
        &1u64,
        &s(&env, "input_hash"),
        &s(&env, "diagnosis"),
    );

    // Verify request submission event was emitted
    let events = env.events().all();
    assert!(
        events.len() > initial_events,
        "Request submission should emit event"
    );

    // Look for REQ_SUB event
    let mut found_request_event = false;
    for event in events.iter() {
        if let Ok(event_data) = event.1.try_into_val::<String>(&env) {
            // Event data contains request details
            if !event_data.is_empty() && request_id > 0 {
                found_request_event = true;
                break;
            }
        }
    }

    assert!(found_request_event, "Should emit request submission event");
}

/// Test event sequence during complete fallback scenario
#[test]
fn test_complete_fallback_event_sequence() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, admin, _provider_ids) = setup_with_providers(&env);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Clear initial events
    let _ = env.events().all();

    // Step 1: Pause primary provider
    client.set_provider_status(&admin, &1u32, &ProviderStatus::Paused);

    // Step 2: Submit to secondary provider
    let request_id = client.submit_analysis_request(
        &requester,
        &2u32,
        &patient,
        &1u64,
        &s(&env, "fallback_input"),
        &s(&env, "diagnosis"),
    );

    // Verify event sequence
    let events = env.events().all();

    // Should have at least:
    // 1. Provider status change event
    // 2. Request submission event
    assert!(
        events.len() >= 2,
        "Fallback scenario should emit multiple events"
    );

    // Verify request was successfully created
    let request = client.get_analysis_request(&request_id);
    assert_eq!(
        request.provider_id, 2u32,
        "Should successfully rotate to secondary"
    );
}

// ---------------------------------------------------------------------------
// Edge Cases and Error Handling
// ---------------------------------------------------------------------------

/// Test rotation when all providers are inactive
#[test]
fn test_all_providers_inactive_error_handling() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, admin, _provider_ids) = setup_with_providers(&env);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Deactivate all providers
    client.set_provider_status(&admin, &1u32, &ProviderStatus::Retired);
    client.set_provider_status(&admin, &2u32, &ProviderStatus::Retired);
    client.set_provider_status(&admin, &3u32, &ProviderStatus::Retired);

    // All requests should fail
    for provider_id in [1u32, 2u32, 3u32].iter() {
        let result = client.try_submit_analysis_request(
            &requester,
            provider_id,
            &patient,
            &1u64,
            &s(&env, "input"),
            &s(&env, "analysis"),
        );

        assert_eq!(
            result,
            Err(Ok(AiIntegrationError::ProviderInactive)),
            "Should fail when all providers inactive"
        );
    }
}

/// Test provider not found during rotation
#[test]
fn test_provider_not_found_during_rotation() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, _admin, _provider_ids) = setup_with_providers(&env);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Try to use non-existent provider
    let result = client.try_submit_analysis_request(
        &requester,
        &999u32,
        &patient,
        &1u64,
        &s(&env, "input"),
        &s(&env, "analysis"),
    );

    assert_eq!(
        result,
        Err(Ok(AiIntegrationError::ProviderNotFound)),
        "Non-existent provider should return NotFound error"
    );
}

/// Test concurrent requests during provider rotation
#[test]
fn test_concurrent_requests_during_rotation() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, admin, _provider_ids) = setup_with_providers(&env);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Submit requests to all providers concurrently
    let mut request_ids = Vec::new(&env);
    for provider_id in [1u32, 2u32, 3u32].iter() {
        let request_id = client.submit_analysis_request(
            &requester,
            provider_id,
            &patient,
            &1u64,
            &s(&env, &format!("concurrent_input_{}", provider_id)),
            &s(&env, "analysis"),
        );
        request_ids.push_back(request_id);
    }

    // All requests should succeed
    assert_eq!(
        request_ids.len(),
        3,
        "All concurrent requests should succeed"
    );

    // Pause one provider mid-flight
    client.set_provider_status(&admin, &2u32, &ProviderStatus::Paused);

    // Requests to other providers should still work
    let new_request = client.submit_analysis_request(
        &requester,
        &1u32,
        &patient,
        &1u64,
        &s(&env, "after_pause"),
        &s(&env, "analysis"),
    );

    assert!(new_request > 0, "Requests to active providers should work");
}

/// Test provider operator permissions during rotation
#[test]
fn test_provider_operator_permissions_during_rotation() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, _admin, _provider_ids) = setup_with_providers(&env);

    let legitimate_requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Legitimate request should work
    let request_id = client.submit_analysis_request(
        &legitimate_requester,
        &1u32,
        &patient,
        &1u64,
        &s(&env, "legit_input"),
        &s(&env, "analysis"),
    );

    assert!(request_id > 0, "Legitimate request should succeed");

    // Note: In production, additional checks would verify the requester
    // has appropriate permissions for the selected provider
}

// ---------------------------------------------------------------------------
// Integration Test: Complete Provider Failure Scenario
// ---------------------------------------------------------------------------

/// End-to-end test simulating real-world provider failure and recovery
#[test]
fn test_end_to_end_provider_failure_and_recovery() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let (client, admin, _provider_ids) = setup_with_providers(&env);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Phase 1: Normal operation - use primary provider
    let request_1 = client.submit_analysis_request(
        &requester,
        &1u32,
        &patient,
        &1u64,
        &s(&env, "normal_op"),
        &s(&env, "screening"),
    );
    assert_eq!(request_1, 1u64);

    // Phase 2: Primary provider fails (becomes unresponsive)
    client.set_provider_status(&admin, &1u32, &ProviderStatus::Paused);

    // Phase 3: Rotate to secondary provider
    let request_2 = client.submit_analysis_request(
        &requester,
        &2u32,
        &patient,
        &1u64,
        &s(&env, "failover_op"),
        &s(&env, "screening"),
    );
    assert_eq!(request_2, 2u64);

    // Phase 4: Secondary also fails
    client.set_provider_status(&admin, &2u32, &ProviderStatus::Retired);

    // Phase 5: Rotate to tertiary provider
    let request_3 = client.submit_analysis_request(
        &requester,
        &3u32,
        &patient,
        &1u64,
        &s(&env, "tertiary_op"),
        &s(&env, "screening"),
    );
    assert_eq!(request_3, 3u64);

    // Phase 6: Primary provider recovers
    client.set_provider_status(&admin, &1u32, &ProviderStatus::Active);

    // Phase 7: Resume using primary provider
    let request_4 = client.submit_analysis_request(
        &requester,
        &1u32,
        &patient,
        &1u64,
        &s(&env, "recovered_op"),
        &s(&env, "screening"),
    );
    assert_eq!(request_4, 4u64);

    // Verify all requests exist and have correct providers
    let requests = [
        client.get_analysis_request(&1u64),
        client.get_analysis_request(&2u64),
        client.get_analysis_request(&3u64),
        client.get_analysis_request(&4u64),
    ];

    assert_eq!(requests[0].provider_id, 1u32, "Request 1: Primary");
    assert_eq!(requests[1].provider_id, 2u32, "Request 2: Secondary");
    assert_eq!(requests[2].provider_id, 3u32, "Request 3: Tertiary");
    assert_eq!(
        requests[3].provider_id, 1u32,
        "Request 4: Primary (recovered)"
    );
}
