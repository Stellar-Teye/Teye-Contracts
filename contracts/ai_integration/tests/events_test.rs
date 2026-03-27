#![allow(clippy::unwrap_used)]

extern crate std;

use ai_integration::{
    AiIntegrationContract, AiIntegrationContractClient, AiProvider, AnalysisRequest, AnalysisResult,
    ProviderStatus, RequestStatus, VerificationState,
};
use soroban_sdk::xdr::{ContractEvent, ContractEventBody, ScVal};
use soroban_sdk::{
    symbol_short, testutils::Address as _, testutils::Events, Address, Env, IntoVal, String,
    TryFromVal, Val,
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

fn to_scvals(env: &Env, topics: soroban_sdk::Vec<Val>) -> std::vec::Vec<ScVal> {
    let mut out = std::vec::Vec::new();
    for topic in topics.iter() {
        out.push(ScVal::try_from_val(env, &topic).unwrap());
    }
    out
}

fn assert_event(
    env: &Env,
    event: &ContractEvent,
    expected_topics: soroban_sdk::Vec<Val>,
    expected_data: Val,
) {
    let ContractEventBody::V0(body) = &event.body;
    assert_eq!(body.topics.as_slice(), to_scvals(env, expected_topics).as_slice());
    assert_eq!(body.data, ScVal::try_from_val(env, &expected_data).unwrap());
}

fn assert_single_event(env: &Env, expected_topics: soroban_sdk::Vec<Val>, expected_data: Val) {
    let events = env.events().all();
    let raw_events = events.events();
    assert_eq!(raw_events.len(), 1);
    assert_event(env, raw_events.last().unwrap(), expected_topics, expected_data);
}

#[test]
fn test_emits_expected_events_for_standard_flow() {
    let (env, client, admin, operator) = setup();
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    assert_single_event(
        &env,
        (symbol_short!("AI_INIT"), admin.clone()).into_val(&env),
        7_000u32.into_val(&env),
    );

    client.register_provider(
        &admin,
        &7,
        &operator,
        &String::from_str(&env, "Provider Event"),
        &String::from_str(&env, "retina-v7"),
        &String::from_str(&env, "sha256:endpoint-v7"),
    );
    let expected_provider = AiProvider {
        provider_id: 7,
        operator: operator.clone(),
        name: String::from_str(&env, "Provider Event"),
        model: String::from_str(&env, "retina-v7"),
        endpoint_hash: String::from_str(&env, "sha256:endpoint-v7"),
        status: ProviderStatus::Active,
        registered_at: 0,
    };
    assert_single_event(
        &env,
        (symbol_short!("PRV_REG"), 7u32).into_val(&env),
        expected_provider.into_val(&env),
    );

    let request_id = client.submit_analysis_request(
        &requester,
        &7,
        &patient,
        &501,
        &String::from_str(&env, "sha256:input-501"),
        &String::from_str(&env, "retina_triage"),
    );
    let expected_request = AnalysisRequest {
        request_id,
        provider_id: 7,
        requester: requester.clone(),
        patient: patient.clone(),
        record_id: 501,
        input_hash: String::from_str(&env, "sha256:input-501"),
        task_type: String::from_str(&env, "retina_triage"),
        requested_at: 0,
        status: RequestStatus::Pending,
    };
    assert_single_event(
        &env,
        (symbol_short!("REQ_SUB"), request_id, 7u32).into_val(&env),
        expected_request.into_val(&env),
    );

    let status = client.store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:output-501"),
        &8_900,
        &4_100,
    );
    assert_eq!(status, RequestStatus::Completed);
    let expected_stored_result = AnalysisResult {
        request_id,
        provider_id: 7,
        output_hash: String::from_str(&env, "sha256:output-501"),
        confidence_bps: 8_900,
        anomaly_score_bps: 4_100,
        completed_at: 0,
        verification_state: VerificationState::Unverified,
        verification_hash: None,
        verified_at: None,
        verified_by: None,
    };
    assert_single_event(
        &env,
        (symbol_short!("RES_STO"), request_id).into_val(&env),
        expected_stored_result.into_val(&env),
    );

    client.verify_analysis_result(
        &admin,
        &request_id,
        &true,
        &String::from_str(&env, "sha256:verify-501"),
    );
    let expected_verified_result = AnalysisResult {
        request_id,
        provider_id: 7,
        output_hash: String::from_str(&env, "sha256:output-501"),
        confidence_bps: 8_900,
        anomaly_score_bps: 4_100,
        completed_at: 0,
        verification_state: VerificationState::Verified,
        verification_hash: Some(String::from_str(&env, "sha256:verify-501")),
        verified_at: Some(0),
        verified_by: Some(admin),
    };
    assert_single_event(
        &env,
        (symbol_short!("RES_VFY"), request_id, true).into_val(&env),
        expected_verified_result.into_val(&env),
    );
}

#[test]
fn test_emits_flagged_and_rejected_result_events() {
    let (env, client, admin, operator) = setup();
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    client.register_provider(
        &admin,
        &11,
        &operator,
        &String::from_str(&env, "Provider Event B"),
        &String::from_str(&env, "retina-v8"),
        &String::from_str(&env, "sha256:endpoint-v8"),
    );

    let request_id = client.submit_analysis_request(
        &requester,
        &11,
        &patient,
        &777,
        &String::from_str(&env, "sha256:input-777"),
        &String::from_str(&env, "macula_scan"),
    );

    let status = client.store_analysis_result(
        &operator,
        &request_id,
        &String::from_str(&env, "sha256:output-777"),
        &7_200,
        &9_900,
    );
    assert_eq!(status, RequestStatus::Flagged);
    let expected_stored_result = AnalysisResult {
        request_id,
        provider_id: 11,
        output_hash: String::from_str(&env, "sha256:output-777"),
        confidence_bps: 7_200,
        anomaly_score_bps: 9_900,
        completed_at: 0,
        verification_state: VerificationState::Unverified,
        verification_hash: None,
        verified_at: None,
        verified_by: None,
    };
    assert_single_event(
        &env,
        (symbol_short!("RES_STO"), request_id).into_val(&env),
        expected_stored_result.into_val(&env),
    );

    client.verify_analysis_result(
        &admin,
        &request_id,
        &false,
        &String::from_str(&env, "sha256:verify-777"),
    );
    let expected_verified_result = AnalysisResult {
        request_id,
        provider_id: 11,
        output_hash: String::from_str(&env, "sha256:output-777"),
        confidence_bps: 7_200,
        anomaly_score_bps: 9_900,
        completed_at: 0,
        verification_state: VerificationState::Rejected,
        verification_hash: Some(String::from_str(&env, "sha256:verify-777")),
        verified_at: Some(0),
        verified_by: Some(admin),
    };
    assert_single_event(
        &env,
        (symbol_short!("RES_VFY"), request_id, false).into_val(&env),
        expected_verified_result.into_val(&env),
    );
}
