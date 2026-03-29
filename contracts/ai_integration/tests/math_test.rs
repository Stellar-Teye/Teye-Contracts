#![allow(clippy::unwrap_used, clippy::expect_used)]

extern crate std;

use ai_integration::{
    AiIntegrationContract, AiIntegrationContractClient, AiIntegrationError, RequestStatus,
};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, String};

fn setup() -> (
    Env,
    AiIntegrationContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    client.initialize(&admin, &7_000);
    client.register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider Math"),
        &String::from_str(&env, "model-math"),
        &String::from_str(&env, "sha256:math"),
    );

    (env, client, admin, operator, requester, patient)
}

#[test]
fn test_initialize_rejects_u32_max_threshold() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    assert_eq!(
        client.try_initialize(&admin, &u32::MAX),
        Err(Ok(AiIntegrationError::InvalidInput))
    );
    assert!(!client.is_initialized());
}

#[test]
fn test_set_anomaly_threshold_rejects_u32_max_without_mutating_state() {
    let (_env, client, admin, _operator, _requester, _patient) = setup();

    assert_eq!(
        client.try_set_anomaly_threshold(&admin, &u32::MAX),
        Err(Ok(AiIntegrationError::InvalidInput))
    );
    assert_eq!(client.get_anomaly_threshold(), 7_000);
}

#[test]
fn test_store_analysis_result_rejects_out_of_range_bps_values() {
    let (env, client, _admin, operator, requester, patient) = setup();

    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &u64::MAX,
        &String::from_str(&env, "sha256:scan-max"),
        &String::from_str(&env, "retina_triage"),
    );

    assert_eq!(
        client.try_store_analysis_result(
            &operator,
            &request_id,
            &String::from_str(&env, "sha256:result-max"),
            &u32::MAX,
            &0,
        ),
        Err(Ok(AiIntegrationError::InvalidInput))
    );

    assert_eq!(
        client.try_store_analysis_result(
            &operator,
            &request_id,
            &String::from_str(&env, "sha256:result-max-2"),
            &10_000,
            &u32::MAX,
        ),
        Err(Ok(AiIntegrationError::InvalidInput))
    );

    assert_eq!(
        client.get_analysis_request(&request_id).status,
        RequestStatus::Pending
    );
}

#[test]
fn test_submit_analysis_request_preserves_u64_max_record_id() {
    let (env, client, _admin, _operator, requester, patient) = setup();

    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &u64::MAX,
        &String::from_str(&env, "sha256:scan-record-max"),
        &String::from_str(&env, "retina_triage"),
    );

    let request = client.get_analysis_request(&request_id);
    assert_eq!(request_id, 1);
    assert_eq!(request.record_id, u64::MAX);
}

#[test]
fn test_request_counter_saturates_at_u64_max_instead_of_wrapping() {
    let (env, client, _admin, _operator, requester, patient) = setup();

    env.as_contract(&client.address, || {
        env.storage()
            .instance()
            .set(&symbol_short!("REQCTR"), &u64::MAX);
    });

    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &42,
        &String::from_str(&env, "sha256:scan-counter-max"),
        &String::from_str(&env, "retina_triage"),
    );

    assert_eq!(request_id, u64::MAX);
    assert_eq!(
        client.get_analysis_request(&request_id).request_id,
        u64::MAX
    );
}
