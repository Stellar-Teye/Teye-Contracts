#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Double Re-initialization Exploit Tests for the CrossChain contract.
//!
//! These tests verify that the `initialize` function can only be called once
//! and that every subsequent call safely reverts with `CrossChainError::AlreadyInitialized`.

use cross_chain::{CrossChainContract, CrossChainContractClient, CrossChainError};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Register the contract and return the client (un-initialized).
fn setup_uninit() -> (Env, CrossChainContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    (env, client)
}

/// Register the contract, initialize it with `admin`, and return the triple.
fn setup() -> (Env, CrossChainContractClient<'static>, Address) {
    let (env, client) = setup_uninit();
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

// ---------------------------------------------------------------------------
// Core: single initialization succeeds
// ---------------------------------------------------------------------------

#[test]
fn test_first_initialization_succeeds() {
    let (env, client) = setup_uninit();
    let admin = Address::generate(&env);

    // First call must succeed without error.
    client.initialize(&admin);
}

// ---------------------------------------------------------------------------
// Double Re-initialization Exploits
// ---------------------------------------------------------------------------

#[test]
fn test_double_init_same_admin_reverts() {
    let (_env, client, admin) = setup();

    // Re-initializing with the *same* admin must fail.
    assert_eq!(
        client.try_initialize(&admin),
        Err(Ok(CrossChainError::AlreadyInitialized))
    );
}

#[test]
fn test_double_init_different_admin_reverts() {
    let (env, client, _admin) = setup();

    // An attacker supplying a *different* admin address must also be rejected.
    let attacker = Address::generate(&env);
    assert_eq!(
        client.try_initialize(&attacker),
        Err(Ok(CrossChainError::AlreadyInitialized))
    );
}

#[test]
fn test_triple_init_reverts_consistently() {
    let (_env, client, admin) = setup();

    // Every subsequent attempt must keep returning the same error.
    for _ in 0..2 {
        assert_eq!(
            client.try_initialize(&admin),
            Err(Ok(CrossChainError::AlreadyInitialized))
        );
    }
}

#[test]
fn test_reinit_with_multiple_distinct_addresses() {
    let (env, client, _admin) = setup();

    // Try re-initializing with several unique addresses — all must fail.
    for _ in 0..5 {
        let new_addr = Address::generate(&env);
        assert_eq!(
            client.try_initialize(&new_addr),
            Err(Ok(CrossChainError::AlreadyInitialized))
        );
    }
}

#[test]
fn test_reinit_does_not_overwrite_admin() {
    let (env, client, original_admin) = setup();

    let attacker = Address::generate(&env);

    // Attempt to overwrite the admin — must be rejected.
    assert_eq!(
        client.try_initialize(&attacker),
        Err(Ok(CrossChainError::AlreadyInitialized))
    );

    // Verify the original admin can still operate (add_relayer is admin-only).
    let relayer = Address::generate(&env);
    client.add_relayer(&original_admin, &relayer);
    assert!(client.is_relayer(&relayer));
}

#[test]
fn test_reinit_rejected_then_attacker_cannot_add_relayer() {
    let (env, client, _original_admin) = setup();

    let attacker = Address::generate(&env);

    // Attacker tries to re-initialize — rejected.
    assert_eq!(
        client.try_initialize(&attacker),
        Err(Ok(CrossChainError::AlreadyInitialized))
    );

    // Attacker also cannot use admin-gated functions.
    let relayer = Address::generate(&env);
    assert_eq!(
        client.try_add_relayer(&attacker, &relayer),
        Err(Ok(CrossChainError::Unauthorized))
    );
}

#[test]
fn test_operations_still_work_after_failed_reinit() {
    let (env, client, admin) = setup();

    // Failed re-init attempt.
    let _ = client.try_initialize(&Address::generate(&env));

    // Contract state is intact — normal operations work.
    let relayer = Address::generate(&env);
    client.add_relayer(&admin, &relayer);
    assert!(client.is_relayer(&relayer));

    let foreign_chain = soroban_sdk::String::from_str(&env, "ethereum");
    let foreign_address = soroban_sdk::String::from_str(&env, "0xabc");
    let local_addr = Address::generate(&env);
    client.map_identity(&admin, &foreign_chain, &foreign_address, &local_addr);

    let result = client.get_local_address(&foreign_chain, &foreign_address);
    assert_eq!(result, Some(local_addr));
}
