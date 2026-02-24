extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::{
    AiIntegrationContract, AiIntegrationContractClient, ProviderStatus, RequestStatus,
    VerificationState,
};

fn setup() -> (Env, AiIntegrationContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);

    client.initialize(&admin, &7_000);

    (env, client, admin, operator)
}

#[test]
fn test_provider_registration_and_status_management() {
    let (env, client, admin, operator) = setup();

    client.register_provider(
        &admin,
        &1,
        &operator,
        &String::from_str(&env, "Provider A"),
        &String::from_str(&env, "retina-v1"),
        &String::from_str(&env, "sha256:endpoint"),
    );

    let provider = client.get_provider(&1);
    assert_eq!(provider.provider_id, 1);
    assert_eq!(provider.status, ProviderStatus::Active);

    client.set_provider_status(&admin, &1, &ProviderStatus::Paused);
    let paused = client.get_provider(&1);
    assert_eq!(paused.status, ProviderStatus::Paused);
}

#[test]
fn test_request_result_and_verification_flow() {
    let (env, client, admin, operator) = setup();
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    client.register_provider(
        &admin,
        &42,
        &operator,
        &String::from_str(&env, "Provider B"),
        &String::from_str(&env, "fundus-v3"),
        &String::from_str(&env, "sha256:provider-b"),
    );

    let request_id = client.submit_analysis_request(
        &requester,
        &42,
        &patient,
        &9001,
        &String::from_str(&env, "sha256:image-1"),
        &String::from_str(&env, "retina_screening"),
    );

    let status = client.store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:result-1"),
        &8_800,
        &3_500,
    );
    assert_eq!(status, RequestStatus::Completed);

    client.verify_analysis_result(
        &admin,
        &request_id,
        &true,
        &String::from_str(&env, "sha256:qa-1"),
    );

    let result = client.get_analysis_result(&request_id);
    assert_eq!(result.verification_state, VerificationState::Verified);
    assert!(result.verified_by.is_some());

    let request = client.get_analysis_request(&request_id);
    assert_eq!(request.status, RequestStatus::Completed);
}

#[test]
fn test_anomaly_detection_flags_request() {
    let (env, client, admin, operator) = setup();
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    client.register_provider(
        &admin,
        &7,
        &operator,
        &String::from_str(&env, "Provider C"),
        &String::from_str(&env, "oct-v2"),
        &String::from_str(&env, "sha256:provider-c"),
    );

    let request_id = client.submit_analysis_request(
        &requester,
        &7,
        &patient,
        &100,
        &String::from_str(&env, "sha256:image-2"),
        &String::from_str(&env, "macula_scan"),
    );

    let status = client.store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:result-2"),
        &6_200,
        &9_300,
    );
    assert_eq!(status, RequestStatus::Flagged);

    let flagged = client.get_flagged_requests();
    assert_eq!(flagged.len(), 1);
    assert_eq!(flagged.get(0).unwrap(), request_id);

    client.verify_analysis_result(
        &admin,
        &request_id,
        &false,
        &String::from_str(&env, "sha256:qa-reject"),
    );

    let result = client.get_analysis_result(&request_id);
    assert_eq!(result.verification_state, VerificationState::Rejected);

    let request = client.get_analysis_request(&request_id);
    assert_eq!(request.status, RequestStatus::Rejected);
}

#[test]
fn test_register_provider_requires_admin() {
    let (env, client, _admin, operator) = setup();
    let unauthorized = Address::generate(&env);

    let result = client.try_register_provider(
        &unauthorized,
        &3,
        &operator,
        &String::from_str(&env, "Bad Actor"),
        &String::from_str(&env, "model-x"),
        &String::from_str(&env, "sha256:bad"),
    );

    assert!(result.is_err());
}
