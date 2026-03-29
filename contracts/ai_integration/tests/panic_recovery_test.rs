#![allow(clippy::unwrap_used)]

extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use ai_integration::{
    AiIntegrationContract, AiIntegrationContractClient, AiIntegrationError, ProviderStatus,
    RequestStatus, VerificationState,
};

fn setup() -> (Env, AiIntegrationContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);

    client.initialize(&admin, &7_000).unwrap();

    (env, client, admin, operator)
}

/// Test panic recovery during provider registration state transition
/// Ensures state is not corrupted if registration fails mid-transition
#[test]
fn test_panic_recovery_provider_registration_invalid_id() {
    let (env, client, admin, operator) = setup();

    // Attempt to register provider with invalid ID (0)
    let result = client.try_register_provider(
        &admin,
        &0, // Invalid provider ID
        &operator,
        &String::from_str(&env, "Provider A"),
        &String::from_str(&env, "retina-v1"),
        &String::from_str(&env, "sha256:endpoint"),
    );

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        AiIntegrationError::InvalidInput
    );

    // Verify state is clean - should be able to register with valid ID
    let result = client.try_register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider A"),
        &String::from_str(&env, "retina-v1"),
        &String::from_str(&env, "sha256:endpoint"),
    );

    assert!(result.is_ok());
    let provider = client.get_provider(&1).unwrap();
    assert_eq!(provider.provider_id, 1);
    assert_eq!(provider.status, ProviderStatus::Active);
}

/// Test panic recovery with empty string validation during registration
#[test]
fn test_panic_recovery_provider_registration_empty_name() {
    let (env, client, admin, operator) = setup();

    let result = client.try_register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, ""), // Empty name
        &String::from_str(&env, "retina-v1"),
        &String::from_str(&env, "sha256:endpoint"),
    );

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        AiIntegrationError::InvalidInput
    );

    // Verify state is clean
    let result = client.try_register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider A"),
        &String::from_str(&env, "retina-v1"),
        &String::from_str(&env, "sha256:endpoint"),
    );

    assert!(result.is_ok());
}

/// Test panic recovery during request submission with invalid provider
#[test]
fn test_panic_recovery_request_submission_invalid_provider() {
    let (env, client, admin, _operator) = setup();
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Try to submit request for non-existent provider
    let result = client.try_submit_analysis_request(
        &requester,
        &999, // Non-existent provider
        &patient,
        &9001,
        &String::from_str(&env, "sha256:image-1"),
        &String::from_str(&env, "retina_screening"),
    );

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        AiIntegrationError::ProviderNotFound
    );

    // Verify request counter wasn't incremented
    let request_counter_before = env
        .storage()
        .instance()
        .get::<_, u64>(&soroban_sdk::symbol_short!("REQCTR"))
        .unwrap_or(0);

    // Register a valid provider and submit request
    client
        .register_provider(
            &admin,
            &1,
            &Address::generate(&env),
            &String::from_str(&env, "Provider A"),
            &String::from_str(&env, "retina-v1"),
            &String::from_str(&env, "sha256:endpoint"),
        )
        .unwrap();

    let request_id = client
        .submit_analysis_request(
            &requester,
            &1,
            &patient,
            &9001,
            &String::from_str(&env, "sha256:image-1"),
            &String::from_str(&env, "retina_screening"),
        )
        .unwrap();

    assert_eq!(request_id, 1);
}

/// Test panic recovery during result storage with invalid state transition
#[test]
fn test_panic_recovery_result_storage_invalid_state() {
    let (env, client, admin, operator) = setup();
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    client
        .register_provider(
            &admin,
            &1,
            &operator,
            &String::from_str(&env, "Provider A"),
            &String::from_str(&env, "retina-v1"),
            &String::from_str(&env, "sha256:endpoint"),
        )
        .unwrap();

    let request_id = client
        .submit_analysis_request(
            &requester,
            &1,
            &patient,
            &9001,
            &String::from_str(&env, "sha256:image-1"),
            &String::from_str(&env, "retina_screening"),
        )
        .unwrap();

    // Store result successfully
    let status = client
        .store_analysis_result(
            &operator,
            &request_id,
            &String::from_str(&env, "sha256:result-1"),
            &8_800,
            &3_500,
        )
        .unwrap();

    assert_eq!(status, RequestStatus::Completed);

    // Try to store result again - should fail with ResultAlreadyExists
    let result = client.try_store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:result-2"),
        &8_800,
        &3_500,
    );

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        AiIntegrationError::ResultAlreadyExists
    );

    // Verify original result is intact
    let stored_result = client.get_analysis_result(&request_id).unwrap();
    assert_eq!(
        stored_result.output_hash,
        String::from_str(&env, "sha256:result-1")
    );
}

