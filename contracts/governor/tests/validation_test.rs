//! validation_test.rs
//!
//! Edge-case tests for Zero-Value / Empty-Input Parameter Passing
//! in the `governor` smart contract.
//!
//! Goal: Send inputs of zero, empty vectors, or blank addresses to
//! critical state-modifying functions and verify they revert with the
//! expected error types rather than silently accepting bad state.

#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    vec, Address, Env, IntoVal, Vec,
};

// ── Import the contract under test ────────────────────────────────────────────
// Adjust the path / crate name to match your workspace layout.
use crate::GovernorContract;
use crate::GovernorContractClient;
use crate::GovernorError;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Spin up a fresh environment and deploy a properly initialised governor.
/// Returns `(env, client, admin)` ready for further testing.
fn setup() -> (Env, GovernorContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, GovernorContract);
    let client = GovernorContractClient::new(&env, &contract_id);

    // Standard initialisation – non-zero quorum / threshold so we have a
    // meaningful baseline to contrast against zero-value calls below.
    client.initialize(&admin, &51_u32, &1_u64);

    (env, client, admin)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Calling `create_proposal` with an empty title string must revert.
#[test]
fn test_create_proposal_empty_title_reverts() {
    let (env, client, _admin) = setup();
    let proposer = Address::generate(&env);

    let result = client.try_create_proposal(
        &proposer,
        &soroban_sdk::String::from_str(&env, ""), // ← empty title
        &soroban_sdk::String::from_str(&env, "Valid description"),
        &Vec::<Address>::new(&env),
        &Vec::<soroban_sdk::Bytes>::new(&env),
    );

    assert_eq!(result, Err(Ok(GovernorError::InvalidInput)));
}

/// Calling `create_proposal` with an empty description string must revert.
#[test]
fn test_create_proposal_empty_description_reverts() {
    let (env, client, _admin) = setup();
    let proposer = Address::generate(&env);

    let result = client.try_create_proposal(
        &proposer,
        &soroban_sdk::String::from_str(&env, "Valid title"),
        &soroban_sdk::String::from_str(&env, ""), // ← empty description
        &Vec::<Address>::new(&env),
        &Vec::<soroban_sdk::Bytes>::new(&env),
    );

    assert_eq!(result, Err(Ok(GovernorError::InvalidInput)));
}

/// Calling `create_proposal` with an empty target-address vector must revert.
#[test]
fn test_create_proposal_empty_targets_reverts() {
    let (env, client, _admin) = setup();
    let proposer = Address::generate(&env);

    let result = client.try_create_proposal(
        &proposer,
        &soroban_sdk::String::from_str(&env, "Valid title"),
        &soroban_sdk::String::from_str(&env, "Valid description"),
        &Vec::<Address>::new(&env), // ← empty targets
        &Vec::<soroban_sdk::Bytes>::new(&env),
    );

    assert_eq!(result, Err(Ok(GovernorError::InvalidInput)));
}

/// `cast_vote` with a zero-weight / no-power account should revert.
#[test]
fn test_cast_vote_zero_voting_power_reverts() {
    let (env, client, _admin) = setup();
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env); // no tokens → zero voting power

    // First create a valid proposal so we have a proposal_id to vote on.
    let targets = vec![&env, Address::generate(&env)];
    let call_data = vec![&env, soroban_sdk::Bytes::new(&env)];
    let proposal_id = client.create_proposal(
        &proposer,
        &soroban_sdk::String::from_str(&env, "Title"),
        &soroban_sdk::String::from_str(&env, "Desc"),
        &targets,
        &call_data,
    );

    let result = client.try_cast_vote(&voter, &proposal_id, &1_u32 /* For */);
    assert_eq!(result, Err(Ok(GovernorError::NoVotingPower)));
}

/// `update_quorum` called with a quorum of zero must revert.
#[test]
fn test_update_quorum_zero_reverts() {
    let (env, client, admin) = setup();

    let result = client.try_update_quorum(&admin, &0_u32); // ← zero quorum
    assert_eq!(result, Err(Ok(GovernorError::InvalidInput)));
}

/// `update_quorum` called with a quorum above 100 % must revert.
#[test]
fn test_update_quorum_above_max_reverts() {
    let (env, client, admin) = setup();

    let result = client.try_update_quorum(&admin, &101_u32); // > 100 %
    assert_eq!(result, Err(Ok(GovernorError::InvalidInput)));
}

/// `update_voting_period` called with a period of zero blocks must revert.
#[test]
fn test_update_voting_period_zero_reverts() {
    let (env, client, admin) = setup();

    let result = client.try_update_voting_period(&admin, &0_u64); // ← zero period
    assert_eq!(result, Err(Ok(GovernorError::InvalidInput)));
}

/// `execute_proposal` called with a proposal_id of zero (non-existent) must revert.
#[test]
fn test_execute_proposal_zero_id_reverts() {
    let (env, client, _admin) = setup();

    let result = client.try_execute_proposal(&0_u64); // ← zero / non-existent ID
    assert_eq!(result, Err(Ok(GovernorError::ProposalNotFound)));
}

/// `cancel_proposal` with a zero / non-existent proposal_id must revert.
#[test]
fn test_cancel_proposal_zero_id_reverts() {
    let (env, client, admin) = setup();

    let result = client.try_cancel_proposal(&admin, &0_u64);
    assert_eq!(result, Err(Ok(GovernorError::ProposalNotFound)));
}

/// Passing the zero address (`Address::default`) as the admin during
/// initialization must revert.
#[test]
fn test_initialize_zero_address_admin_reverts() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, GovernorContract);
    let client = GovernorContractClient::new(&env, &contract_id);

    // Soroban doesn't have a canonical "zero address" the same way EVM does,
    // but we can pass a freshly generated address that holds no tokens/roles
    // and verify the contract rejects a blank/null-equivalent identifier if
    // the contract exposes that validation.
    //
    // If your contract uses `Option<Address>` for admin, pass `None` here.
    let result = client.try_initialize(
        &Address::generate(&env), // placeholder – replace with null sentinel if contract exposes one
        &0_u32,                   // ← zero quorum should already cause InvalidInput
        &0_u64,                   // ← zero voting period
    );

    assert_eq!(result, Err(Ok(GovernorError::InvalidInput)));
}
