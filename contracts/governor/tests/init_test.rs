//! Initialization Constraints Validation tests for the Governor contract.
//!
//! These tests verify that:
//! - The contract boots correctly with valid parameters
//! - Re-initialization is rejected (`AlreadyInitialized`)
//! - Invalid inputs (zero/negative supply) are rejected
//! - All state is set deterministically after initialization
//! - Operations fail with `NotInitialized` on an uninitialized contract
//! - Admin-gated functions enforce the stored admin identity
//! - `is_initialized` reports correct status before and after init

#![cfg(test)]
#![allow(clippy::unwrap_used)]

extern crate std;

use governor::{
    proposal::{ProposalAction, ProposalType},
    voting::VoteChoice,
    ContractError, GovernorContract, GovernorContractClient,
};
use soroban_sdk::{
    symbol_short,
    testutils::Address as _,
    vec, Address, BytesN, Env, String,
};

// ── Helpers ────────────────────────────────────────────────────────────────

/// Create an environment and register the governor contract without initializing.
fn setup_uninit() -> (Env, Address, GovernorContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &contract_id);

    (env, contract_id, client)
}

/// Create an initialized governor with sensible defaults.
fn setup() -> (Env, Address, GovernorContractClient<'static>, Address) {
    let (env, contract_id, client) = setup_uninit();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.initialize(&admin, &staking, &treasury, &1_000_000i128);

    (env, contract_id, client, admin)
}

fn set_mock_stake(env: &Env, contract_id: &Address, voter: &Address, amount: i128) {
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .set(&(symbol_short!("M_STK"), voter.clone()), &amount);
    });
}

fn set_mock_age(env: &Env, contract_id: &Address, voter: &Address, age_secs: u64) {
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .set(&(symbol_short!("M_AGE"), voter.clone()), &age_secs);
    });
}

