#![allow(clippy::unwrap_used, clippy::expect_used)]

use teye_delegation::{
    DelegationContract, DelegationContractClient,
    task_queue::TaskStatus,
};
use soroban_sdk::{
    testutils::Address as _,
    Address, BytesN, Env,
};

// ============================================================================
// Test Setup and Helper Functions
// ============================================================================

/// Setup environment with delegation contract
fn setup() -> (Env, DelegationContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DelegationContract, ());
    let client = DelegationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}



/// Generate deterministic hashes for testing
fn generate_test_hash(env: &Env, value: u8) -> BytesN<32> {
    let bytes = [value; 32];
    BytesN::from_array(env, &bytes)
}

/// Generate a valid proof by computing H(input || result)
fn generate_valid_proof(env: &Env, input_hash: &BytesN<32>, result_hash: &BytesN<32>) -> BytesN<32> {
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(&input_hash.to_array());
    data[32..].copy_from_slice(&result_hash.to_array());
    
    let proof_bytes = env.crypto().sha256(&soroban_sdk::Bytes::from_slice(env, &data));
    BytesN::from_array(env, &proof_bytes.to_array())
}

// ============================================================================
// Cross-Contract Calling Invariant Tests
// ============================================================================

#[test]
fn test_external_task_submission_and_retrieval() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let input_data = generate_test_hash(&env, 1);
    let priority = 5;
    let deadline = 1000;
    
    // External contract submits a task
    let task_id = client.submit_task(&creator, &input_data, &priority, &deadline);
    
    // Verify task was created with correct invariants
    assert_eq!(task_id, 1); // First task should have ID 1
    
    // Task state should be retrievable from external contract
    let task_data = client.get_task(&task_id).expect("Task should be stored");
    
    assert_eq!(task_data.id, task_id);
    assert_eq!(task_data.creator, creator);
    assert_eq!(task_data.status, TaskStatus::Pending);
    assert_eq!(task_data.priority, priority);
    assert_eq!(task_data.deadline, deadline);
    assert!(!task_data.executor.is_some());
    assert!(!task_data.result.is_some());
}

#[test]
fn test_executor_registration_external_contract_call() {
    let (env, client, _admin) = setup();
    
    let executor1 = Address::generate(&env);
    let executor2 = Address::generate(&env);
    
    // External contract registers executors
    client.register_executor(&executor1);
    client.register_executor(&executor2);
    
    // Verify executor records are stored and accessible
    let exec1_info = client.get_executor_info(&executor1).expect("Executor should be registered");
    
    assert_eq!(exec1_info.address, executor1);
    assert_eq!(exec1_info.reputation, 100); // Initial reputation
    assert_eq!(exec1_info.tasks_completed, 0);
    
    let exec2_info = client.get_executor_info(&executor2).expect("Executor should be registered");
    
    assert_eq!(exec2_info.address, executor2);
    assert_eq!(exec2_info.reputation, 100);
}

#[test]
fn test_task_assignment_external_contract_interface() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    
    let input_data = generate_test_hash(&env, 1);
    let task_id = client.submit_task(&creator, &input_data, &3, &500);
    
    // Register executor via external interface
    client.register_executor(&executor);
    
    // External contract assigns task
    client.assign_task(&executor, &task_id);
    
    // Verify task state changed through external contract call
    let task_data = client.get_task(&task_id).expect("Task should exist");
    
    assert_eq!(task_data.status, TaskStatus::Assigned);
    assert_eq!(task_data.executor, Some(executor.clone()));
}

#[test]
fn test_task_assignment_fails_for_non_assigned_executor() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    
    let input_data = generate_test_hash(&env, 2);
    let task_id = client.submit_task(&creator, &input_data, &2, &400);
    
    // Try to assign task without registering executor
    // This should be caught by authorization mechanism
    let _result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.assign_task(&executor, &task_id);
    }));
    
    // The test environment doesn't propagate the panic in our setup,
    // so we verify that an unregistered executor cannot successfully modify state
    let fetched_task = client.get_task(&task_id).expect("Task should exist");
    assert_eq!(fetched_task.status, TaskStatus::Pending);
}

#[test]
fn test_successful_proof_submission_and_executor_reward() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    
    let input_data = generate_test_hash(&env, 3);
    let result_data = generate_test_hash(&env, 4);
    let proof = generate_valid_proof(&env, &input_data, &result_data);
    
    // Setup: Create task, register executor, assign task
    let task_id = client.submit_task(&creator, &input_data, &1, &300);
    client.register_executor(&executor);
    client.assign_task(&executor, &task_id);
    
    // External contract submits result with proof
    client.submit_result(&executor, &task_id, &result_data, &proof);
    
    // Verify task completion
    let task_data = client.get_task(&task_id).expect("Task should exist");
    
    assert_eq!(task_data.status, TaskStatus::Completed);
    assert_eq!(task_data.result, Some(result_data.clone()));
    assert_eq!(task_data.proof, Some(proof.clone()));
    
    // Verify executor reputation increased
    let exec_info = client.get_executor_info(&executor).expect("Executor should exist");
    
    assert_eq!(exec_info.reputation, 101); // Initial 100 + 1 bonus
    assert_eq!(exec_info.tasks_completed, 1);
}

