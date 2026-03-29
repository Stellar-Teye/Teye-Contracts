use super::{DelegationContract, DelegationContractClient};
use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env};

fn setup() -> (Env, DelegationContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DelegationContract, ());
    let client = DelegationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client)
}

fn zero_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0; 32])
}

fn proof_for(env: &Env, input: &BytesN<32>, result: &BytesN<32>) -> BytesN<32> {
    let mut payload = [0u8; 64];
    payload[..32].copy_from_slice(&input.to_array());
    payload[32..].copy_from_slice(&result.to_array());
    let proof = env.crypto().sha256(&Bytes::from_slice(env, &payload));
    BytesN::from_array(env, &proof.to_array())
}

#[test]
fn test_submit_result_on_completed_task_fails() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    let input = zero_hash(&env);
    let result = zero_hash(&env);
    let proof = proof_for(&env, &input, &result);

    let task_id = client.submit_task(&creator, &input, &0, &0);
    client.register_executor(&executor);
    client.assign_task(&executor, &task_id);

    // Initial submission succeeds
    client.submit_result(&executor, &task_id, &result, &proof);

    // Second submission should fail because task is no longer in Assigned state (it's Completed)
    let result2 = client.try_submit_result(&executor, &task_id, &result, &proof);
    assert!(result2.is_err());
}

#[test]
fn test_executor_reputation_consistency() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    let input = zero_hash(&env);
    let result = zero_hash(&env);
    let proof = proof_for(&env, &input, &result);

    let task_id = client.submit_task(&creator, &input, &0, &0);
    client.register_executor(&executor);
    client.assign_task(&executor, &task_id);

    client.submit_result(&executor, &task_id, &result, &proof);

    let info = client.get_executor_info(&executor).unwrap();
    assert_eq!(info.reputation, 101);
    assert_eq!(info.tasks_completed, 1);
}
