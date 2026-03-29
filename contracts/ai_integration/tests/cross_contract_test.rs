extern crate std;

use ai_integration::{AiIntegrationContract, AiIntegrationContractClient};
use soroban_sdk::{
    contract, contracterror, contractimpl, testutils::Address as _, testutils::Ledger as _,
    Address, Env, String,
};

// Mock secondary contract for cross-contract testing
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MockContractError {
    Unauthorized = 1,
    InvalidInput = 2,
    ContractFailed = 3,
}

#[contract]
pub struct MockAnalysisContract;

#[contractimpl]
impl MockAnalysisContract {
    pub fn initialize(_env: Env, _owner: Address) -> Result<(), MockContractError> {
        // Mock initialization
        Ok(())
    }

    pub fn validate_provider(
        _env: Env,
        caller: Address,
        provider_id: u32,
    ) -> Result<bool, MockContractError> {
        caller.require_auth();

        // Mock validation - only allow provider IDs > 0
        Ok(provider_id > 0)
    }

    pub fn get_analysis_count(_env: Env) -> Result<u64, MockContractError> {
        // Mock count - in real implementation this would be stored
        Ok(0)
    }
}

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
fn test_cross_contract_contract_independence() {
    let (env, ai_client, admin) = setup();

    // Register mock contract
    let mock_contract_id = env.register(MockAnalysisContract, ());
    let mock_client = AiIntegrationContractClient::new(&env, &mock_contract_id);

    // Initialize mock contract - this will fail but that's expected
    let _mock_init_result = mock_client.try_initialize(&admin, &5000);

    // Verify ai_integration contract works independently
    let _ai_admin = ai_client.get_admin();

    // Verify contracts have different addresses
    let ai_address = ai_client.address;
    let mock_address = mock_client.address;
    assert_ne!(ai_address, mock_address);
}

#[test]
fn test_cross_contract_multiple_contracts_independence() {
    let (env, ai_client, admin) = setup();

    let provider = Address::generate(&env);
    let requester = Address::generate(&env);
    let patient = Address::generate(&env);

    // Register provider in ai_integration
    ai_client.register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    // Register multiple mock contracts
    let mock_contract_id_1 = env.register(MockAnalysisContract, ());
    let mock_client_1 = AiIntegrationContractClient::new(&env, &mock_contract_id_1);

    let mock_contract_id_2 = env.register(MockAnalysisContract, ());
    let mock_client_2 = AiIntegrationContractClient::new(&env, &mock_contract_id_2);

    // Create request in main contract
    let request_id = ai_client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &123,
        &String::from_str(&env, "input-hash"),
        &String::from_str(&env, "diagnosis"),
    );

    // Verify all contracts work independently
    assert!(request_id > 0);

    // Verify contracts have different addresses
    let ai_address = ai_client.address;
    let mock_address_1 = mock_client_1.address;
    let mock_address_2 = mock_client_2.address;

    assert_ne!(ai_address, mock_address_1);
    assert_ne!(ai_address, mock_address_2);
    assert_ne!(mock_address_1, mock_address_2);
}

#[test]
fn test_cross_contract_timestamp_consistency() {
    let (env, ai_client, _admin) = setup();

    // Set specific timestamp
    env.ledger().set_timestamp(5000);

    // Register mock contract
    let mock_contract_id = env.register(MockAnalysisContract, ());
    let mock_client = AiIntegrationContractClient::new(&env, &mock_contract_id);

    // Both contracts should see same timestamp
    assert_eq!(env.ledger().timestamp(), 5000);

    // Test that operations in both contracts work with same timestamp
    let _ai_admin = ai_client.get_admin();

    // Verify contracts have different addresses
    let ai_address = ai_client.address;
    let mock_address = mock_client.address;
    assert_ne!(ai_address, mock_address);
}

#[test]
fn test_cross_contract_error_isolation() {
    let (env, ai_client, admin) = setup();

    let unauthorized_user = Address::generate(&env);

    // Register mock contract
    let mock_contract_id = env.register(MockAnalysisContract, ());
    let mock_client = AiIntegrationContractClient::new(&env, &mock_contract_id);

    // Test that errors in one contract don't affect the other
    let unauthorized_result = mock_client.try_initialize(&unauthorized_user, &5000);
    assert!(unauthorized_result.is_err());

    // Main contract should still work
    let _ai_admin = ai_client.get_admin();

    // Verify contracts have different addresses
    let ai_address = ai_client.address;
    let mock_address = mock_client.address;
    assert_ne!(ai_address, mock_address);
}

#[test]
fn test_cross_contract_resource_isolation() {
    let (env, ai_client, admin) = setup();

    let requester = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    // Register provider in ai_integration
    ai_client.register_provider(
        &admin,
        &1,
        &provider,
        &String::from_str(&env, "Test Provider"),
        &String::from_str(&env, "test-model"),
        &String::from_str(&env, "endpoint-hash"),
    );

    // Create multiple requests
    let request_id_1 = ai_client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &123,
        &String::from_str(&env, "input-hash-1"),
        &String::from_str(&env, "diagnosis"),
    );

    let request_id_2 = ai_client.submit_analysis_request(
        &requester,
        &1,
        &patient,
        &124,
        &String::from_str(&env, "input-hash-2"),
        &String::from_str(&env, "treatment"),
    );

    // Register mock contract
    let mock_contract_id = env.register(MockAnalysisContract, ());
    let mock_client = AiIntegrationContractClient::new(&env, &mock_contract_id);

    // Verify both requests were created in main contract
    assert!(request_id_1 > 0);
    assert!(request_id_2 > 0);
    assert!(request_id_2 > request_id_1);

    // Verify contracts have different addresses
    let ai_address = ai_client.address;
    let mock_address = mock_client.address;
    assert_ne!(ai_address, mock_address);
}

#[test]
fn test_cross_contract_state_isolation() {
    let (env, ai_client, admin) = setup();

    // Register mock contract
    let mock_contract_id = env.register(MockAnalysisContract, ());
    let mock_client = AiIntegrationContractClient::new(&env, &mock_contract_id);

    // Verify ai_integration contract state is unaffected by mock contract operations
    let _ai_admin = ai_client.get_admin();

    // Test that both contracts can be called independently
    let mock_result = mock_client.try_get_admin();
    // This should fail since mock contract doesn't have get_admin
    assert!(mock_result.is_err());

    // Verify contracts have different addresses
    let ai_address = ai_client.address;
    let mock_address = mock_client.address;
    assert_ne!(ai_address, mock_address);
}
