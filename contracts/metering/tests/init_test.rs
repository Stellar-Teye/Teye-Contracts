#![allow(clippy::unwrap_used, clippy::expect_used)]

use metering::{
    GasCosts, MeteringContract, MeteringContractClient, MeteringError, OperationType, TenantLevel,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn uninitialized_setup() -> (Env, MeteringContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MeteringContract, ());
    let client = MeteringContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let outsider = Address::generate(&env);

    (env, client, admin, outsider)
}

fn setup() -> (Env, MeteringContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MeteringContract, ());
    let client = MeteringContractClient::new(&env, &contract_id);

    (env, client)
}

#[test]
fn initialize_sets_initial_admin_and_default_cost_constraints() {
    let (_env, client, admin, _outsider) = uninitialized_setup();

    client.initialize(&admin);

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_gas_costs(), GasCosts::default_costs());
}

#[test]
fn stateful_entrypoints_reject_calls_before_initialization() {
    let (env, client, admin, outsider) = uninitialized_setup();
    let tenant = Address::generate(&env);
    let costs = GasCosts {
        read_cost: 2,
        write_cost: 6,
        compute_cost: 11,
        storage_cost: 4,
    };

    assert_eq!(
        client.try_get_admin(),
        Err(Ok(MeteringError::NotInitialized))
    );
    assert_eq!(
        client.try_set_gas_costs(&admin, &costs),
        Err(Ok(MeteringError::NotInitialized))
    );
    assert_eq!(
        client.try_register_tenant(&admin, &tenant, &TenantLevel::Organization, &tenant),
        Err(Ok(MeteringError::NotInitialized))
    );
    assert_eq!(
        client.try_record_gas(&outsider, &tenant, &OperationType::Read),
        Err(Ok(MeteringError::NotInitialized))
    );
}

#[test]
fn second_initialize_cannot_overwrite_original_admin_or_defaults() {
    let (_env, client, admin, outsider) = uninitialized_setup();

    client.initialize(&admin);

    let reinitialize = client.try_initialize(&outsider);
    assert_eq!(reinitialize, Err(Ok(MeteringError::AlreadyInitialized)));

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_gas_costs(), GasCosts::default_costs());
}

#[test]
fn post_initialize_constraints_still_enforce_admin_boundaries() {
    let (_env, client, admin, outsider) = uninitialized_setup();
    let custom_costs = GasCosts {
        read_cost: 3,
        write_cost: 7,
        compute_cost: 12,
        storage_cost: 5,
    };

    client.initialize(&admin);

    assert_eq!(
        client.try_set_gas_costs(&outsider, &custom_costs),
        Err(Ok(MeteringError::Unauthorized))
    );
    assert_eq!(client.get_gas_costs(), GasCosts::default_costs());
}

#[test]
fn test_successful_initialization() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    // First initialization should succeed
    client.initialize(&admin);

    // Verify admin is set correctly
    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin);
}

#[test]
fn test_double_initialization_fails() {
    let (env, client) = setup();
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);

    // First initialization should succeed
    client.initialize(&admin1);

    // Second initialization with same admin should fail
    let result2 = client.try_initialize(&admin1);
    assert_eq!(result2, Err(Ok(MeteringError::AlreadyInitialized)));

    // Second initialization with different admin should also fail
    let result3 = client.try_initialize(&admin2);
    assert_eq!(result3, Err(Ok(MeteringError::AlreadyInitialized)));

    // Verify original admin is still set
    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin1);
}

#[test]
fn test_multiple_initialization_attempts() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    // First initialization should succeed
    client.initialize(&admin);

    // Multiple subsequent attempts should all fail with AlreadyInitialized
    for _ in 0..5 {
        let new_admin = Address::generate(&env);
        let result = client.try_initialize(&new_admin);
        assert_eq!(result, Err(Ok(MeteringError::AlreadyInitialized)));
    }

    // Verify original admin is still set
    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin);
}

#[test]
fn test_initialization_state_persistence() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    // Initialize the contract
    client.initialize(&admin);

    // Verify that contract functions requiring initialization work
    let gas_costs = client.get_gas_costs();
    assert!(gas_costs.cost_for(&metering::OperationType::Read) > 0);

    // Verify that attempting to initialize again still fails
    let result2 = client.try_initialize(&admin);
    assert_eq!(result2, Err(Ok(MeteringError::AlreadyInitialized)));
}

#[test]
fn test_initialization_with_different_admins_after_failure() {
    let (env, client) = setup();
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admin3 = Address::generate(&env);

    // First initialization succeeds
    client.initialize(&admin1);

    // Subsequent attempts with different admins should fail
    let result2 = client.try_initialize(&admin2);
    assert_eq!(result2, Err(Ok(MeteringError::AlreadyInitialized)));

    let result3 = client.try_initialize(&admin3);
    assert_eq!(result3, Err(Ok(MeteringError::AlreadyInitialized)));

    // Original admin should still be the admin
    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin1);
}

#[test]
fn test_contract_functions_fail_before_initialization() {
    let (env, client) = setup();

    // Attempting to get admin before initialization should fail
    let result = client.try_get_admin();
    assert_eq!(result, Err(Ok(MeteringError::NotInitialized)));

    // Now initialize and verify it works
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin);
}

#[test]
fn test_initialization_exploit_prevention() {
    let (env, client) = setup();
    let original_admin = Address::generate(&env);
    let malicious_admin = Address::generate(&env);

    // Legitimate initialization
    client.initialize(&original_admin);

    // Simulate various attack scenarios

    // 1. Attempt to re-initialize with the same admin (should fail)
    let attack1 = client.try_initialize(&original_admin);
    assert_eq!(attack1, Err(Ok(MeteringError::AlreadyInitialized)));

    // 2. Attempt to re-initialize with a different admin (should fail)
    let attack2 = client.try_initialize(&malicious_admin);
    assert_eq!(attack2, Err(Ok(MeteringError::AlreadyInitialized)));

    // 3. Rapid-fire initialization attempts (should all fail)
    for i in 0..10 {
        let attacker = Address::generate(&env);
        let result = client.try_initialize(&attacker);
        assert_eq!(
            result,
            Err(Ok(MeteringError::AlreadyInitialized)),
            "Attack attempt {} should have failed",
            i
        );
    }

    // 4. Verify the original admin is still in control
    let current_admin = client.get_admin();
    assert_eq!(current_admin, original_admin);

    // 5. Verify contract functionality still works with original admin
    let gas_costs = client.get_gas_costs();
    assert!(gas_costs.cost_for(&metering::OperationType::Read) > 0);
    assert!(gas_costs.cost_for(&metering::OperationType::Write) > 0);
    assert!(gas_costs.cost_for(&metering::OperationType::Compute) > 0);
    assert!(gas_costs.cost_for(&metering::OperationType::Storage) > 0);
}

#[test]
fn test_initialization_error_consistency() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    // Initialize once
    client.initialize(&admin);

    // All subsequent initialization attempts should return the same error
    let error_type = MeteringError::AlreadyInitialized;

    for _ in 0..3 {
        let result = client.try_initialize(&Address::generate(&env));
        assert_eq!(result, Err(Ok(error_type)));
    }
}