/// Test panic recovery during verification with invalid state
#[test]
fn test_panic_recovery_verification_invalid_state() {
    let (env, client, admin, operator) = setup();
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    client
        .register_provider(
            &admin,
            &1,
            &operator,
            &String::from_str(&env, "Provider A"),
            &String::from_str(&env, "retina-v1"),
            &String::from_str(&env, "sha256:endpoint"),
        )
        .unwrap();

    let request_id = client
        .submit_analysis_request(
            &requester,
            &1,
            &patient,
            &9001,
            &String::from_str(&env, "sha256:image-1"),
            &String::from_str(&env, "retina_screening"),
        )
        .unwrap();

    // Try to verify result before storing it
    let result = client.try_verify_analysis_result(
        &admin,
        &request_id,
        &true,
        &String::from_str(&env, "sha256:qa-1"),
    );

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        AiIntegrationError::InvalidState
    );

    // Verify request is still in Pending state
    let request = client.get_analysis_request(&request_id).unwrap();
    assert_eq!(request.status, RequestStatus::Pending);
}

/// Test panic recovery with concurrent state transitions
/// Simulates multiple requests being processed simultaneously
#[test]
fn test_panic_recovery_concurrent_requests() {
    let (env, client, admin, operator) = setup();

    client
        .register_provider(
            &admin,
            &1,
            &operator,
            &String::from_str(&env, "Provider A"),
            &String::from_str(&env, "retina-v1"),
            &String::from_str(&env, "sha256:endpoint"),
        )
        .unwrap();

    // Submit multiple requests
    let mut request_ids = Vec::new();
    for i in 0..5 {
        let requester = Address::generate(&env);
        let patient = Address::generate(&env);

        let request_id = client
            .submit_analysis_request(
                &requester,
                &1,
                &patient,
                &(9000 + i as u64),
                &String::from_str(&env, &format!("sha256:image-{}", i)),
                &String::from_str(&env, "retina_screening"),
            )
            .unwrap();

        request_ids.push(request_id);
    }

    // Verify all requests have unique IDs
    for i in 0..5 {
        assert_eq!(request_ids[i], (i + 1) as u64);
    }

    // Store results for all requests
    for (i, &request_id) in request_ids.iter().enumerate() {
        let status = client
            .store_analysis_result(
                &operator,
                &request_id,
                &String::from_str(&env, &format!("sha256:result-{}", i)),
                &8_800,
                &3_500,
            )
            .unwrap();

        assert_eq!(status, RequestStatus::Completed);
    }

    // Verify all results are stored correctly
    for (i, &request_id) in request_ids.iter().enumerate() {
        let result = client.get_analysis_result(&request_id).unwrap();
        assert_eq!(
            result.output_hash,
            String::from_str(&env, &format!("sha256:result-{}", i))
        );
        assert_eq!(result.verification_state, VerificationState::Unverified);
    }
}

/// Test panic recovery with provider status changes during request processing
#[test]
fn test_panic_recovery_provider_status_change_during_processing() {
    let (env, client, admin, operator) = setup();
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    client
        .register_provider(
            &admin,
            &1,
            &operator,
            &String::from_str(&env, "Provider A"),
            &String::from_str(&env, "retina-v1"),
            &String::from_str(&env, "sha256:endpoint"),
        )
        .unwrap();

    let request_id = client
        .submit_analysis_request(
            &requester,
            &1,
            &patient,
            &9001,
            &String::from_str(&env, "sha256:image-1"),
            &String::from_str(&env, "retina_screening"),
        )
        .unwrap();

    // Pause the provider
    client
        .set_provider_status(&admin, &1, &ProviderStatus::Paused)
        .unwrap();

    // Try to store result with paused provider
    let result = client.try_store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:result-1"),
        &8_800,
        &3_500,
    );

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        AiIntegrationError::ProviderInactive
    );

    // Verify request is still in Pending state
    let request = client.get_analysis_request(&request_id).unwrap();
    assert_eq!(request.status, RequestStatus::Pending);

    // Reactivate provider and retry
    client
        .set_provider_status(&admin, &1, &ProviderStatus::Active)
        .unwrap();

    let status = client
        .store_analysis_result(
            &operator,
            &request_id,
            &String::from_str(&env, "sha256:result-1"),
            &8_800,
            &3_500,
        )
        .unwrap();

    assert_eq!(status, RequestStatus::Completed);
}

