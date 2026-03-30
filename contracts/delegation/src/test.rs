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

// ── State consistency tests for simultaneous operations ─────────────────────

/// Test that multiple tasks maintain independent state
#[test]
fn test_multiple_tasks_state_isolation() {
    let (env, client) = setup();
    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);
    let executor = Address::generate(&env);

    client.register_executor(&executor);

    // Create two tasks with different inputs
    let input1 = BytesN::from_array(&env, &[1u8; 32]);
    let input2 = BytesN::from_array(&env, &[2u8; 32]);
    let result1 = BytesN::from_array(&env, &[10u8; 32]);
    let result2 = BytesN::from_array(&env, &[20u8; 32]);

    let task_id1 = client.submit_task(&creator1, &input1, &100, &0);
    let task_id2 = client.submit_task(&creator2, &input2, &200, &0);

    assert_ne!(task_id1, task_id2);

    // Assign both tasks to the same executor
    client.assign_task(&executor, &task_id1);
    client.assign_task(&executor, &task_id2);

    // Submit results for both tasks
    let proof1 = proof_for(&env, &input1, &result1);
    let proof2 = proof_for(&env, &input2, &result2);

    client.submit_result(&executor, &task_id1, &result1, &proof1);
    client.submit_result(&executor, &task_id2, &result2, &proof2);

    // Verify both tasks completed independently
    let info = client.get_executor_info(&executor).unwrap();
    assert_eq!(info.tasks_completed, 2);
    assert_eq!(info.reputation, 102);
}

/// Test that multiple executors don't interfere with each other's state
#[test]
fn test_multiple_executors_state_isolation() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor1 = Address::generate(&env);
    let executor2 = Address::generate(&env);

    client.register_executor(&executor1);
    client.register_executor(&executor2);

    let input = zero_hash(&env);
    let result = zero_hash(&env);
    let proof = proof_for(&env, &input, &result);

    // Create two tasks
    let task_id1 = client.submit_task(&creator, &input, &0, &0);
    let task_id2 = client.submit_task(&creator, &input, &0, &0);

    // Assign to different executors
    client.assign_task(&executor1, &task_id1);
    client.assign_task(&executor2, &task_id2);

    // Both complete their tasks
    client.submit_result(&executor1, &task_id1, &result, &proof);
    client.submit_result(&executor2, &task_id2, &result, &proof);

    // Verify each executor's state is independent
    let info1 = client.get_executor_info(&executor1).unwrap();
    let info2 = client.get_executor_info(&executor2).unwrap();

    assert_eq!(info1.tasks_completed, 1);
    assert_eq!(info1.reputation, 101);
    assert_eq!(info2.tasks_completed, 1);
    assert_eq!(info2.reputation, 101);
}

/// Test that concurrent task assignments maintain consistency
#[test]
fn test_concurrent_task_assignments_consistency() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor1 = Address::generate(&env);
    let executor2 = Address::generate(&env);
    let executor3 = Address::generate(&env);

    client.register_executor(&executor1);
    client.register_executor(&executor2);
    client.register_executor(&executor3);

    let input = zero_hash(&env);
    let result = zero_hash(&env);
    let proof = proof_for(&env, &input, &result);

    // Create multiple tasks
    let mut task_ids = Vec::new();
    for _ in 0..3 {
        let task_id = client.submit_task(&creator, &input, &0, &0);
        task_ids.push(task_id);
    }

    // Assign each task to a different executor (simulating concurrent assignments)
    client.assign_task(&executor1, &task_ids[0]);
    client.assign_task(&executor2, &task_ids[1]);
    client.assign_task(&executor3, &task_ids[2]);

    // All executors complete their tasks
    client.submit_result(&executor1, &task_ids[0], &result, &proof);
    client.submit_result(&executor2, &task_ids[1], &result, &proof);
    client.submit_result(&executor3, &task_ids[2], &result, &proof);

    // Verify all completed correctly
    let info1 = client.get_executor_info(&executor1).unwrap();
    let info2 = client.get_executor_info(&executor2).unwrap();
    let info3 = client.get_executor_info(&executor3).unwrap();

    assert_eq!(info1.tasks_completed, 1);
    assert_eq!(info2.tasks_completed, 1);
    assert_eq!(info3.tasks_completed, 1);
}

/// Test that task state transitions are atomic under multiple operations
#[test]
fn test_task_state_transitions_atomic() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor1 = Address::generate(&env);
    let executor2 = Address::generate(&env);

    client.register_executor(&executor1);
    client.register_executor(&executor2);

    let input = zero_hash(&env);
    let result = zero_hash(&env);
    let proof = proof_for(&env, &input, &result);

    // Create task
    let task_id = client.submit_task(&creator, &input, &0, &0);

    // Executor1 assigns the task
    client.assign_task(&executor1, &task_id);

    // Executor2 cannot assign an already-assigned task
    let result = client.try_assign_task(&executor2, &task_id);
    assert!(result.is_err());

    // Executor1 completes the task
    client.submit_result(&executor1, &task_id, &result, &proof);

    // Executor2 cannot submit result for a completed task
    let result2 = client.try_submit_result(&executor2, &task_id, &result, &proof);
    assert!(result2.is_err());
}

