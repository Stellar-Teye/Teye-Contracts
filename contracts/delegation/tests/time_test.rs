#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, BytesN, Env,
};
use teye_delegation::DelegationContract;

fn make_bytes(env: &Env, val: u8) -> BytesN<32> {
    let mut arr = [0u8; 32];
    arr[0] = val;
    BytesN::from_array(env, &arr)
}

fn ledger_at(ts: u64) -> LedgerInfo {
    LedgerInfo {
        timestamp: ts,
        protocol_version: 20,
        sequence_number: 100,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 10_000,
    }
}

fn setup() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(ledger_at(1_000));
    let contract_id = env.register_contract(None, DelegationContract);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    let client = teye_delegation::DelegationContractClient::new(&env, &contract_id);
    client.initialize(&admin);
    client.register_executor(&executor);
    (env, contract_id, creator, executor)
}

#[test]
fn test_task_deadline_in_future_is_valid() {
    let (env, contract_id, creator, _executor) = setup();
    let client = teye_delegation::DelegationContractClient::new(&env, &contract_id);
    let task_id = client.submit_task(&creator, &make_bytes(&env, 1), &1u32, &5_000u64);
    let task = client.get_task(&task_id).expect("Task should exist");
    assert_eq!(task.deadline, 5_000);
    assert!(env.ledger().timestamp() < task.deadline);
}

#[test]
#[should_panic]
fn test_assign_task_after_deadline_panics() {
    let (env, contract_id, creator, executor) = setup();
    let client = teye_delegation::DelegationContractClient::new(&env, &contract_id);
    let task_id = client.submit_task(&creator, &make_bytes(&env, 2), &1u32, &2_000u64);
    env.ledger().set(ledger_at(3_000));
    client.assign_task(&executor, &task_id);
}

#[test]
fn test_assign_task_before_deadline_succeeds() {
    let (env, contract_id, creator, executor) = setup();
    let client = teye_delegation::DelegationContractClient::new(&env, &contract_id);
    let task_id = client.submit_task(&creator, &make_bytes(&env, 3), &1u32, &5_000u64);
    client.assign_task(&executor, &task_id);
    let task = client.get_task(&task_id).expect("Task should exist");
    assert_eq!(task.executor, Some(executor));
}

#[test]
#[should_panic]
fn test_task_deadline_at_current_timestamp_expired() {
    let (env, contract_id, creator, executor) = setup();
    let client = teye_delegation::DelegationContractClient::new(&env, &contract_id);
    let task_id = client.submit_task(&creator, &make_bytes(&env, 4), &1u32, &1_000u64);
    client.assign_task(&executor, &task_id);
}

#[test]
#[should_panic]
fn test_submit_result_after_deadline_panics() {
    let (env, contract_id, creator, executor) = setup();
    let client = teye_delegation::DelegationContractClient::new(&env, &contract_id);
    let task_id = client.submit_task(&creator, &make_bytes(&env, 5), &1u32, &3_000u64);
    client.assign_task(&executor, &task_id);
    env.ledger().set(ledger_at(4_000));
    client.submit_result(
        &executor,
        &task_id,
        &make_bytes(&env, 10),
        &make_bytes(&env, 11),
    );
}

#[test]
fn test_submit_result_before_deadline_changes_status() {
    let (env, contract_id, creator, executor) = setup();
    let client = teye_delegation::DelegationContractClient::new(&env, &contract_id);
    let task_id = client.submit_task(&creator, &make_bytes(&env, 6), &1u32, &9_000u64);
    client.assign_task(&executor, &task_id);
    env.ledger().set(ledger_at(2_000));
    client.submit_result(
        &executor,
        &task_id,
        &make_bytes(&env, 20),
        &make_bytes(&env, 21),
    );
    let task = client.get_task(&task_id).expect("Task should exist");
    assert_ne!(format!("{:?}", task.status), "Assigned");
}

#[test]
fn test_high_priority_tight_deadline() {
    let (env, contract_id, creator, executor) = setup();
    let client = teye_delegation::DelegationContractClient::new(&env, &contract_id);
    let task_id = client.submit_task(&creator, &make_bytes(&env, 7), &10u32, &1_001u64);
    let task = client.get_task(&task_id).unwrap();
    assert_eq!(task.priority, 10);
    assert_eq!(task.deadline, 1_001);
    client.assign_task(&executor, &task_id);
}

#[test]
fn test_multiple_tasks_independent_deadlines() {
    let (env, contract_id, creator, executor) = setup();
    let client = teye_delegation::DelegationContractClient::new(&env, &contract_id);
    let executor2 = Address::generate(&env);
    client.register_executor(&executor2);
    let _short = client.submit_task(&creator, &make_bytes(&env, 8), &1u32, &2_000u64);
    let long = client.submit_task(&creator, &make_bytes(&env, 9), &1u32, &9_000u64);
    env.ledger().set(ledger_at(3_000));
    client.assign_task(&executor, &long);
    assert_eq!(client.get_task(&long).unwrap().executor, Some(executor));
}

#[test]
fn test_executor_reputation_changes_after_result() {
    let (env, contract_id, creator, executor) = setup();
    let client = teye_delegation::DelegationContractClient::new(&env, &contract_id);
    let rep_before = client.get_executor_info(&executor).unwrap().reputation;
    let task_id = client.submit_task(&creator, &make_bytes(&env, 10), &1u32, &9_000u64);
    client.assign_task(&executor, &task_id);
    client.submit_result(
        &executor,
        &task_id,
        &make_bytes(&env, 30),
        &make_bytes(&env, 31),
    );
    let rep_after = client.get_executor_info(&executor).unwrap().reputation;
    assert_ne!(rep_after, rep_before);
}

#[test]
fn test_get_nonexistent_task_returns_none() {
    let (env, contract_id, _c, _e) = setup();
    let client = teye_delegation::DelegationContractClient::new(&env, &contract_id);
    env.ledger().set(ledger_at(99_999));
    assert!(client.get_task(&999_999u64).is_none());
}
