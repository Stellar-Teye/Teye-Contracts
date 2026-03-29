#![allow(clippy::unwrap_used, clippy::expect_used)]

use ai_integration::{AiIntegrationContract, AiIntegrationContractClient, AiIntegrationError};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup() -> (Env, AiIntegrationContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &5000); // 50% threshold

    (env, client, admin)
}

fn register_provider(
    env: &Env,
    client: &AiIntegrationContractClient,
    admin: &Address,
    provider_id: u32,
    operator: &Address,
) {
    client.register_provider(
        admin,
        &provider_id,
        operator,
        &String::from_str(env, "Test Provider"),
        &String::from_str(env, "test-model"),
        &String::from_str(env, "endpoint-hash"),
    );
}

fn submit_request(
    env: &Env,
    client: &AiIntegrationContractClient,
    requester: &Address,
    provider_id: u32,
) -> (u64, Address) {
    let patient = Address::generate(env);
    let request_id = client.submit_analysis_request(
        requester,
        &provider_id,
        &patient,
        &123,
        &String::from_str(env, "input-hash"),
        &String::from_str(env, "diagnosis"),
    );
    (request_id, patient)
}

#[test]
fn test_unauthorized_set_anomaly_threshold() {
    let (env, client, _admin) = setup();

    let random_user = Address::generate(&env);

    let result = client.try_set_anomaly_threshold(&random_user, &6000);
    assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));
}

#[test]
fn test_unauthorized_register_provider() {
    let (env, client, _admin) = setup();

    let random_user = Address::generate(&env);

    let result = client.try_register_provider(
        &random_user,
        &1,
        &Address::generate(&env),
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );
    assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));
}

#[test]
fn test_unauthorized_set_provider_status() {
    let (env, client, admin) = setup();

    let provider = Address::generate(&env);
    client.register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    let random_user = Address::generate(&env);

    let result =
        client.try_set_provider_status(&random_user, &1, &ai_integration::ProviderStatus::Paused);
    assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));
}

#[test]
fn test_unauthorized_verify_analysis_result() {
    let (env, client, admin) = setup();

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    client.register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &123,
        &String::from_str(&env, "input-hash"),
        &String::from_str(&env, "diagnosis"),
    );

    client.store_analysis_result(
        &provider,
        &request_id,
        &String::from_str(&env, "output-hash"),
        &9500,
        &100,
    );

    let random_user = Address::generate(&env);

    let result = client.try_verify_analysis_result(
        &random_user,
        &request_id,
        &true,
        &String::from_str(&env, "new-verification-hash"),
    );
    assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));
}

#[test]
fn test_unauthorized_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &5000);

    let random_user = Address::generate(&env);
    let result = client.try_initialize(&random_user, &6000);
    assert_eq!(result, Err(Ok(AiIntegrationError::AlreadyInitialized)));
}

#[test]
fn test_authorized_admin_functions_succeed() {
    let (env, client, admin) = setup();

    let result = client.try_set_anomaly_threshold(&admin, &6000);
    assert_eq!(result, Ok(Ok(())));

    let provider = Address::generate(&env);
    let result = client.try_register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );
    assert_eq!(result, Ok(Ok(())));

    let result =
        client.try_set_provider_status(&admin, &1, &ai_integration::ProviderStatus::Paused);
    assert_eq!(result, Ok(Ok(())));
}

#[test]
fn test_multiple_unauthorized_users() {
    let (env, client, _admin) = setup();

    let unauthorized_users = vec![
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];

    for user in unauthorized_users {
        let result = client.try_set_anomaly_threshold(&user, &6000);
        assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));

        let result = client.try_register_provider(
            &user,
            &1,
            &Address::generate(&env),
            &String::from_str(&env, "Test Provider"),
            &String::from_str(&env, "test-model"),
            &String::from_str(&env, "endpoint-hash"),
        );
        assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));

        let result =
            client.try_set_provider_status(&user, &1, &ai_integration::ProviderStatus::Paused);
        assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));
    }
}

#[test]
fn test_unauthorized_get_admin() {
    let (_env, client, _admin) = setup();

    let admin_result = client.try_get_admin();
    assert_eq!(admin_result.is_ok(), true);
}

#[test]
fn test_uninitialized_contract_rejects_admin_calls() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    let result = client.try_set_anomaly_threshold(&admin, &6000);
    assert_eq!(result, Err(Ok(AiIntegrationError::NotInitialized)));

    let result = client.try_register_provider(
        &admin,
        &1,
        &Address::generate(&env),
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );
    assert_eq!(result, Err(Ok(AiIntegrationError::NotInitialized)));
}

#[test]
fn test_registered_operator_cannot_escalate_to_admin_methods() {
    let (env, client, admin) = setup();
    let operator = Address::generate(&env);

    register_provider(&env, &client, &admin, 1, &operator);

    let threshold = client.try_set_anomaly_threshold(&operator, &6000);
    assert_eq!(threshold, Err(Ok(AiIntegrationError::Unauthorized)));

    let register_again = client.try_register_provider(
        &operator,
        &2,
        &Address::generate(&env),
        &String::from_str(&env, "Shadow Provider"),
        &String::from_str(&env, "shadow-model"),
        &String::from_str(&env, "shadow-endpoint"),
    );
    assert_eq!(register_again, Err(Ok(AiIntegrationError::Unauthorized)));

    let pause =
        client.try_set_provider_status(&operator, &1, &ai_integration::ProviderStatus::Paused);
    assert_eq!(pause, Err(Ok(AiIntegrationError::Unauthorized)));
}

#[test]
fn test_request_participants_cannot_escalate_to_result_verifier() {
    let (env, client, admin) = setup();
    let operator = Address::generate(&env);
    let requester = Address::generate(&env);

    register_provider(&env, &client, &admin, 1, &operator);
    let (request_id, patient) = submit_request(&env, &client, &requester, 1);

    client.store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "output-hash"),
        &9500,
        &100,
    );

    let requester_attempt = client.try_verify_analysis_result(
        &requester,
        &request_id,
        &true,
        &String::from_str(&env, "requester-verification"),
    );
    assert_eq!(requester_attempt, Err(Ok(AiIntegrationError::Unauthorized)));

    let patient_attempt = client.try_verify_analysis_result(
        &patient,
        &request_id,
        &true,
        &String::from_str(&env, "patient-verification"),
    );
    assert_eq!(patient_attempt, Err(Ok(AiIntegrationError::Unauthorized)));
}

#[test]
fn test_admin_cannot_escalate_into_provider_operator_role() {
    let (env, client, admin) = setup();
    let operator = Address::generate(&env);
    let requester = Address::generate(&env);

    register_provider(&env, &client, &admin, 1, &operator);
    let (request_id, _patient) = submit_request(&env, &client, &requester, 1);

    let result = client.try_store_analysis_result(
        &admin,
        &request_id,
        &String::from_str(&env, "admin-output"),
        &9500,
        &100,
    );
    assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));
}

#[test]
fn test_other_provider_operator_cannot_store_result_for_foreign_request() {
    let (env, client, admin) = setup();
    let operator_one = Address::generate(&env);
    let operator_two = Address::generate(&env);
    let requester = Address::generate(&env);

    register_provider(&env, &client, &admin, 1, &operator_one);
    register_provider(&env, &client, &admin, 2, &operator_two);

    let (request_id, _patient) = submit_request(&env, &client, &requester, 1);

    let result = client.try_store_analysis_result(
        &operator_two,
        &request_id,
        &String::from_str(&env, "foreign-output"),
        &8800,
        &250,
    );
    assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));
}
