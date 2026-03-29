#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_integration::{
    AiIntegrationContract, AiIntegrationContractClient, AiIntegrationError, ProviderStatus,
    RequestStatus, VerificationState,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup(
    anomaly_threshold_bps: u32,
) -> (Env, AiIntegrationContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &anomaly_threshold_bps);
    client.register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider-A"),
        &String::from_str(&env, "gpt-4o"),
        &String::from_str(&env, "sha256:endpoint"),
    );
    (env, client, admin, operator)
}

fn submit_and_store(
    env: &Env,
    client: &AiIntegrationContractClient,
    operator: &Address,
    confidence_bps: u32,
    anomaly_score_bps: u32,
) -> u64 {
    let requester = Address::generate(env);
    let patient = Address::generate(env);
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &1u64,
        &String::from_str(env, "sha256:scan"),
        &String::from_str(env, "retina_triage"),
    );
    client.store_analysis_result(
        operator,
        &request_id,
        &String::from_str(env, "sha256:output"),
        &confidence_bps,
        &anomaly_score_bps,
    );
    request_id
}

// ── Successful inference verification ────────────────────────────────────────

#[test]
fn test_verify_result_accepted_sets_verified_state() {
    let (env, client, admin, operator) = setup(5_000);
    let request_id = submit_and_store(&env, &client, &operator, 8_000, 0);

    client.verify_analysis_result(
        &admin,
        &request_id,
        &true,
        &String::from_str(&env, "sha256:verification"),
    );

    let result = client.get_analysis_result(&request_id);
    assert_eq!(result.verification_state, VerificationState::Verified);
    assert_eq!(result.verified_by, Some(admin));
    assert!(result.verified_at.is_some());
    assert_eq!(
        result.verification_hash,
        Some(String::from_str(&env, "sha256:verification"))
    );
}

#[test]
fn test_verify_result_rejected_sets_rejected_state_and_request() {
    let (env, client, admin, operator) = setup(5_000);
    let request_id = submit_and_store(&env, &client, &operator, 7_000, 0);

    client.verify_analysis_result(
        &admin,
        &request_id,
        &false,
        &String::from_str(&env, "sha256:rejection-hash"),
    );

    let result = client.get_analysis_result(&request_id);
    assert_eq!(result.verification_state, VerificationState::Rejected);

    let request = client.get_analysis_request(&request_id);
    assert_eq!(request.status, RequestStatus::Rejected);
}

#[test]
fn test_successful_inference_full_lifecycle() {
    let (env, client, admin, operator) = setup(7_000);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &42u64,
        &String::from_str(&env, "sha256:input-data"),
        &String::from_str(&env, "cardiac_analysis"),
    );

    let request = client.get_analysis_request(&request_id);
    assert_eq!(request.status, RequestStatus::Pending);
    assert_eq!(request.provider_id, 1);

    let status = client.store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:output-data"),
        &9_000,
        &5_000, // below threshold of 7_000 → Completed
    );
    assert_eq!(status, RequestStatus::Completed);

    client.verify_analysis_result(
        &admin,
        &request_id,
        &true,
        &String::from_str(&env, "sha256:verified"),
    );

    let result = client.get_analysis_result(&request_id);
    assert_eq!(result.confidence_bps, 9_000);
    assert_eq!(result.anomaly_score_bps, 5_000);
    assert_eq!(result.verification_state, VerificationState::Verified);
}

// ── Malformed / out-of-range response values ─────────────────────────────────

#[test]
fn test_store_result_rejects_confidence_above_max_bps() {
    let (env, client, _admin, operator) = setup(5_000);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &1u64,
        &String::from_str(&env, "sha256:scan"),
        &String::from_str(&env, "retina_triage"),
    );

    let result = client.try_store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:output"),
        &10_001, // above MAX_BPS
        &0,
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidInput)));
}

#[test]
fn test_store_result_rejects_anomaly_score_above_max_bps() {
    let (env, client, _admin, operator) = setup(5_000);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &1u64,
        &String::from_str(&env, "sha256:scan"),
        &String::from_str(&env, "retina_triage"),
    );

    let result = client.try_store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:output"),
        &0,
        &10_001, // above MAX_BPS
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidInput)));
}

