#![cfg(test)]

use super::*;
use crate::settlement::{BatchRecordInput, RecordType};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, Vec,
};

#[soroban_sdk::contract]
pub struct MockVisionRecords;

#[soroban_sdk::contractimpl]
impl MockVisionRecords {
    pub fn add_records(
        _env: Env,
        _provider: Address,
        _records: Vec<BatchRecordInput>,
    ) -> Result<Vec<u64>, common::CommonError> {
        Ok(Vec::new(&_env))
    }
}

#[test]
fn test_initialize() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let vision_records = Address::generate(&env);

    let contract_id = env.register(StateChannelContract, ());
    let client = StateChannelContractClient::new(&env, &contract_id);

    client.initialize(&admin, &vision_records);

    // Check double initialization fails
    let res = client.try_initialize(&admin, &vision_records);
    assert!(res.is_err());
}

#[test]
fn test_cooperative_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let vision_records_id = env.register(MockVisionRecords, ());
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    let contract_id = env.register(StateChannelContract, ());
    let client = StateChannelContractClient::new(&env, &contract_id);

    client.initialize(&admin, &vision_records_id);

    // 1. Open
    let capacity = 1000;
    let channel_id = client.open_channel(&patient, &provider, &capacity);
    assert_eq!(channel_id, 1);

    // 2. Cooperative Close
    let balance = 400;
    let nonce = 5;
    let sig = BytesN::from_array(&env, &[0u8; 64]);

    client.cooperative_close(&channel_id, &balance, &nonce, &sig, &sig);

    // 3. Settle
    client.settle(&channel_id);
}

#[test]
fn test_dispute_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let vision_records_id = env.register(MockVisionRecords, ());
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    let contract_id = env.register(StateChannelContract, ());
    let client = StateChannelContractClient::new(&env, &contract_id);

    client.initialize(&admin, &vision_records_id);

    let channel_id = client.open_channel(&patient, &provider, &1000);

    // 1. Unilateral Close by patient
    client.unilateral_close(&channel_id, &patient);

    // 2. Submit Fraud Proof (e.g. provider showing a later state)
    let sig = BytesN::from_array(&env, &[0u8; 64]);
    client.submit_fraud_proof(&channel_id, &10, &600, &sig);

    // 3. Try settle too early
    let res = client.try_settle(&channel_id);
    assert!(res.is_err());

    // 4. Advance time
    env.ledger().set_timestamp(env.ledger().timestamp() + 86401);

    // 5. Settle
    client.settle(&channel_id);
}

#[test]
fn test_rebalance() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let vision_records_id = env.register(MockVisionRecords, ());
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    let contract_id = env.register(StateChannelContract, ());
    let client = StateChannelContractClient::new(&env, &contract_id);

    client.initialize(&admin, &vision_records_id);

    let channel_id = client.open_channel(&patient, &provider, &1000);

    let sig = BytesN::from_array(&env, &[0u8; 64]);
    client.rebalance(&channel_id, &2000, &sig, &sig);
}

#[test]
fn test_multi_hop() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let vision_records_id = env.register(MockVisionRecords, ());
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let intermediary = Address::generate(&env);

    let contract_id = env.register(StateChannelContract, ());
    let client = StateChannelContractClient::new(&env, &contract_id);

    client.initialize(&admin, &vision_records_id);

    let channel_id = client.open_multi_hop(&patient, &provider, &intermediary, &1000);
    assert_eq!(channel_id, 1);
}
