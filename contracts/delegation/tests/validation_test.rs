#![allow(clippy::unwrap_used, clippy::expect_used)]

use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env};
use teye_delegation::{
    task_queue::TaskStatus, DelegationContract, DelegationContractClient,
};

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
fn test_submit_task_accepts_zero_priority_deadline_and_input_hash() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let input = zero_hash(&env);

    let task_id = client.submit_task(&creator, &input, &0, &0);
    let task = client.get_task(&task_id).expect("task should be stored");

    assert_eq!(task.id, 1);
    assert_eq!(task.priority, 0);
    assert_eq!(task.deadline, 0);
    assert_eq!(task.input_data, input);
    assert_eq!(task.status, TaskStatus::Pending);
}

#[test]
fn test_assign_task_zero_id_reverts_and_preserves_real_task_state() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    let input = zero_hash(&env);

    let task_id = client.submit_task(&creator, &input, &0, &0);
    client.register_executor(&executor);

    assert!(client.try_assign_task(&executor, &0).is_err());

    let task = client.get_task(&task_id).expect("task should still exist");
    assert_eq!(task.status, TaskStatus::Pending);
    assert_eq!(task.executor, None);
}

#[test]
fn test_submit_result_zero_hashes_with_matching_proof_completes_task() {
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

    let task = client.get_task(&task_id).expect("task should exist");
    let executor_info = client
        .get_executor_info(&executor)
        .expect("executor should exist");

    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(task.result, Some(result));
    assert_eq!(executor_info.tasks_completed, 1);
    assert_eq!(executor_info.reputation, 101);
}

#[test]
fn test_submit_result_zero_id_reverts_without_mutating_executor() {
    let (env, client) = setup();
    let executor = Address::generate(&env);
    client.register_executor(&executor);

    let info_before = client
        .get_executor_info(&executor)
        .expect("executor should exist");
    assert!(client
        .try_submit_result(&executor, &0, &zero_hash(&env), &zero_hash(&env))
        .is_err());

    let info_after = client
        .get_executor_info(&executor)
        .expect("executor should still exist");
    assert_eq!(info_after.tasks_completed, info_before.tasks_completed);
    assert_eq!(info_after.reputation, info_before.reputation);
}

#[test]
fn test_submit_result_zero_proof_marks_task_failed_and_slashes_executor() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    let input = zero_hash(&env);
    let result = BytesN::from_array(&env, &[1; 32]);
    let invalid_zero_proof = zero_hash(&env);

    let task_id = client.submit_task(&creator, &input, &0, &0);
    client.register_executor(&executor);
    client.assign_task(&executor, &task_id);
    client.submit_result(&executor, &task_id, &result, &invalid_zero_proof);

    let task = client.get_task(&task_id).expect("task should exist");
    let executor_info = client
        .get_executor_info(&executor)
        .expect("executor should exist");

    assert_eq!(task.status, TaskStatus::Failed);
    assert_eq!(task.result, None);
    assert_eq!(executor_info.reputation, 90);
    assert_eq!(executor_info.tasks_completed, 0);
}
