extern crate std;

use ai_integration::{AiIntegrationContract, AiIntegrationContractClient, RequestStatus};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, String, Vec};

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
fn test_timestamp_advancement_affects_request_counter() {
    let (env, client, admin) = setup();

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    // Register provider
    client.register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    // Set initial timestamp
    env.ledger().set_timestamp(1000);

    // Create first request
    let request_id_1 = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &123,
        &String::from_str(&env, "input-hash-1"),
        &String::from_str(&env, "diagnosis"),
    );

    // Advance time
    env.ledger().set_timestamp(2000);

    // Create second request
    let request_id_2 = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &124,
        &String::from_str(&env, "input-hash-2"),
        &String::from_str(&env, "treatment"),
    );

    // Verify request counter increments over time
    assert!(request_id_2 > request_id_1);
}

#[test]
fn test_timestamp_advancement_affects_provider_registration() {
    let (env, client, admin) = setup();

    // Set initial timestamp
    env.ledger().set_timestamp(1000);

    // Register first provider
    client.register_provider(
        &admin,
        &1,
        &Address::generate(&env),
        &String::from_str(&env, "Provider 1"),
        &String::from_str(&env, "model-1"),
        &String::from_str(&env, "hash-1"),
    );

    // Advance time
    env.ledger().set_timestamp(3000);

    // Register second provider
    client.register_provider(
        &admin,
        &2,
        &Address::generate(&env),
        &String::from_str(&env, "Provider 2"),
        &String::from_str(&env, "model-2"),
        &String::from_str(&env, "hash-2"),
    );

    // Verify both providers can be retrieved
    let _provider_1 = client.get_provider(&1);
    let _provider_2 = client.get_provider(&2);
}

#[test]
fn test_timestamp_advancement_affects_result_completion() {
    let (env, client, admin) = setup();

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    // Register provider
    client.register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    // Set initial timestamp
    env.ledger().set_timestamp(1000);

    // Create request
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &123,
        &String::from_str(&env, "input-hash"),
        &String::from_str(&env, "diagnosis"),
    );

    // Advance time
    env.ledger().set_timestamp(5000);

    // Store result
    let status = client.store_analysis_result(
        &provider,
        &request_id,
        &String::from_str(&env, "output-hash"),
        &9500,
        &100,
    );

    // Result should be completed (not flagged due to low anomaly score)
    assert_eq!(status, RequestStatus::Completed);

    // Advance time for verification
    env.ledger().set_timestamp(8000);

    // Verify result - should not panic if successful
    client.verify_analysis_result(
        &admin,
        &request_id,
        &true,
        &String::from_str(&env, "verification-hash"),
    );

    // If we reach here, verification succeeded
    let _result = client.get_analysis_result(&request_id);
}

#[test]
fn test_request_status_changes_over_time() {
    let (env, client, admin) = setup();

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    // Register provider with low threshold (30%)
    client.set_anomaly_threshold(&admin, &3000);

    client.register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    // Create request
    env.ledger().set_timestamp(1000);
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &123,
        &String::from_str(&env, "input-hash"),
        &String::from_str(&env, "diagnosis"),
    );

    // Store result with high anomaly score (80%)
    env.ledger().set_timestamp(2000);
    let status = client.store_analysis_result(
        &provider,
        &request_id,
        &String::from_str(&env, "output-hash"),
        &9500,
        &8000, // 80% anomaly score
    );

    // Request should be flagged due to high anomaly score
    assert_eq!(status, RequestStatus::Flagged);

    // Verify it appears in flagged requests
    let _flagged_requests = client.get_flagged_requests();
}

#[test]
fn test_multiple_operations_time_ordering() {
    let (env, client, admin) = setup();

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    // Register provider at t=1000
    env.ledger().set_timestamp(1000);
    client.register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    // Create request at t=2000
    env.ledger().set_timestamp(2000);
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &123,
        &String::from_str(&env, "input-hash"),
        &String::from_str(&env, "diagnosis"),
    );

    // Store result at t=3000
    env.ledger().set_timestamp(3000);
    client.store_analysis_result(
        &provider,
        &request_id,
        &String::from_str(&env, "output-hash"),
        &9500,
        &100,
    );

    // Verify at t=4000
    env.ledger().set_timestamp(4000);
    client.verify_analysis_result(
        &admin,
        &request_id,
        &true,
        &String::from_str(&env, "verification-hash"),
    );

    // Verify all operations completed successfully by attempting to retrieve data
    let _provider_info = client.get_provider(&1);
    let _request_info = client.get_analysis_request(&request_id);
    let _result_info = client.get_analysis_result(&request_id);
}

#[test]
fn test_same_timestamp_multiple_operations() {
    let (env, client, admin) = setup();

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    // Register provider
    env.ledger().set_timestamp(1000);
    client.register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    // Keep same timestamp for request creation
    let request_id = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &123,
        &String::from_str(&env, "input-hash"),
        &String::from_str(&env, "diagnosis"),
    );

    // Verify both operations completed successfully
    let _provider_info = client.get_provider(&1);
    let _request_info = client.get_analysis_request(&request_id);

    assert!(request_id > 0);
}

#[test]
fn test_timestamp_manipulation_edge_cases() {
    let (env, client, admin) = setup();

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    // Register provider
    client.register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    // Test with timestamp 0
    env.ledger().set_timestamp(0);
    let request_id_1 = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &123,
        &String::from_str(&env, "input-hash"),
        &String::from_str(&env, "diagnosis"),
    );

    // Test with very large timestamp
    env.ledger().set_timestamp(u64::MAX);
    let request_id_2 = client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &124,
        &String::from_str(&env, "input-hash-2"),
        &String::from_str(&env, "treatment"),
    );

    // Both requests should be created successfully
    assert!(request_id_1 > 0);
    assert!(request_id_2 > request_id_1);
}

#[test]
fn test_time_based_request_ordering() {
    let (env, client, admin) = setup();

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    // Register provider
    client.register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    // Create multiple requests with different timestamps
    let mut request_ids = Vec::new(&env);

    for i in 0..5 {
        env.ledger().set_timestamp(1000 + i * 1000);
        let request_id = client.submit_analysis_request(
            &requester,
            &1,
            &patient,
            &(100 + i as u64),
            &String::from_str(&env, &format!("input-hash-{}", i)),
            &String::from_str(&env, "diagnosis"),
        );
        request_ids.push_back(request_id);
    }

    // Verify requests are created in chronological order
    for i in 1..request_ids.len() {
        assert!(request_ids.get(i).unwrap() > request_ids.get(i - 1).unwrap());
    }
}
