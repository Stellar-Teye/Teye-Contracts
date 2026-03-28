//! init_test.rs
//!
//! Tests for Double Re-initialisation Exploits in the `governor` contract.
//!
//! Goal: Verify that calling the initialisation function more than once
//! causes the contract to revert with an appropriate error, preventing
//! anyone from hijacking ownership or resetting governance parameters.

#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::GovernorContract;
use crate::GovernorContractClient;
use crate::GovernorError;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn deploy_uninitialised(env: &Env) -> GovernorContractClient<'static> {
    let contract_id = env.register_contract(None, GovernorContract);
    GovernorContractClient::new(env, &contract_id)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// The most basic re-init attempt: same caller, same params, second call must
/// revert with `AlreadyInitialized`.
#[test]
fn test_double_initialize_same_caller_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let client = deploy_uninitialised(&env);

    // First call – must succeed.
    client.initialize(&admin, &51_u32, &100_u64);

    // Second call – must revert.
    let result = client.try_initialize(&admin, &51_u32, &100_u64);
    assert_eq!(
        result,
        Err(Ok(GovernorError::AlreadyInitialized)),
        "Re-initialising with the same admin must be rejected"
    );
}

/// An attacker uses a *different* address hoping to hijack the admin role.
/// The contract must still revert with `AlreadyInitialized`.
#[test]
fn test_double_initialize_attacker_address_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    let legitimate_admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let client = deploy_uninitialised(&env);

    client.initialize(&legitimate_admin, &51_u32, &100_u64);

    let result = client.try_initialize(&attacker, &99_u32, &1_u64);
    assert_eq!(
        result,
        Err(Ok(GovernorError::AlreadyInitialized)),
        "Attacker must not be able to re-initialise the contract"
    );
}

/// After a failed re-init attempt the original admin must still be in place.
#[test]
fn test_admin_unchanged_after_reinit_attempt() {
    let env = Env::default();
    env.mock_all_auths();

    let legitimate_admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let client = deploy_uninitialised(&env);

    client.initialize(&legitimate_admin, &51_u32, &100_u64);

    // Attempt to replace admin – must fail.
    let _ = client.try_initialize(&attacker, &99_u32, &1_u64);

    // The stored admin must still be the original one.
    assert_eq!(
        client.get_admin(),
        legitimate_admin,
        "Admin must remain unchanged after a rejected re-initialisation"
    );
}

/// Governance parameters (quorum) must remain unchanged after a failed re-init.
#[test]
fn test_quorum_unchanged_after_reinit_attempt() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let client = deploy_uninitialised(&env);

    client.initialize(&admin, &51_u32, &100_u64);

    // Attempt re-init with a different quorum value.
    let _ = client.try_initialize(&attacker, &10_u32, &5_u64);

    assert_eq!(
        client.get_quorum(),
        51_u32,
        "Quorum must not be overwritten by a failed re-initialisation"
    );
}

/// Voting period must remain unchanged after a failed re-init.
#[test]
fn test_voting_period_unchanged_after_reinit_attempt() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let client = deploy_uninitialised(&env);

    client.initialize(&admin, &51_u32, &100_u64);

    let _ = client.try_initialize(&attacker, &51_u32, &9999_u64);

    assert_eq!(
        client.get_voting_period(),
        100_u64,
        "Voting period must not be overwritten by a failed re-initialisation"
    );
}

/// Re-initialisation must be rejected even after legitimate admin operations
/// have taken place (i.e., the guard is persistent, not one-shot).
#[test]
fn test_reinit_blocked_after_admin_operations() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let client = deploy_uninitialised(&env);

    client.initialize(&admin, &51_u32, &100_u64);

    // Perform some legitimate admin operations.
    client.update_quorum(&admin, &60_u32);
    client.update_voting_period(&admin, &200_u64);

    // Even after state mutations the re-init guard must hold.
    let result = client.try_initialize(&admin, &51_u32, &100_u64);
    assert_eq!(
        result,
        Err(Ok(GovernorError::AlreadyInitialized)),
        "Re-initialisation must still be rejected after subsequent admin operations"
    );
}

/// Triple-init: a second re-init attempt must fail for the same reason.
#[test]
fn test_triple_initialize_all_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attacker_a = Address::generate(&env);
    let attacker_b = Address::generate(&env);
    let client = deploy_uninitialised(&env);

    client.initialize(&admin, &51_u32, &100_u64);

    let result_a = client.try_initialize(&attacker_a, &51_u32, &100_u64);
    let result_b = client.try_initialize(&attacker_b, &51_u32, &100_u64);

    assert_eq!(result_a, Err(Ok(GovernorError::AlreadyInitialized)));
    assert_eq!(result_b, Err(Ok(GovernorError::AlreadyInitialized)));
}