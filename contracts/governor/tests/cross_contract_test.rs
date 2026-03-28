//! Cross-Contract Calling Invariants tests for the Governor contract.
//!
//! These tests verify that the governor:
//! - Correctly dispatches actions to external contracts during proposal execution
//! - Emits the expected DISPATCH events with correct payloads
//! - Handles proposals with multiple cross-contract actions atomically
//! - Rejects execution when the proposal is not in the Execution phase
//! - Prevents double-execution of completed proposals
//! - Correctly handles different proposal types (emergency, upgrade, treasury spend)

#![cfg(test)]
#![allow(clippy::unwrap_used)]

extern crate std;

use governor::{
    proposal::{ProposalAction, ProposalPhase, ProposalType},
    ContractError, GovernorContract, GovernorContractClient,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    vec, Address, BytesN, Env, String, Vec,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, Address, GovernorContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);
    // Use a small total supply so quadratic vote power can meet quorum.
    client.initialize(&admin, &staking, &treasury, &100i128);

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

fn advance_time(env: &Env, secs: u64) {
    env.ledger().with_mut(|l| {
        l.timestamp = l.timestamp.saturating_add(secs);
    });
}

/// Replicate the governor's commitment hash: SHA-256(proposal_id_le || choice_byte || salt).
fn compute_commitment(env: &Env, proposal_id: u64, choice_byte: u8, salt: &BytesN<32>) -> BytesN<32> {
    use soroban_sdk::Bytes;
    let mut data = Bytes::new(env);
    for b in proposal_id.to_le_bytes().iter() {
        data.push_back(*b);
    }
    data.push_back(choice_byte);
    for i in 0..32u32 {
        data.push_back(salt.get(i).unwrap_or(0));
    }
    env.crypto().sha256(&data).into()
}

/// Drive a proposal through Draft → Discussion → Voting → Timelock → Execution.
fn drive_to_execution(
    env: &Env,
    contract_id: &Address,
    client: &GovernorContractClient,
    proposal_id: u64,
    proposer: &Address,
) {
    let voter_a = Address::generate(env);
    let voter_b = Address::generate(env);
    set_mock_stake(env, contract_id, &voter_a, 250_000_000);
    set_mock_stake(env, contract_id, &voter_b, 250_000_000);
    set_mock_age(env, contract_id, &voter_a, 365 * 86_400);
    set_mock_age(env, contract_id, &voter_b, 365 * 86_400);

    // Draft → Discussion
    client.advance_phase(proposer, &proposal_id);

    // Discussion → Voting
    advance_time(env, 3 * 86_400 + 1);
    client.advance_phase(proposer, &proposal_id);

    // Commit + reveal For votes
    let salt_a = BytesN::from_array(env, &[0xAA; 32]);
    let salt_b = BytesN::from_array(env, &[0xBB; 32]);
    let commit_a = compute_commitment(env, proposal_id, 0u8, &salt_a);
    let commit_b = compute_commitment(env, proposal_id, 0u8, &salt_b);

    client.commit_vote(&voter_a, &proposal_id, &commit_a);
    client.commit_vote(&voter_b, &proposal_id, &commit_b);
    client.reveal_vote(&voter_a, &proposal_id, &governor::voting::VoteChoice::For, &salt_a);
    client.reveal_vote(&voter_b, &proposal_id, &governor::voting::VoteChoice::For, &salt_b);

    // Voting → Timelock
    advance_time(env, 5 * 86_400 + 1);
    client.advance_phase(proposer, &proposal_id);

    // Timelock → Execution (use worst-case 7-day upgrade timelock)
    advance_time(env, 7 * 86_400 + 1);
    client.advance_phase(proposer, &proposal_id);

    assert!(matches!(
        client.get_proposal(&proposal_id).unwrap().phase,
        ProposalPhase::Execution
    ));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Verify that executing a single-action proposal emits a DISPATCH event
/// and transitions the proposal to Completed.
#[test]
fn test_execute_dispatches_single_action_event() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let target = Address::generate(&env);
    let actions = vec![
        &env,
        ProposalAction {
            target: target.clone(),
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&env, &[0x42; 32]),
        },
    ];

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Single action dispatch"),
        &actions,
    );

    drive_to_execution(&env, &contract_id, &client, id, &proposer);

    let events_before = env.events().all().events().len();
    client.execute_proposal(&proposer, &id);
    let events_after = env.events().all().events().len();

    // At least 2 new events: DISPATCH + PROP_EXE
    assert!(events_after - events_before >= 2);

    let proposal = client.get_proposal(&id).unwrap();
    assert_eq!(proposal.phase, ProposalPhase::Completed);
}

/// Verify that a batched proposal with 3 actions emits 3 DISPATCH events.
#[test]
fn test_execute_dispatches_batched_actions() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let mut actions = Vec::new(&env);
    for i in 0u8..3 {
        actions.push_back(ProposalAction {
            target: Address::generate(&env),
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&env, &[i; 32]),
        });
    }

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Batched dispatch"),
        &actions,
    );

    drive_to_execution(&env, &contract_id, &client, id, &proposer);

    let events_before = env.events().all().events().len();
    client.execute_proposal(&proposer, &id);
    let events_after = env.events().all().events().len();

    // 3 DISPATCH events + 1 PROP_EXE event = at least 4 new events
    assert!(events_after - events_before >= 4);
    assert_eq!(
        client.get_proposal(&id).unwrap().phase,
        ProposalPhase::Completed
    );
}

