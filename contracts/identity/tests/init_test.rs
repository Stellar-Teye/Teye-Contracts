#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(test)]

extern crate std;

use identity::{recovery::RecoveryError, IdentityContract, IdentityContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_pre_initialization_state() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);

    // Before initialize: owner should not be active, guardians empty, threshold 0
    assert!(!client.is_owner_active(&owner));
    assert_eq!(client.get_guardians(&owner).len(), 0);
    assert_eq!(client.get_recovery_threshold(&owner), 0);

    // Calling guarded methods without initialization should return Unauthorized or NoActiveRecovery as appropriate
    let attacker = Address::generate(&env);
    let new_guard = Address::generate(&env);
    assert_eq!(
        client.try_add_guardian(&attacker, &new_guard),
        Err(Ok(RecoveryError::Unauthorized))
    );
}

#[test]
fn test_initialize_sets_state_and_prevents_double_init() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);

    // Initialize should succeed and set owner active
    client.initialize(&owner);
    assert!(client.is_owner_active(&owner));
    assert_eq!(client.get_guardians(&owner).len(), 0);
    assert_eq!(client.get_recovery_threshold(&owner), 0);

    // Double initialization must fail with AlreadyInitialized
    assert_eq!(
        client.try_initialize(&Address::generate(&env)),
        Err(Ok(RecoveryError::AlreadyInitialized))
    );
}

#[test]
fn test_double_reinitialization_exploits() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);

    client.initialize(&owner);

    // Attempting to initialize the contract a second time
    let hacker = Address::generate(&env);
    let result = client.try_initialize(&hacker);

    assert_eq!(
        result,
        Err(Ok(RecoveryError::AlreadyInitialized)),
        "Double re-initialization exploits should revert"
    );
}