/// Test that executor reputation is consistently updated across operations
#[test]
fn test_executor_reputation_consistency_multiple_tasks() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);

    client.register_executor(&executor);

    let input = zero_hash(&env);
    let result = zero_hash(&env);
    let proof = proof_for(&env, &input, &result);

    // Submit and complete 5 tasks
    for _ in 0..5 {
        let task_id = client.submit_task(&creator, &input, &0, &0);
        client.assign_task(&executor, &task_id);
        client.submit_result(&executor, &task_id, &result, &proof);
    }

    let info = client.get_executor_info(&executor).unwrap();
    assert_eq!(info.tasks_completed, 5);
    assert_eq!(info.reputation, 105); // Initial 100 + 5 completions
}

/// Test that failed tasks don't affect executor's completion count
#[test]
fn test_failed_task_doesnt_increment_completion_count() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);

    client.register_executor(&executor);

    let input = zero_hash(&env);
    let result1 = zero_hash(&env);
    let bad_result = BytesN::from_array(&env, &[99u8; 32]);
    let proof_bad = proof_for(&env, &input, &bad_result);
    let proof_good = proof_for(&env, &input, &result1);

    // Submit two tasks
    let task_id1 = client.submit_task(&creator, &input, &0, &0);
    let task_id2 = client.submit_task(&creator, &input, &0, &0);

    client.register_executor(&executor);
    client.assign_task(&executor, &task_id1);
    client.assign_task(&executor, &task_id2);

    // Submit bad result for task1 (will fail verification)
    client.submit_result(&executor, &task_id1, &bad_result, &proof_bad);

    // Submit good result for task2
    client.submit_result(&executor, &task_id2, &result1, &proof_good);

    let info = client.get_executor_info(&executor).unwrap();
    // Only one task should have succeeded
    assert_eq!(info.tasks_completed, 1);
    // Bad result should trigger slash
    assert!(info.reputation < 101);
}

/// Test task priority doesn't affect state consistency
#[test]
fn test_task_priority_doesnt_affect_state() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);

    client.register_executor(&executor);

    let input = zero_hash(&env);
    let result = zero_hash(&env);
    let proof = proof_for(&env, &input, &result);

    // Create tasks with different priorities
    let task_high = client.submit_task(&creator, &input, &1000, &0);
    let task_low = client.submit_task(&creator, &input, &10, &0);

    // Assign in reverse priority order
    client.assign_task(&executor, &task_low);
    client.assign_task(&executor, &task_high);

    // Complete in same order
    client.submit_result(&executor, &task_low, &result, &proof);
    client.submit_result(&executor, &task_high, &result, &proof);

    let info = client.get_executor_info(&executor).unwrap();
    assert_eq!(info.tasks_completed, 2);
}

/// Test that deadline doesn't affect task state consistency
#[test]
fn test_task_deadline_doesnt_affect_state() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);

    client.register_executor(&executor);

    let input = zero_hash(&env);
    let result = zero_hash(&env);
    let proof = proof_for(&env, &input, &result);

    // Create tasks with different deadlines
    let task_with_deadline = client.submit_task(&creator, &input, &0, &1000000);
    let task_no_deadline = client.submit_task(&creator, &input, &0, &0);

    client.assign_task(&executor, &task_with_deadline);
    client.assign_task(&executor, &task_no_deadline);

    client.submit_result(&executor, &task_with_deadline, &result, &proof);
    client.submit_result(&executor, &task_no_deadline, &result, &proof);

    let info = client.get_executor_info(&executor).unwrap();
    assert_eq!(info.tasks_completed, 2);
}

/// Test that sequential operations maintain state order
#[test]
fn test_sequential_operations_maintain_order() {
    let (env, client) = setup();
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);

    client.register_executor(&executor);

    let input = zero_hash(&env);
    let result = zero_hash(&env);
    let proof = proof_for(&env, &input, &result);

    let mut task_ids = Vec::new();

    // Create 3 tasks sequentially
    for i in 0..3 {
        let task_id = client.submit_task(&creator, &input, &i, &0);
        task_ids.push(task_id);
    }

    // Verify they're in order
    assert_eq!(task_ids[0] + 1, task_ids[1]);
    assert_eq!(task_ids[1] + 1, task_ids[2]);

    // Assign and complete in sequence
    for task_id in &task_ids {
        client.assign_task(&executor, task_id);
        client.submit_result(&executor, task_id, &result, &proof);
    }

    let info = client.get_executor_info(&executor).unwrap();
    assert_eq!(info.tasks_completed, 3);
}