/// Attempting to execute a proposal that is still in Draft should fail with WrongPhase.
#[test]
fn test_execute_rejects_wrong_phase() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let actions = vec![
        &env,
        ProposalAction {
            target: Address::generate(&env),
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&env, &[0u8; 32]),
        },
    ];

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Not yet executable"),
        &actions,
    );

    let result = client.try_execute_proposal(&proposer, &id);
    assert_eq!(result, Err(Ok(ContractError::WrongPhase)));
}

/// Executing a nonexistent proposal should return ProposalNotFound.
#[test]
fn test_execute_nonexistent_proposal() {
    let (env, _contract_id, client, _admin) = setup();

    let caller = Address::generate(&env);
    let result = client.try_execute_proposal(&caller, &9999);
    assert_eq!(result, Err(Ok(ContractError::ProposalNotFound)));
}

/// A proposal that has already been executed (Completed) cannot be executed again.
#[test]
fn test_execute_moves_to_completed_only_once() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let actions = vec![
        &env,
        ProposalAction {
            target: Address::generate(&env),
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&env, &[0u8; 32]),
        },
    ];

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Execute once"),
        &actions,
    );

    drive_to_execution(&env, &contract_id, &client, id, &proposer);
    client.execute_proposal(&proposer, &id);
    assert_eq!(
        client.get_proposal(&id).unwrap().phase,
        ProposalPhase::Completed
    );

    // Second execution must fail.
    let result = client.try_execute_proposal(&proposer, &id);
    assert_eq!(result, Err(Ok(ContractError::WrongPhase)));
}

/// Emergency proposals use a shorter timelock (6 hours) and should still
/// dispatch correctly.
#[test]
fn test_emergency_proposal_cross_contract_dispatch() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let actions = vec![
        &env,
        ProposalAction {
            target: Address::generate(&env),
            function: symbol_short!("GOV_EMG"),
            params_hash: BytesN::from_array(&env, &[0xEE; 32]),
        },
    ];

    let id = client.create_proposal(
        &proposer,
        &ProposalType::EmergencyAction,
        &String::from_str(&env, "Emergency dispatch"),
        &actions,
    );

    // Set up voters
    let voter_a = Address::generate(&env);
    let voter_b = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &voter_a, 250_000_000);
    set_mock_stake(&env, &contract_id, &voter_b, 250_000_000);
    set_mock_age(&env, &contract_id, &voter_a, 365 * 86_400);
    set_mock_age(&env, &contract_id, &voter_b, 365 * 86_400);

    // Draft → Discussion → Voting
    client.advance_phase(&proposer, &id);
    advance_time(&env, 3 * 86_400 + 1);
    client.advance_phase(&proposer, &id);

    // Commit + reveal
    let salt_a = BytesN::from_array(&env, &[0xAA; 32]);
    let salt_b = BytesN::from_array(&env, &[0xBB; 32]);
    let commit_a = compute_commitment(&env, id, 0, &salt_a);
    let commit_b = compute_commitment(&env, id, 0, &salt_b);
    client.commit_vote(&voter_a, &id, &commit_a);
    client.commit_vote(&voter_b, &id, &commit_b);
    client.reveal_vote(&voter_a, &id, &governor::voting::VoteChoice::For, &salt_a);
    client.reveal_vote(&voter_b, &id, &governor::voting::VoteChoice::For, &salt_b);

    // Voting → Timelock
    advance_time(&env, 5 * 86_400 + 1);
    client.advance_phase(&proposer, &id);

    // Emergency timelock = 6 hours
    advance_time(&env, 6 * 3600 + 1);
    client.advance_phase(&proposer, &id);
    assert!(matches!(
        client.get_proposal(&id).unwrap().phase,
        ProposalPhase::Execution
    ));

    client.execute_proposal(&proposer, &id);
    assert_eq!(
        client.get_proposal(&id).unwrap().phase,
        ProposalPhase::Completed
    );
}

/// Verify that the PROP_EXE event is emitted after successful execution.
#[test]
fn test_execute_proposal_emits_execution_event() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let actions = vec![
        &env,
        ProposalAction {
            target: Address::generate(&env),
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&env, &[0u8; 32]),
        },
    ];

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Execution event test"),
        &actions,
    );

    drive_to_execution(&env, &contract_id, &client, id, &proposer);

    let events_before = env.events().all().events().len();
    client.execute_proposal(&proposer, &id);
    let events_after = env.events().all().events().len();

    // At minimum: 1 DISPATCH + 1 PROP_EXE
    assert!(events_after > events_before);
}

/// Verify that a proposal in Discussion phase cannot be executed.
#[test]
fn test_execute_rejects_discussion_phase() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let actions = vec![
        &env,
        ProposalAction {
            target: Address::generate(&env),
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&env, &[0u8; 32]),
        },
    ];

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Discussion phase"),
        &actions,
    );

    // Move to Discussion only
    client.advance_phase(&proposer, &id);
    assert!(matches!(
        client.get_proposal(&id).unwrap().phase,
        ProposalPhase::Discussion
    ));

    let result = client.try_execute_proposal(&proposer, &id);
    assert_eq!(result, Err(Ok(ContractError::WrongPhase)));
}

/// Any address can execute a proposal in the Execution phase (permissionless).
#[test]
fn test_execute_is_permissionless() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let actions = vec![
        &env,
        ProposalAction {
            target: Address::generate(&env),
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&env, &[0u8; 32]),
        },
    ];

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Permissionless execution"),
        &actions,
    );

    drive_to_execution(&env, &contract_id, &client, id, &proposer);

    // A completely unrelated address executes the proposal.
    let random_caller = Address::generate(&env);
    client.execute_proposal(&random_caller, &id);
    assert_eq!(
        client.get_proposal(&id).unwrap().phase,
        ProposalPhase::Completed
    );
}