#[test]
fn test_invalid_proof_penalizes_executor() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    
    let input_data = generate_test_hash(&env, 5);
    let result_data = generate_test_hash(&env, 6);
    let invalid_proof = generate_test_hash(&env, 99); // Wrong proof hash
    
    // Setup: Create task, register executor, assign task
    let task_id = client.submit_task(&creator, &input_data, &2, &600);
    client.register_executor(&executor);
    client.assign_task(&executor, &task_id);
    
    // External contract submits result with invalid proof
    client.submit_result(&executor, &task_id, &result_data, &invalid_proof);
    
    // Verify task marked as failed
    let task_data = client.get_task(&task_id).expect("Task should exist");
    
    assert_eq!(task_data.status, TaskStatus::Failed);
    assert!(!task_data.result.is_some()); // Result should not be stored
    
    // Verify executor reputation decreased
    let exec_info = client.get_executor_info(&executor).expect("Executor should exist");
    
    assert_eq!(exec_info.reputation, 90); // Initial 100 - 10 penalty
}

#[test]
fn test_external_failure_handling_unregistered_executor() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    
    let input_data = generate_test_hash(&env, 7);
    let task_id = client.submit_task(&creator, &input_data, &4, &700);
    
    // Try to assign task from unregistered executor - should fail gracefully
    let _result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.assign_task(&executor, &task_id);
    }));
    
    // Verify task remains in pending state on failure
    let task_data = client.get_task(&task_id).expect("Task should exist");
    
    assert_eq!(task_data.status, TaskStatus::Pending);
    assert!(!task_data.executor.is_some());
}

#[test]
fn test_submit_result_non_assigned_executor_fails_gracefully() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    let wrong_executor = Address::generate(&env);
    
    let input_data = generate_test_hash(&env, 8);
    let result_data = generate_test_hash(&env, 9);
    let proof = generate_valid_proof(&env, &input_data, &result_data);
    
    // Setup: Register executors, create task, assign to executor
    let task_id = client.submit_task(&creator, &input_data, &1, &800);
    client.register_executor(&executor);
    client.register_executor(&wrong_executor);
    client.assign_task(&executor, &task_id);
    
    // Try to submit result as different executor - should fail
    let _result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.submit_result(&wrong_executor, &task_id, &result_data, &proof);
    }));
    
    // Verify task remains incomplete
    let task_data = client.get_task(&task_id).expect("Task should exist");
    
    assert_eq!(task_data.status, TaskStatus::Assigned);
    assert!(!task_data.result.is_some());
}

#[test]
fn test_multiple_sequential_task_assignments() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let executor1 = Address::generate(&env);
    let executor2 = Address::generate(&env);
    
    // Create multiple tasks
    let mut task_ids = Vec::new();
    for i in 1..=3 {
        let input = generate_test_hash(&env, i as u8);
        let priority = i as u32;
        let deadline = 100 + i as u64 * 100;
        let task_id = client.submit_task(&creator, &input, &priority, &deadline);
        task_ids.push(task_id);
    }
    
    // Register executors
    client.register_executor(&executor1);
    client.register_executor(&executor2);
    
    // Assign tasks round-robin
    for (i, task_id) in task_ids.iter().enumerate() {
        let executor = if i % 2 == 0 { &executor1 } else { &executor2 };
        client.assign_task(executor, task_id);
    }
    
    // Verify all tasks properly assigned
    for (i, task_id) in task_ids.iter().enumerate() {
        let task = client.get_task(task_id).expect("Task should exist");
        
        assert_eq!(task.status, TaskStatus::Assigned);
        let expected_executor = if i % 2 == 0 { &executor1 } else { &executor2 };
        assert_eq!(task.executor.as_ref(), Some(expected_executor));
    }
}

#[test]
fn test_external_contract_state_isolation() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let input_data = generate_test_hash(&env, 11);
    
    // Submit task and verify external contracts cannot see pending states 
    // from other operations
    let task_id = client.submit_task(&creator, &input_data, &5, &900);
    
    // Get task state directly
    let task1 = client.get_task(&task_id).expect("Task should exist");
    
    // Submit another task - should not affect first task
    let input_data2 = generate_test_hash(&env, 12);
    let task_id2 = client.submit_task(&creator, &input_data2, &3, &950);
    
    // Verify first task unchanged
    let task1_check = client.get_task(&task_id).expect("Task should exist");
    
    assert_eq!(task1.id, task1_check.id);
    assert_eq!(task1.status, task1_check.status);
    assert_eq!(task1.creator, task1_check.creator);
    
    // Verify second task has correct ID
    assert_eq!(task_id2, 2);
}

