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

#[test]
fn test_unauthorized_set_anomaly_threshold() {
    let (env, client, _admin) = setup();

    let random_user = Address::generate(&env);

    // Try to set threshold as unauthorized user
    let result = client.try_set_anomaly_threshold(&random_user, &6000);
    assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));
}

#[test]
fn test_unauthorized_register_provider() {
    let (env, client, _admin) = setup();

    let random_user = Address::generate(&env);

    // Try to register provider as unauthorized user
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

    // First register a provider as admin
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

    // Try to update provider status as unauthorized user
    let result =
        client.try_set_provider_status(&random_user, &1, &ai_integration::ProviderStatus::Paused);
    assert_eq!(result, Err(Ok(AiIntegrationError::Unauthorized)));
}

#[test]
fn test_unauthorized_verify_analysis_result() {
    let (env, client, admin) = setup();

    // First create a request and result as admin
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
        &9500, // 95% confidence
        &100,  // 1% anomaly score
    );

    let random_user = Address::generate(&env);

    // Try to verify result as unauthorized user
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

    // First initialize with a legitimate admin
    let admin = Address::generate(&env);
    client.initialize(&admin, &5000);

    // Try to initialize again with a different user
    let random_user = Address::generate(&env);
    let result = client.try_initialize(&random_user, &6000);
    assert_eq!(result, Err(Ok(AiIntegrationError::AlreadyInitialized)));
}

#[test]
fn test_authorized_admin_functions_succeed() {
    let (env, client, admin) = setup();

    // Verify admin can successfully call admin functions
    let result = client.try_set_anomaly_threshold(&admin, &6000);
    assert_eq!(result, Ok(Ok(())));

    // Register a provider
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

    // Update provider status
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
        // All unauthorized users should fail on admin functions
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

    // get_admin should work for anyone (it's a public read function)
    // This test ensures it doesn't have authorization requirements
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

    // Even admin calls should fail on uninitialized contract
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