#[test]
fn test_store_result_accepts_max_bps_boundary_values() {
    let (env, client, _admin, operator) = setup(10_000);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &1u64,
        &String::from_str(&env, "sha256:scan"),
        &String::from_str(&env, "retina_triage"),
    );

    // Exactly MAX_BPS (10_000) must be accepted.
    // anomaly_score == threshold → Flagged.
    let status = client.store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:output"),
        &10_000,
        &10_000,
    );

    assert_eq!(status, RequestStatus::Flagged);
}

#[test]
fn test_anomaly_at_threshold_boundary_flags_request() {
    let (env, client, _admin, operator) = setup(6_000);
    let request_id = submit_and_store(&env, &client, &operator, 5_000, 6_000);

    // anomaly_score_bps == threshold → Flagged (>= check)
    let request = client.get_analysis_request(&request_id);
    assert_eq!(request.status, RequestStatus::Flagged);

    let flagged = client.get_flagged_requests();
    assert!(flagged.contains(&request_id));
}

#[test]
fn test_anomaly_one_below_threshold_completes_request() {
    let (env, client, _admin, operator) = setup(6_000);
    let request_id = submit_and_store(&env, &client, &operator, 5_000, 5_999);

    let request = client.get_analysis_request(&request_id);
    assert_eq!(request.status, RequestStatus::Completed);

    let flagged = client.get_flagged_requests();
    assert!(!flagged.contains(&request_id));
}

// ── Invalid provider signatures ───────────────────────────────────────────────

#[test]
fn test_store_result_by_wrong_operator_is_unauthorized() {
    let (env, client, _admin, _operator) = setup(5_000);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &1u64,
        &String::from_str(&env, "sha256:scan"),
        &String::from_str(&env, "retina_triage"),
    );

    let wrong_operator = Address::generate(&env);
    let result = client.try_store_analysis_result(
        &wrong_operator,
        &request_id,
        &String::from_str(&env, "sha256:output"),
        &5_000,
        &0,
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));
}

#[test]
fn test_store_result_for_paused_provider_is_inactive() {
    let (env, client, admin, operator) = setup(5_000);
    client.set_provider_status(&admin, &1, &ProviderStatus::Paused);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // submit while active, then pause before the operator tries to store
    client.set_provider_status(&admin, &1, &ProviderStatus::Active);
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &1u64,
        &String::from_str(&env, "sha256:scan"),
        &String::from_str(&env, "retina_triage"),
    );
    client.set_provider_status(&admin, &1, &ProviderStatus::Paused);

    let result = client.try_store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:output"),
        &5_000,
        &0,
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::ProviderInactive)));
}

#[test]
fn test_store_result_for_retired_provider_is_inactive() {
    let (env, client, admin, operator) = setup(5_000);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &1u64,
        &String::from_str(&env, "sha256:scan"),
        &String::from_str(&env, "retina_triage"),
    );
    client.set_provider_status(&admin, &1, &ProviderStatus::Retired);

    let result = client.try_store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:output"),
        &5_000,
        &0,
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::ProviderInactive)));
}

#[test]
fn test_store_result_twice_is_rejected() {
    let (env, client, _admin, operator) = setup(5_000);
    let request_id = submit_and_store(&env, &client, &operator, 5_000, 0);

    let result = client.try_store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:output-2"),
        &5_000,
        &0,
    );

    // The request is no longer Pending after the first store, so the
    // non-pending state check fires before the duplicate-result check.
    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidState)));
}

#[test]
fn test_verify_result_for_nonexistent_request_is_not_found() {
    let (env, client, admin, _operator) = setup(5_000);

    let result = client.try_verify_analysis_result(
        &admin,
        &999u64,
        &true,
        &String::from_str(&env, "sha256:hash"),
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::RequestNotFound)));
}

#[test]
fn test_non_admin_cannot_verify_result() {
    let (env, client, admin, operator) = setup(5_000);
    let request_id = submit_and_store(&env, &client, &operator, 8_000, 0);

    let non_admin = Address::generate(&env);
    let result = client.try_verify_analysis_result(
        &non_admin,
        &request_id,
        &true,
        &String::from_str(&env, "sha256:hash"),
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));
    let _ = admin; // keep admin in scope
}
