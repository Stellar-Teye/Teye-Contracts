#![allow(clippy::unwrap_used, clippy::expect_used)]

use teye_delegation::{DelegationContract, DelegationContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ============================================================================
// Test Setup Helpers
// ============================================================================

/// Create environment and contract client without initializing.
fn setup_uninit() -> (Env, DelegationContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DelegationContract, ());
    let client = DelegationContractClient::new(&env, &contract_id);

    (env, client)
}

/// Create environment, contract client, and perform first initialization.
fn setup() -> (Env, DelegationContractClient<'static>, Address) {
    let (env, client) = setup_uninit();
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

// ============================================================================
// Double Re-initialization Exploit Tests
// ============================================================================

/// Core test: calling initialize a second time must panic with
/// "Already initialized" and must NOT overwrite the original admin.
#[test]
fn test_double_initialization_reverts() {
    let (env, client, _original_admin) = setup();

    let attacker = Address::generate(&env);

    // Second call must panic — contract is already initialized.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.initialize(&attacker);
    }));

    assert!(
        result.is_err(),
        "Expected panic on double initialization, but call succeeded"
    );

    // Verify the original admin was NOT overwritten by confirming
    // contract state is still functional under the original setup.
    let creator = Address::generate(&env);
    let input_data = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    let task_id = client.submit_task(&creator, &input_data, &1, &1000);
    let task = client.get_task(&task_id).expect("Contract should remain operational");
    assert_eq!(task.id, task_id);
    assert_eq!(task.creator, creator);
}

/// Triple initialization: ensure the guard holds on the third call too.
#[test]
fn test_triple_initialization_reverts() {
    let (env, client, _admin) = setup();

    let second = Address::generate(&env);
    let third = Address::generate(&env);

    let result2 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.initialize(&second);
    }));
    assert!(
        result2.is_err(),
        "Second initialization should have panicked"
    );

    let result3 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.initialize(&third);
    }));
    assert!(
        result3.is_err(),
        "Third initialization should have panicked"
    );
}

/// Re-initialization with the same admin address must also be rejected.
#[test]
fn test_reinit_same_admin_reverts() {
    let (_env, client, original_admin) = setup();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.initialize(&original_admin);
    }));

    assert!(
        result.is_err(),
        "Re-initialization with the same admin should still panic"
    );
}

/// After a failed re-initialization attempt, the contract must remain
/// fully operational — no state corruption.
#[test]
fn test_contract_operational_after_failed_reinit() {
    let (env, client, _admin) = setup();

    let attacker = Address::generate(&env);

    // Attempt re-initialization (will panic internally)
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.initialize(&attacker);
    }));

    // Contract should still work: register executor, submit task, assign, complete
    let creator = Address::generate(&env);
    let executor = Address::generate(&env);
    let input = soroban_sdk::BytesN::from_array(&env, &[42u8; 32]);
    let result_data = soroban_sdk::BytesN::from_array(&env, &[43u8; 32]);

    let task_id = client.submit_task(&creator, &input, &5, &2000);
    client.register_executor(&executor);
    client.assign_task(&executor, &task_id);

    // Generate valid proof: H(input || result)
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(&input.to_array());
    data[32..].copy_from_slice(&result_data.to_array());
    let proof_bytes = env.crypto().sha256(&soroban_sdk::Bytes::from_slice(&env, &data));
    let proof = soroban_sdk::BytesN::from_array(&env, &proof_bytes.to_array());

    client.submit_result(&executor, &task_id, &result_data, &proof);

    let task = client.get_task(&task_id).expect("Task should exist");
    assert_eq!(task.status, teye_delegation::task_queue::TaskStatus::Completed);
}

/// Rapid-fire re-initialization attempts must all fail without corrupting state.
#[test]
fn test_rapid_reinit_attempts_all_revert() {
    let (env, client, _admin) = setup();

    for i in 0u8..10 {
        let attacker = Address::generate(&env);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.initialize(&attacker);
        }));
        assert!(
            result.is_err(),
            "Re-initialization attempt {} should have panicked",
            i
        );
    }

    // Contract still works after 10 failed re-init attempts
    let creator = Address::generate(&env);
    let input = soroban_sdk::BytesN::from_array(&env, &[99u8; 32]);
    let task_id = client.submit_task(&creator, &input, &1, &500);
    let task = client.get_task(&task_id).expect("Task should exist after rapid reinit attempts");
    assert_eq!(task.status, teye_delegation::task_queue::TaskStatus::Pending);
}

/// Ensure that a fresh (uninitialized) contract CAN be initialized once.
/// This is the positive control for the re-initialization tests.
#[test]
fn test_first_initialization_succeeds() {
    let (env, client) = setup_uninit();
    let admin = Address::generate(&env);

    // First initialization should not panic
    client.initialize(&admin);

    // Verify contract is operational
    let creator = Address::generate(&env);
    let input = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
    let task_id = client.submit_task(&creator, &input, &1, &100);
    assert_eq!(task_id, 1);
}
