extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use ai_integration::{AiIntegrationContract, AiIntegrationContractClient, RequestStatus};

#[test]
fn integration_ai_result_is_flagged_above_threshold() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AiIntegrationContract, ());
    let client = AiIntegrationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    client.initialize(&admin, &6_000);
    client.register_provider(
        &admin,
        &12,
        &operator,
        &String::from_str(&env, "Provider Int"),
        &String::from_str(&env, "model-int"),
        &String::from_str(&env, "sha256:int"),
    );

    let request_id = client.submit_analysis_request(
        &requester,
        &12,
        &patient,
        &777,
        &String::from_str(&env, "sha256:scan-int"),
        &String::from_str(&env, "retina_triage"),
    );

    let status = client.store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:out-int"),
        &7_000,
        &8_100,
    );

    assert_eq!(status, RequestStatus::Flagged);
}
