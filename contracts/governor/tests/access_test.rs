//! access_test.rs
//!
//! Tests for Unauthenticated Admin Function Calls in the `governor` contract.
//!
//! Goal: Ensure every admin-gated function reverts with `GovernorError::Unauthorized`
//! (or equivalent) when invoked by an arbitrary, unauthenticated address.

#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, vec, Address, Bytes, Env, String, Vec};

use crate::GovernorContract;
use crate::GovernorContractClient;
use crate::GovernorError;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Deploy and initialise the governor; return `(env, client, admin, stranger)`.
/// `stranger` is a freshly generated address with no roles whatsoever.
fn setup() -> (Env, GovernorContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let stranger = Address::generate(&env);

    let contract_id = env.register_contract(None, GovernorContract);
    let client = GovernorContractClient::new(&env, &contract_id);

    client.initialize(&admin, &51_u32, &100_u64);

    (env, client, admin, stranger)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// A random address cannot update the quorum threshold.
#[test]
fn test_update_quorum_unauthorized() {
    let (env, client, _admin, stranger) = setup();
    env.mock_all_auths_allowing_non_root_auth();

    let result = client.try_update_quorum(&stranger, &60_u32);
    assert_eq!(result, Err(Ok(GovernorError::Unauthorized)));
}

/// A random address cannot change the voting period.
#[test]
fn test_update_voting_period_unauthorized() {
    let (env, client, _admin, stranger) = setup();
    env.mock_all_auths_allowing_non_root_auth();

    let result = client.try_update_voting_period(&stranger, &200_u64);
    assert_eq!(result, Err(Ok(GovernorError::Unauthorized)));
}

/// A random address cannot transfer admin ownership.
#[test]
fn test_transfer_admin_unauthorized() {
    let (env, client, _admin, stranger) = setup();
    env.mock_all_auths_allowing_non_root_auth();

    let new_admin = Address::generate(&env);
    let result = client.try_transfer_admin(&stranger, &new_admin);
    assert_eq!(result, Err(Ok(GovernorError::Unauthorized)));
}

/// A random address cannot cancel another user's proposal.
#[test]
fn test_cancel_proposal_unauthorized() {
    let (env, client, _admin, stranger) = setup();
    env.mock_all_auths_allowing_non_root_auth();

    // Create a valid proposal as admin first so we have a real proposal_id.
    let targets = vec![&env, Address::generate(&env)];
    let call_data = vec![&env, Bytes::new(&env)];
    let proposal_id = client.create_proposal(
        &_admin,
        &String::from_str(&env, "Title"),
        &String::from_str(&env, "Description"),
        &targets,
        &call_data,
    );

    let result = client.try_cancel_proposal(&stranger, &proposal_id);
    assert_eq!(result, Err(Ok(GovernorError::Unauthorized)));
}

/// A random address cannot pause the contract.
#[test]
fn test_pause_unauthorized() {
    let (env, client, _admin, stranger) = setup();
    env.mock_all_auths_allowing_non_root_auth();

    let result = client.try_pause(&stranger);
    assert_eq!(result, Err(Ok(GovernorError::Unauthorized)));
}

/// A random address cannot unpause the contract.
#[test]
fn test_unpause_unauthorized() {
    let (env, client, admin, stranger) = setup();
    env.mock_all_auths_allowing_non_root_auth();

    // Pause first (as admin) so unpause has meaningful state to revert.
    client.pause(&admin);

    let result = client.try_unpause(&stranger);
    assert_eq!(result, Err(Ok(GovernorError::Unauthorized)));
}

/// A random address cannot add a new guardian / council member.
#[test]
fn test_add_guardian_unauthorized() {
    let (env, client, _admin, stranger) = setup();
    env.mock_all_auths_allowing_non_root_auth();

    let new_guardian = Address::generate(&env);
    let result = client.try_add_guardian(&stranger, &new_guardian);
    assert_eq!(result, Err(Ok(GovernorError::Unauthorized)));
}

/// A random address cannot remove a guardian.
#[test]
fn test_remove_guardian_unauthorized() {
    let (env, client, admin, stranger) = setup();
    env.mock_all_auths_allowing_non_root_auth();

    // Add a guardian as admin first.
    let guardian = Address::generate(&env);
    client.add_guardian(&admin, &guardian);

    let result = client.try_remove_guardian(&stranger, &guardian);
    assert_eq!(result, Err(Ok(GovernorError::Unauthorized)));
}

/// A random address cannot execute a passed proposal ahead of schedule.
#[test]
fn test_execute_proposal_unauthorized() {
    let (env, client, _admin, stranger) = setup();
    env.mock_all_auths_allowing_non_root_auth();

    // Use a non-existent ID; the contract should reject auth before even
    // checking whether the proposal exists.
    let result = client.try_execute_proposal_as_admin(&stranger, &999_u64);
    assert_eq!(result, Err(Ok(GovernorError::Unauthorized)));
}

/// Verifying that the legitimate admin CAN still update the quorum (sanity
/// check so we don't mistake a broken function for an access-control win).
#[test]
fn test_update_quorum_by_admin_succeeds() {
    let (env, client, admin, _stranger) = setup();

    let result = client.try_update_quorum(&admin, &65_u32);
    assert!(result.is_ok(), "Admin should be able to update quorum");
    assert_eq!(client.get_quorum(), 65_u32);
}