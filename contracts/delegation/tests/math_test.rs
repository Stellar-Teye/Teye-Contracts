#![allow(clippy::unwrap_used, clippy::expect_used)]

use teye_delegation::{
    DelegationContract, DelegationContractClient,
    executor::ExecutorInfo,
};
use soroban_sdk::{
    testutils::Address as _,
    Address, BytesN, Env, Symbol, symbol_short,
};

/// Setup environment with delegation contract
fn setup() -> (Env, DelegationContractClient<'static>, Address, soroban_sdk::Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DelegationContract, ());
    let client = DelegationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin, contract_id)
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

#[test]
fn test_reputation_overflow_protection() {
    let (env, client, _admin, contract_id) = setup();
    let executor = Address::generate(&env);
    
    // Manually inject an executor with max reputation into storage
    let executors_key: Symbol = symbol_short!("EXECS");
    let info = ExecutorInfo {
        address: executor.clone(),
        reputation: u32::MAX,
        tasks_completed: 0,
        last_active: env.ledger().timestamp(),
    };
    
    env.as_contract(&contract_id, || {
        env.storage().persistent().set(&(executors_key, executor.clone()), &info);
    });

    // Setup a task to complete
    let creator = Address::generate(&env);
    let input_data = generate_test_hash(&env, 1);
    let result_data = generate_test_hash(&env, 2);
    let proof = generate_valid_proof(&env, &input_data, &result_data);
    
    let task_id = client.submit_task(&creator, &input_data, &1, &1000);
    client.assign_task(&executor, &task_id);

    // This should panic due to u32 overflow (reputation += 1)
    let result = client.try_submit_result(&executor, &task_id, &result_data, &proof);
    
    // Verify it failed (panicked in the contract)
    assert!(result.is_err());
}

#[test]
fn test_tasks_completed_overflow_protection() {
    let (env, client, _admin, contract_id) = setup();
    let executor = Address::generate(&env);
    
    // Manually inject an executor with max tasks_completed into storage
    let executors_key: Symbol = symbol_short!("EXECS");
    let info = ExecutorInfo {
        address: executor.clone(),
        reputation: 100,
        tasks_completed: u64::MAX,
        last_active: env.ledger().timestamp(),
    };
    
    env.as_contract(&contract_id, || {
        env.storage().persistent().set(&(executors_key, executor.clone()), &info);
    });

    // Setup a task to complete
    let creator = Address::generate(&env);
    let input_data = generate_test_hash(&env, 3);
    let result_data = generate_test_hash(&env, 4);
    let proof = generate_valid_proof(&env, &input_data, &result_data);
    
    let task_id = client.submit_task(&creator, &input_data, &1, &1000);
    client.assign_task(&executor, &task_id);

    // This should panic due to u64 overflow (tasks_completed += 1)
    let result = client.try_submit_result(&executor, &task_id, &result_data, &proof);
    
    assert!(result.is_err());
}

#[test]
fn test_task_id_counter_overflow() {
    let (env, client, _admin, contract_id) = setup();
    
    // Manually set the task counter to u64::MAX
    let task_counter_key: Symbol = symbol_short!("TASK_CTR");
    env.as_contract(&contract_id, || {
        env.storage().instance().set(&task_counter_key, &u64::MAX);
    });

    let creator = Address::generate(&env);
    let input_data = generate_test_hash(&env, 5);
    
    // This should panic in next_task_id (id += 1)
    let result = client.try_submit_task(&creator, &input_data, &1, &1000);
    
    assert!(result.is_err());
}

#[test]
fn test_slash_executor_underflow_saturation() {
    let (env, client, _admin, contract_id) = setup();
    let executor = Address::generate(&env);
    
    // Register executor (starts with 100 reputation)
    client.register_executor(&executor);
    
    // Verify initial reputation
    let info = client.get_executor_info(&executor).unwrap();
    assert_eq!(info.reputation, 100);

    // Setup a task and fail it with an invalid proof
    let creator = Address::generate(&env);
    let input_data = generate_test_hash(&env, 6);
    let result_data = generate_test_hash(&env, 7);
    let invalid_proof = generate_test_hash(&env, 99);
    
    let task_id = client.submit_task(&creator, &input_data, &1, &1000);
    client.assign_task(&executor, &task_id);
    
    // First slash: 100 -> 90
    client.submit_result(&executor, &task_id, &result_data, &invalid_proof);
    let info = client.get_executor_info(&executor).unwrap();
    assert_eq!(info.reputation, 90);

    // Manually set reputation to low value to test saturation
    let executors_key: Symbol = symbol_short!("EXECS");
    let mut low_info = info;
    low_info.reputation = 5;
    
    env.as_contract(&contract_id, || {
        env.storage().persistent().set(&(executors_key, executor.clone()), &low_info);
    });

    // Create another task and fail it
    let task_id2 = client.submit_task(&creator, &input_data, &1, &2000);
    client.assign_task(&executor, &task_id2);
    
    // Second slash: 5 -> 0 (saturated, not underflow)
    client.submit_result(&executor, &task_id2, &result_data, &invalid_proof);
    
    let final_info = client.get_executor_info(&executor).unwrap();
    assert_eq!(final_info.reputation, 0);
}