/// Test panic recovery with anomaly threshold boundary conditions
#[test]
fn test_panic_recovery_anomaly_threshold_boundaries() {
    let (env, client, admin, operator) = setup();
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Set threshold to maximum
    client.set_anomaly_threshold(&admin, &10_000).unwrap();

    client
        .register_provider(
            &admin,
            &1,
            &operator,
            &String::from_str(&env, "Provider A"),
            &String::from_str(&env, "retina-v1"),
            &String::from_str(&env, "sha256:endpoint"),
        )
        .unwrap();

    let request_id = client
        .submit_analysis_request(
            &requester,
            &1,
            &patient,
            &9001,
            &String::from_str(&env, "sha256:image-1"),
            &String::from_str(&env, "retina_screening"),
        )
        .unwrap();

    // Store result with max anomaly score - should not be flagged
    let status = client
        .store_analysis_result(
            &operator,
            &request_id,
            &String::from_str(&env, "sha256:result-1"),
            &10_000,
            &10_000,
        )
        .unwrap();

    assert_eq!(status, RequestStatus::Completed);

    // Try to set invalid threshold
    let result = client.try_set_anomaly_threshold(&admin, &10_001);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        AiIntegrationError::InvalidInput
    );

    // Verify threshold is still at max
    let threshold = client.get_anomaly_threshold().unwrap();
    assert_eq!(threshold, 10_000);
}

/// Test panic recovery with request counter overflow protection
#[test]
fn test_panic_recovery_request_counter_saturation() {
    let (env, client, admin, operator) = setup();

    client
        .register_provider(
            &admin,
            &1,
            &operator,
            &String::from_str(&env, "Provider A"),
            &String::from_str(&env, "retina-v1"),
            &String::from_str(&env, "sha256:endpoint"),
        )
        .unwrap();

    // Manually set request counter to near u64::MAX
    let counter_key = soroban_sdk::symbol_short!("REQCTR");
    env.storage().instance().set(&counter_key, &(u64::MAX - 1));

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Submit request - should use saturating_add
    let request_id_1 = client
        .submit_analysis_request(
            &requester,
            &1,
            &patient,
            &9001,
            &String::from_str(&env, "sha256:image-1"),
            &String::from_str(&env, "retina_screening"),
        )
        .unwrap();

    assert_eq!(request_id_1, u64::MAX);

    // Submit another request - should saturate at u64::MAX
    let request_id_2 = client
        .submit_analysis_request(
            &requester,
            &1,
            &patient,
            &9002,
            &String::from_str(&env, "sha256:image-2"),
            &String::from_str(&env, "retina_screening"),
        )
        .unwrap();

    assert_eq!(request_id_2, u64::MAX);
}

/// Test panic recovery with verification rejection state transition
#[test]
fn test_panic_recovery_verification_rejection_state() {
    let (env, client, admin, operator) = setup();
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    client
        .register_provider(
            &admin,
            &1,
            &operator,
            &String::from_str(&env, "Provider A"),
            &String::from_str(&env, "retina-v1"),
            &String::from_str(&env, "sha256:endpoint"),
        )
        .unwrap();

    let request_id = client
        .submit_analysis_request(
            &requester,
            &1,
            &patient,
            &9001,
            &String::from_str(&env, "sha256:image-1"),
            &String::from_str(&env, "retina_screening"),
        )
        .unwrap();

    client
        .store_analysis_result(
            &operator,
            &request_id,
            &String::from_str(&env, "sha256:result-1"),
            &8_800,
            &3_500,
        )
        .unwrap();

    // Reject the result
    client
        .verify_analysis_result(
            &admin,
            &request_id,
            &false,
            &String::from_str(&env, "sha256:qa-reject"),
        )
        .unwrap();

    // Verify both request and result are in rejected state
    let request = client.get_analysis_request(&request_id).unwrap();
    assert_eq!(request.status, RequestStatus::Rejected);

    let result = client.get_analysis_result(&request_id).unwrap();
    assert_eq!(result.verification_state, VerificationState::Rejected);

    // Try to verify again - should fail with InvalidState
    let result = client.try_verify_analysis_result(
        &admin,
        &request_id,
        &true,
        &String::from_str(&env, "sha256:qa-retry"),
    );

    assert!(result.is_err());
}
