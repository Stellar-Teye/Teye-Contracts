#![allow(clippy::unwrap_used, clippy::expect_used)]

extern crate std;

use ai_integration::{
    AiIntegrationContract, AiIntegrationContractClient, AiIntegrationError, ProviderStatus,
    RequestStatus,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup_initialized(
    anomaly_threshold_bps: u32,
) -> (Env, AiIntegrationContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &anomaly_threshold_bps);

    (env, client, admin, operator)
}

#[test]
fn test_register_provider_rejects_zero_provider_id() {
    let (env, client, admin, operator) = setup_initialized(5_000);

    let result = client.try_register_provider(
        &admin,
        &0,
        &operator,
        &String::from_str(&env, "Provider"),
        &String::from_str(&env, "model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidInput)));
}

#[test]
fn test_register_provider_rejects_empty_name() {
    let (env, client, admin, operator) = setup_initialized(5_000);

    let result = client.try_register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, ""),
        &String::from_str(&env, "model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidInput)));
}

#[test]
fn test_register_provider_rejects_empty_model() {
    let (env, client, admin, operator) = setup_initialized(5_000);

    let result = client.try_register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider"),
        &String::from_str(&env, ""),
        &String::from_str(&env, "endpoint-hash"),
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidInput)));
}

#[test]
fn test_register_provider_rejects_empty_endpoint_hash() {
    let (env, client, admin, operator) = setup_initialized(5_000);

    let result = client.try_register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider"),
        &String::from_str(&env, "model"),
        &String::from_str(&env, ""),
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidInput)));
}

#[test]
fn test_submit_analysis_request_rejects_empty_input_hash() {
    let (env, client, admin, operator) = setup_initialized(5_000);
    client.register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider"),
        &String::from_str(&env, "model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    let result = client.try_submit_analysis_request(
        &requester,
        &1,
        &patient,
        &0u64,
        &String::from_str(&env, ""),
        &String::from_str(&env, "retina_triage"),
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidInput)));
}

#[test]
fn test_submit_analysis_request_rejects_empty_task_type() {
    let (env, client, admin, operator) = setup_initialized(5_000);
    client.register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider"),
        &String::from_str(&env, "model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    let result = client.try_submit_analysis_request(
        &requester,
        &1,
        &patient,
        &0u64,
        &String::from_str(&env, "sha256:scan"),
        &String::from_str(&env, ""),
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidInput)));
}

#[test]
fn test_submit_analysis_request_rejects_zero_provider_id_as_not_found() {
    let (env, client, _admin, _operator) = setup_initialized(5_000);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    let result = client.try_submit_analysis_request(
        &requester,
        &0,
        &patient,
        &0u64,
        &String::from_str(&env, "sha256:scan"),
        &String::from_str(&env, "retina_triage"),
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::ProviderNotFound)));
}

#[test]
fn test_submit_analysis_request_rejects_inactive_provider() {
    let (env, client, admin, operator) = setup_initialized(5_000);
    client.register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider"),
        &String::from_str(&env, "model"),
        &String::from_str(&env, "endpoint-hash"),
    );
    client.set_provider_status(&admin, &1, &ProviderStatus::Paused);

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    let result = client.try_submit_analysis_request(
        &requester,
        &1,
        &patient,
        &123u64,
        &String::from_str(&env, "sha256:scan"),
        &String::from_str(&env, "retina_triage"),
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::ProviderInactive)));
}

#[test]
fn test_store_analysis_result_rejects_empty_output_hash() {
    let (env, client, _admin, operator) = setup_initialized(5_000);

    // Output hash is validated before the request lookup/state transitions.
    let result = client.try_store_analysis_result(
        &operator,
        &0u64,
        &String::from_str(&env, ""),
        &0u32,
        &0u32,
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidInput)));
}

#[test]
fn test_store_analysis_result_rejects_zero_request_id() {
    let (env, client, _admin, operator) = setup_initialized(5_000);

    let result = client.try_store_analysis_result(
        &operator,
        &0u64,
        &String::from_str(&env, "sha256:result"),
        &0u32,
        &0u32,
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::RequestNotFound)));
}

#[test]
fn test_store_analysis_result_accepts_zero_confidence_and_anomaly_scores() {
    let (env, client, admin, operator) = setup_initialized(6_000); // threshold > 0

    client.register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider"),
        &String::from_str(&env, "model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &0u64, // zero record id should be accepted
        &String::from_str(&env, "sha256:scan"),
        &String::from_str(&env, "retina_triage"),
    );

    let status = client.store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:result"),
        &0u32, // zero confidence
        &0u32, // zero anomaly score
    );

    assert_eq!(status, RequestStatus::Completed);

    let request = client.get_analysis_request(&request_id);
    assert_eq!(request.status, RequestStatus::Completed);

    let flagged = client.get_flagged_requests();
    assert_eq!(flagged.len(), 0);
}

#[test]
fn test_verify_analysis_result_rejects_empty_verification_hash() {
    let (env, client, admin, operator) = setup_initialized(5_000);

    client.register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider"),
        &String::from_str(&env, "model"),
        &String::from_str(&env, "endpoint-hash"),
    );

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

    client.store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:result"),
        &1_000u32,
        &0u32,
    );

    let result =
        client.try_verify_analysis_result(&admin, &request_id, &true, &String::from_str(&env, ""));

    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidInput)));
}

#[test]
fn test_verify_analysis_result_rejects_pending_request_state() {
    let (env, client, admin, _operator) = setup_initialized(5_000);

    client.register_provider(
        &admin,
        &1,
        &_operator,
        &String::from_str(&env, "Provider"),
        &String::from_str(&env, "model"),
        &String::from_str(&env, "endpoint-hash"),
    );

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

    let result = client.try_verify_analysis_result(
        &admin,
        &request_id,
        &true,
        &String::from_str(&env, "sha256:verification"),
    );

    assert_eq!(result, Err(Ok(AiIntegrationError::InvalidState)));
}

#[test]
fn test_set_provider_status_rejects_zero_provider_id_as_not_found() {
    let (env, client, admin, _operator) = setup_initialized(5_000);
    let _ = env; // keep env referenced if clippy complains about unused

    let result = client.try_set_provider_status(&admin, &0u32, &ProviderStatus::Paused);

    assert_eq!(result, Err(Ok(AiIntegrationError::ProviderNotFound)));
}

#[test]
fn test_set_anomaly_threshold_accepts_zero_bps() {
    let (_env, client, admin, _operator) = setup_initialized(5_000);

    let result = client.try_set_anomaly_threshold(&admin, &0u32);
    assert_eq!(result, Ok(Ok(())));
}