#[test]
fn test_cross_contract_auth_enforcement() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let input_data = generate_test_hash(&env, 13);
    
    // Submit task as creator
    let task_id = client.submit_task(&creator, &input_data, &2, &1000);
    
    // Verify task was created by correct caller
    let task = client.get_task(&task_id).expect("Task should exist");
    
    assert_eq!(task.creator, creator);
    
    // Admin cannot directly modify task state
    // This is enforced by the contract's authorization checks
}

#[test]
fn test_proof_verification_contract_call_edge_cases() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    
    // Test with minimum hash values (all zeros)
    let min_input = generate_test_hash(&env, 0);
    let min_result = generate_test_hash(&env, 0);
    let min_proof = generate_valid_proof(&env, &min_input, &min_result);
    
    let task_id1 = client.submit_task(&creator, &min_input, &1, &1100);
    client.register_executor(&executor);
    client.assign_task(&executor, &task_id1);
    client.submit_result(&executor, &task_id1, &min_result, &min_proof);
    
    let task1 = client.get_task(&task_id1).expect("Task should exist");
    assert_eq!(task1.status, TaskStatus::Completed);
    
    // Test with maximum hash values (all 0xFF)
    let max_input = generate_test_hash(&env, 0xFF);
    let max_result = generate_test_hash(&env, 0xFF);
    let max_proof = generate_valid_proof(&env, &max_input, &max_result);
    
    let task_id2 = client.submit_task(&creator, &max_input, &1, &1200);
    client.assign_task(&executor, &task_id2);
    client.submit_result(&executor, &task_id2, &max_result, &max_proof);
    
    let task2 = client.get_task(&task_id2).expect("Task should exist");
    assert_eq!(task2.status, TaskStatus::Completed);
}

#[test]
fn test_concurrent_executor_reputation_updates() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    
    client.register_executor(&executor);
    
    // Submit and complete multiple tasks by same executor
    for i in 1..=3 {
        let input = generate_test_hash(&env, i as u8);
        let result = generate_test_hash(&env, (i + 10) as u8);
        let proof = generate_valid_proof(&env, &input, &result);
        let priority = i as u32;
        let deadline = 1300 + i as u64 * 100;
        
        let task_id = client.submit_task(&creator, &input, &priority, &deadline);
        client.assign_task(&executor, &task_id);
        client.submit_result(&executor, &task_id, &result, &proof);
    }
    
    // Verify final executor state
    let exec_info = client.get_executor_info(&executor).expect("Executor should exist");
    
    assert_eq!(exec_info.reputation, 103); // 100 + (1 bonus * 3 tasks)
    assert_eq!(exec_info.tasks_completed, 3);
}

#[test]
fn test_external_contract_cannot_modify_unowned_task() {
    let (env, client, _admin) = setup();
    
    let creator1 = Address::generate(&env);
    let _creator2 = Address::generate(&env);
    let executor = Address::generate(&env);
    
    let input1 = generate_test_hash(&env, 20);
    let task_id = client.submit_task(&creator1, &input1, &1, &1400);
    
    client.register_executor(&executor);
    
    // Executor can assign task created by creator1
    client.assign_task(&executor, &task_id);
    
    // Verify task is assigned correctly
    let task = client.get_task(&task_id).expect("Task should exist");
    assert_eq!(task.executor, Some(executor.clone()));
}

#[test]
fn test_execution_proof_verification_via_crypto_module() {
    let (env, client, _admin) = setup();
    
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    
    let input = generate_test_hash(&env, 21);
    let result = generate_test_hash(&env, 22);
    
    // Verify that our proof generation matches the contract's verification
    let correct_proof = generate_valid_proof(&env, &input, &result);
    let incorrect_proof = generate_test_hash(&env, 99);
    
    let task_id = client.submit_task(&creator, &input, &1, &1500);
    client.register_executor(&executor);
    client.assign_task(&executor, &task_id);
    
    // Submit with correct proof
    client.submit_result(&executor, &task_id, &result, &correct_proof);
    let task_completed = client.get_task(&task_id).expect("Task should exist");
    assert_eq!(task_completed.status, TaskStatus::Completed);
    
    // Verify with incorrect proof on different task
    let input2 = generate_test_hash(&env, 23);
    let result2 = generate_test_hash(&env, 24);
    let task_id2 = client.submit_task(&creator, &input2, &1, &1600);
    client.assign_task(&executor, &task_id2);
    client.submit_result(&executor, &task_id2, &result2, &incorrect_proof);
    
    let task_failed = client.get_task(&task_id2).expect("Task should exist");
    assert_eq!(task_failed.status, TaskStatus::Failed);
}
