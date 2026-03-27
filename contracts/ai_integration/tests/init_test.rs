#![allow(clippy::unwrap_used, clippy::expect_used)]

extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use ai_integration::{
    AiIntegrationContract, AiIntegrationContractClient, AiIntegrationError, RequestStatus,
};

fn setup_uninitialized() -> (Env, AiIntegrationContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);

    (env, client)
}

fn setup_initialized(
    anomaly_threshold_bps: u32,
) -> (Env, AiIntegrationContractClient<'static>, Address) {
    let (env, client) = setup_uninitialized();
    let admin = Address::generate(&env);
    client.initialize(&admin, &anomaly_threshold_bps);
    (env, client, admin)
}

#[test]
fn test_uninitialized_functions_rejected() {
    let (env, client) = setup_uninitialized();
    assert!(!client.is_initialized());

    assert_eq!(
        client.try_get_admin(),
        Err(Ok(AiIntegrationError::NotInitialized))
    );
    assert_eq!(
        client.try_get_anomaly_threshold(),
        Err(Ok(AiIntegrationError::NotInitialized))
    );

    let caller = Address::generate(&env);
    assert_eq!(
        client.try_set_anomaly_threshold(&caller, &1_000),
        Err(Ok(AiIntegrationError::NotInitialized))
    );

    let operator = Address::generate(&env);
    assert_eq!(
        client.try_register_provider(
            &caller,
            &1,
            &operator,
            &String::from_str(&env, "P"),
            &String::from_str(&env, "m"),
            &String::from_str(&env, "h"),
        ),
        Err(Ok(AiIntegrationError::NotInitialized))
    );
}

#[test]
fn test_initialize_rejects_invalid_threshold_and_does_not_initialize() {
    let (env, client) = setup_uninitialized();
    let admin = Address::generate(&env);

    assert_eq!(
        client.try_initialize(&admin, &10_001),
        Err(Ok(AiIntegrationError::InvalidInput))
    );
    assert!(!client.is_initialized());
    assert_eq!(
        client.try_get_admin(),
        Err(Ok(AiIntegrationError::NotInitialized))
    );
    assert_eq!(
        client.try_get_anomaly_threshold(),
        Err(Ok(AiIntegrationError::NotInitialized))
    );
}

#[test]
fn test_initialize_sets_initial_state_constraints() {
    let (env, client, admin) = setup_initialized(7_000);
    assert!(client.is_initialized());
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_anomaly_threshold(), 7_000);

    // Prove the request counter starts from 0 by verifying the first request id is 1.
    let operator = Address::generate(&env);
    client.register_provider(
        &admin,
        &12,
        &operator,
        &String::from_str(&env, "Provider Init"),
        &String::from_str(&env, "model-init"),
        &String::from_str(&env, "sha256:init"),
    );

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let request_id = client.submit_analysis_request(
        &requester,
        &12,
        &patient,
        &777,
        &String::from_str(&env, "sha256:scan-init"),
        &String::from_str(&env, "retina_triage"),
    );

    assert_eq!(request_id, 1);

    // Sanity: below threshold is completed (not flagged).
    let status = client.store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:out-init"),
        &8_800,
        &6_999,
    );
    assert_eq!(status, RequestStatus::Completed);
}

#[test]
fn test_double_initialize_fails_and_state_is_not_overwritten() {
    let (env, client) = setup_uninitialized();
    let admin_1 = Address::generate(&env);
    let admin_2 = Address::generate(&env);

    client.initialize(&admin_1, &6_000);

    // Even if the second init passes an invalid threshold, it must still fail
    // as AlreadyInitialized (init cannot be bypassed by triggering other checks).
    assert_eq!(
        client.try_initialize(&admin_2, &10_001),
        Err(Ok(AiIntegrationError::AlreadyInitialized))
    );

    assert_eq!(client.get_admin(), admin_1);
    assert_eq!(client.get_anomaly_threshold(), 6_000);
}