fn dummy_action(env: &Env) -> ProposalAction {
    ProposalAction {
        target: Address::generate(env),
        function: symbol_short!("GOV_PRM"),
        params_hash: BytesN::from_array(env, &[0u8; 32]),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Successful Initialization
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn initialize_sets_initialized_flag() {
    let (_env, _contract_id, client, _admin) = setup();
    assert!(client.is_initialized());
}

#[test]
fn initialize_stores_admin() {
    let (_env, _contract_id, client, admin) = setup();
    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin);
}

#[test]
fn initialize_with_minimum_valid_supply() {
    let (env, _contract_id, client) = setup_uninit();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    // total_vote_supply = 1 is the smallest valid value.
    client.initialize(&admin, &staking, &treasury, &1i128);
    assert!(client.is_initialized());
}

#[test]
fn initialize_with_large_supply() {
    let (env, _contract_id, client) = setup_uninit();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Very large supply should be accepted.
    client.initialize(&admin, &staking, &treasury, &i128::MAX);
    assert!(client.is_initialized());
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Re-Initialization Guard
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn double_initialization_rejected() {
    let (env, _contract_id, client, admin) = setup();

    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Second initialize must fail with AlreadyInitialized.
    let result = client.try_initialize(&admin, &staking, &treasury, &500i128);
    assert_eq!(
        result,
        Err(Ok(ContractError::AlreadyInitialized))
    );
}

#[test]
fn double_initialization_with_different_admin_rejected() {
    let (env, _contract_id, client, _admin) = setup();

    let new_admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Even with different parameters, re-init must be blocked.
    let result = client.try_initialize(&new_admin, &staking, &treasury, &999i128);
    assert_eq!(
        result,
        Err(Ok(ContractError::AlreadyInitialized))
    );
}

#[test]
fn state_unchanged_after_failed_reinit() {
    let (env, _contract_id, client, admin) = setup();

    let new_admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Attempt re-init with a different admin.
    let _ = client.try_initialize(&new_admin, &staking, &treasury, &500i128);

    // Original admin should still be stored.
    assert_eq!(client.get_admin(), admin);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Invalid Input Rejection
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn zero_total_supply_rejected() {
    let (env, _contract_id, client) = setup_uninit();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    let result = client.try_initialize(&admin, &staking, &treasury, &0i128);
    assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
}

#[test]
fn negative_total_supply_rejected() {
    let (env, _contract_id, client) = setup_uninit();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    let result = client.try_initialize(&admin, &staking, &treasury, &(-1i128));
    assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
}

#[test]
fn large_negative_supply_rejected() {
    let (env, _contract_id, client) = setup_uninit();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    let result = client.try_initialize(&admin, &staking, &treasury, &i128::MIN);
    assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
}

#[test]
fn contract_not_initialized_after_invalid_input() {
    let (env, _contract_id, client) = setup_uninit();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Fail with invalid supply.
    let _ = client.try_initialize(&admin, &staking, &treasury, &0i128);

    // Contract should remain uninitialized.
    assert!(!client.is_initialized());
}

#[test]
fn valid_init_succeeds_after_prior_invalid_attempt() {
    let (env, _contract_id, client) = setup_uninit();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    // First attempt: invalid.
    let _ = client.try_initialize(&admin, &staking, &treasury, &0i128);
    assert!(!client.is_initialized());

    // Second attempt: valid.
    client.initialize(&admin, &staking, &treasury, &100i128);
    assert!(client.is_initialized());
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. is_initialized View Function
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn is_initialized_false_before_init() {
    let (_env, _contract_id, client) = setup_uninit();
    assert!(!client.is_initialized());
}

#[test]
fn is_initialized_true_after_init() {
    let (_env, _contract_id, client, _admin) = setup();
    assert!(client.is_initialized());
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Operations Fail Before Initialization (NotInitialized Guard)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn create_proposal_before_init_fails() {
    let (env, _contract_id, client) = setup_uninit();

    let proposer = Address::generate(&env);
    let action = dummy_action(&env);
    let title = String::from_str(&env, "Test Proposal");

    let result = client.try_create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &title,
        &vec![&env, action],
    );
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn advance_phase_before_init_fails() {
    let (env, _contract_id, client) = setup_uninit();

    let caller = Address::generate(&env);
    let result = client.try_advance_phase(&caller, &1u64);
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn commit_vote_before_init_fails() {
    let (env, _contract_id, client) = setup_uninit();

    let voter = Address::generate(&env);
    let commitment = BytesN::from_array(&env, &[0u8; 32]);
    let result = client.try_commit_vote(&voter, &1u64, &commitment);
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn reveal_vote_before_init_fails() {
    let (env, _contract_id, client) = setup_uninit();

    let voter = Address::generate(&env);
    let salt = BytesN::from_array(&env, &[0u8; 32]);
    let result = client.try_reveal_vote(&voter, &1u64, &VoteChoice::For, &salt);
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn delegate_before_init_fails() {
    let (env, _contract_id, client) = setup_uninit();

    let voter = Address::generate(&env);
    let delegate = Address::generate(&env);
    let result = client.try_delegate(&voter, &delegate);
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn revoke_delegation_before_init_fails() {
    let (env, _contract_id, client) = setup_uninit();

    let voter = Address::generate(&env);
    let result = client.try_revoke_delegation(&voter);
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn execute_proposal_before_init_fails() {
    let (env, _contract_id, client) = setup_uninit();

    let caller = Address::generate(&env);
    let result = client.try_execute_proposal(&caller, &1u64);
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn set_total_vote_supply_before_init_fails() {
    let (env, _contract_id, client) = setup_uninit();

    let caller = Address::generate(&env);
    let result = client.try_set_total_vote_supply(&caller, &500i128);
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn get_admin_before_init_fails() {
    let (_env, _contract_id, client) = setup_uninit();

    let result = client.try_get_admin();
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Admin-Gated Functions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn admin_can_set_total_vote_supply() {
    let (_env, _contract_id, client, admin) = setup();

    // Admin should be able to update the supply.
    client.set_total_vote_supply(&admin, &2_000_000i128);
}

#[test]
fn non_admin_cannot_set_total_vote_supply() {
    let (env, _contract_id, client, _admin) = setup();

    let non_admin = Address::generate(&env);
    let result = client.try_set_total_vote_supply(&non_admin, &2_000_000i128);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn admin_cannot_set_zero_supply() {
    let (_env, _contract_id, client, admin) = setup();

    let result = client.try_set_total_vote_supply(&admin, &0i128);
    assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
}

#[test]
fn admin_cannot_set_negative_supply() {
    let (_env, _contract_id, client, admin) = setup();

    let result = client.try_set_total_vote_supply(&admin, &(-100i128));
    assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Post-Initialization State Consistency
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn create_proposal_works_after_init() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 1000);

    let action = dummy_action(&env);
    let title = String::from_str(&env, "Test Proposal");

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &title,
        &vec![&env, action],
    );
    assert_eq!(id, 1);
}

#[test]
fn proposal_requires_stake_after_init() {
    let (env, _contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    // No mock stake set → zero balance.
    let action = dummy_action(&env);
    let title = String::from_str(&env, "No Stake Proposal");

    let result = client.try_create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &title,
        &vec![&env, action],
    );
    assert_eq!(result, Err(Ok(ContractError::InsufficientStake)));
}

#[test]
fn proposal_with_empty_actions_rejected() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 1000);

    let title = String::from_str(&env, "Empty Actions");
    let empty_actions = vec![&env];

    let result = client.try_create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &title,
        &empty_actions,
    );
    assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
}

#[test]
fn vote_power_returns_zero_for_unstaked_user() {
    let (env, _contract_id, client, _admin) = setup();

    let voter = Address::generate(&env);
    let power = client.get_vote_power(&voter);
    assert_eq!(power, 0);
}

#[test]
fn vote_power_returns_correct_value_for_staked_user() {
    let (env, contract_id, client, _admin) = setup();

    let voter = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &voter, 100);
    set_mock_age(&env, &contract_id, &voter, 0);

    // sqrt(100) * 1000 (scale) * 1.0 (loyalty) / 1000 = 10
    let power = client.get_vote_power(&voter);
    assert_eq!(power, 10);
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Delegation Constraints After Init
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn delegation_works_after_init() {
    let (env, _contract_id, client, _admin) = setup();

    let voter = Address::generate(&env);
    let delegate = Address::generate(&env);

    client.delegate(&voter, &delegate);
    let stored = client.get_delegation(&voter);
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().delegate, delegate);
}

#[test]
fn self_delegation_rejected_after_init() {
    let (env, _contract_id, client, _admin) = setup();

    let voter = Address::generate(&env);
    let result = client.try_delegate(&voter, &voter);
    assert_eq!(result, Err(Ok(ContractError::SelfDelegation)));
}

#[test]
fn delegation_count_starts_at_zero() {
    let (env, _contract_id, client, _admin) = setup();

    let delegate = Address::generate(&env);
    assert_eq!(client.get_delegation_count(&delegate), 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Deterministic Initialization — Same Inputs Yield Same State
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn initialization_is_deterministic() {
    // Two contracts initialized with the same admin should have the same admin stored.
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    let id_a = env.register(GovernorContract, ());
    let client_a = GovernorContractClient::new(&env, &id_a);
    client_a.initialize(&admin, &staking, &treasury, &500i128);

    let id_b = env.register(GovernorContract, ());
    let client_b = GovernorContractClient::new(&env, &id_b);
    client_b.initialize(&admin, &staking, &treasury, &500i128);

    assert_eq!(client_a.get_admin(), client_b.get_admin());
    assert!(client_a.is_initialized());
    assert!(client_b.is_initialized());
}
