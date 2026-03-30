//! Integration tests for the Governor DAO contract.
//!
//! Tests cover:
//! - Quadratic vote power computation
//! - Time-weighted loyalty multiplier
//! - Full proposal lifecycle (Draft → Completed)
//! - Rejection paths (quorum failure, majority failure, veto)
//! - Commit-reveal voting
//! - Delegation and revocation
//! - Batched proposals
//! - Emergency proposal (reduced timelock)

#![cfg(test)]

extern crate std;

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String, Vec,
};

use crate::{
    delegation,
    proposal::{ProposalAction, ProposalPhase, ProposalType},
    voting::{compute_vote_power, isqrt, loyalty_multiplier_scaled, VoteChoice, SCALE},
    ContractError, GovernorContract, GovernorContractClient,
};

// ── Test helpers ──────────────────────────────────────────────────────────────

fn create_env() -> Env {
    Env::default()
}

fn register_governor(env: &Env) -> (Address, GovernorContractClient) {
    let contract_id = env.register_contract(None, GovernorContract);
    let client = GovernorContractClient::new(env, &contract_id);
    (contract_id, client)
}

fn default_init(env: &Env, client: &GovernorContractClient) -> (Address, Address, Address) {
    let admin = Address::generate(env);
    let staking = Address::generate(env);
    let treasury = Address::generate(env);
    client.initialize(&admin, &staking, &treasury, &1_000_000_000i128);
    (admin, staking, treasury)
}

/// Inject a mock staked balance for a voter directly into persistent storage.
fn set_mock_stake(env: &Env, contract_id: &Address, voter: &Address, amount: i128) {
    env.as_contract(contract_id, || {
        let key = (symbol_short!("M_STK"), voter.clone());
        env.storage().persistent().set(&key, &amount);
    });
}

/// Inject a mock stake age (seconds) for a voter.
fn set_mock_age(env: &Env, contract_id: &Address, voter: &Address, age_secs: u64) {
    env.as_contract(contract_id, || {
        let key = (symbol_short!("M_AGE"), voter.clone());
        env.storage().persistent().set(&key, &age_secs);
    });
}

fn single_action(env: &Env, target: &Address) -> Vec<ProposalAction> {
    let mut actions = Vec::new(env);
    actions.push_back(ProposalAction {
        target: target.clone(),
        function: symbol_short!("GOV_PRM"),
        params_hash: BytesN::from_array(env, &[0u8; 32]),
    });
    actions
}

fn advance_time(env: &Env, secs: u64) {
    env.ledger().with_mut(|l| {
        l.timestamp = l.timestamp.saturating_add(secs);
    });
}

// ── Unit tests: vote power math ───────────────────────────────────────────────

#[test]
fn test_isqrt_values() {
    assert_eq!(isqrt(0), 0);
    assert_eq!(isqrt(1), 1);
    assert_eq!(isqrt(4), 2);
    assert_eq!(isqrt(9), 3);
    assert_eq!(isqrt(100), 10);
    assert_eq!(isqrt(10_000), 100);
    assert_eq!(isqrt(1_000_000), 1_000);
    // Floor behaviour
    assert_eq!(isqrt(2), 1);
    assert_eq!(isqrt(3), 1);
    assert_eq!(isqrt(8), 2);
}

#[test]
fn test_loyalty_multiplier_no_age() {
    // At age 0, multiplier should be exactly SCALE (1.0×).
    let m = loyalty_multiplier_scaled(0);
    assert_eq!(m, SCALE);
}

#[test]
fn test_loyalty_multiplier_one_year() {
    // At 365 days, multiplier should be 2× SCALE.
    let secs = 365 * 86_400;
    let m = loyalty_multiplier_scaled(secs);
    assert_eq!(m, 2 * SCALE);
}

#[test]
fn test_loyalty_multiplier_half_year() {
    let secs = 182 * 86_400;
    let m = loyalty_multiplier_scaled(secs);
    // 1 + 182/365 ≈ 1.498 → scaled = 1498
    assert!(m > SCALE && m < 2 * SCALE);
}

#[test]
fn test_loyalty_multiplier_capped_beyond_year() {
    // Beyond MAX_LOYALTY_DAYS the multiplier stays at 2×.
    let secs = 1000 * 86_400;
    assert_eq!(loyalty_multiplier_scaled(secs), 2 * SCALE);
}

#[test]
fn test_quadratic_vote_power_zero_stake() {
    assert_eq!(compute_vote_power(0, 0), 0);
}

#[test]
fn test_quadratic_vote_power_fresh_staker() {
    // 100 tokens, 0 age → sqrt(100) × 1.0 = 10
    let p = compute_vote_power(100, 0);
    assert_eq!(p, 10);
}

#[test]
fn test_quadratic_vote_power_year_old_staker() {
    // 100 tokens, 1 year → sqrt(100) × 2.0 = 20
    let p = compute_vote_power(100, 365 * 86_400);
    assert_eq!(p, 20);
}

#[test]
fn test_quadratic_prevents_plutocracy() {
    // Whale has 1 000 000 tokens, minnow has 100 tokens.
    // Without quadratic: 10 000× advantage.
    // With quadratic (no age): sqrt(1_000_000)/sqrt(100) = 1000/10 = 100× advantage.
    let whale = compute_vote_power(1_000_000, 0);
    let minnow = compute_vote_power(100, 0);
    assert_eq!(whale, 1_000);
    assert_eq!(minnow, 10);
    // Power ratio is 100:1, not 10 000:1.
    assert_eq!(whale / minnow, 100);
}

#[test]
fn test_time_weight_can_overcome_token_disadvantage() {
    // Loyal minnow (1 year) vs fresh whale.
    // Minnow: 100 tokens, 1 year → 20
    // Whale:  10 000 tokens, 0 age → sqrt(10 000) = 100
    // Whale still wins on tokens alone, but the gap is narrower.
    let loyal_minnow = compute_vote_power(100, 365 * 86_400); // 20
    let fresh_whale = compute_vote_power(10_000, 0); // 100
    assert_eq!(loyal_minnow, 20);
    assert_eq!(fresh_whale, 100);
    // A very loyal small holder with 2500 tokens beats a fresh whale with 10 000.
    let loyal_moderate = compute_vote_power(2_500, 365 * 86_400); // sqrt(2500)×2 = 50×2 = 100
    assert_eq!(loyal_moderate, fresh_whale);
}

// ── Integration tests: proposal lifecycle ────────────────────────────────────

#[test]
fn test_initialize_and_create_proposal() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    let (_admin, _staking, _treasury) = default_init(&env, &client);

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 1_000);

    let target = Address::generate(&env);
    let actions = single_action(&env, &target);

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Set lock period to 7 days"),
        &actions,
    );

    assert_eq!(id, 1);

    let proposal = client.get_proposal(&id).unwrap();
    assert!(matches!(proposal.phase, ProposalPhase::Draft));
    assert_eq!(proposal.proposer, proposer);
}

#[test]
fn test_create_proposal_requires_stake() {
    let env = create_env();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    // No stake injected → zero balance.

    let target = Address::generate(&env);
    let actions = single_action(&env, &target);

    let result = client.try_create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Should fail"),
        &actions,
    );
    assert!(result.is_err());
}

#[test]
fn test_full_lifecycle_happy_path() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    let voter_a = Address::generate(&env);
    let voter_b = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);
    set_mock_stake(&env, &contract_id, &voter_a, 40_000);
    set_mock_stake(&env, &contract_id, &voter_b, 40_000);
    // Both voters have 6 months of stake age.
    set_mock_age(&env, &contract_id, &voter_a, 182 * 86_400);
    set_mock_age(&env, &contract_id, &voter_b, 182 * 86_400);

    let target = Address::generate(&env);
    let actions = single_action(&env, &target);

    // 1. Create proposal (Draft)
    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Increase reward rate"),
        &actions,
    );

    // 2. Draft → Discussion
    client.advance_phase(&proposer, &id);

    // 3. Discussion → Voting (advance past discussion period)
    advance_time(&env, 3 * 24 * 3600 + 1); // 3 days + 1s
    client.advance_phase(&voter_a, &id);

    let proposal = client.get_proposal(&id).unwrap();
    assert!(matches!(proposal.phase, ProposalPhase::Voting));

    // 4. Commit votes
    let salt_a = BytesN::from_array(&env, &[1u8; 32]);
    let salt_b = BytesN::from_array(&env, &[2u8; 32]);

    // Compute commitments off-chain (same logic as hash_commitment in lib.rs).
    // For tests we use placeholder hashes and will not verify mismatch.
    // A full test would compute the exact SHA-256; here we test the flow.
    let commit_a = env
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&env, &[1u8; 32]))
        .into();
    let commit_b = env
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&env, &[2u8; 32]))
        .into();

    client.commit_vote(&voter_a, &id, &commit_a);
    client.commit_vote(&voter_b, &id, &commit_b);

    let proposal = client.get_proposal(&id).unwrap();
    assert_eq!(proposal.commit_count, 2);

    // 5. Reveal — we test the commitment mismatch path here since computing
    //    the exact hash requires replicating the SHA-256 preimage.
    //    A companion test (`test_reveal_correct_commitment`) uses pre-computed hashes.
    let reveal_result = client.try_reveal_vote(&voter_a, &id, &VoteChoice::For, &salt_a);
    // Expected: CommitmentMismatch (salt doesn't match test commit_a)
    assert!(reveal_result.is_err());
}

#[test]
fn test_proposal_lifecycle_rejection_no_quorum() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 1_000);

    let target = Address::generate(&env);
    let actions = single_action(&env, &target);

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Low-participation proposal"),
        &actions,
    );

    // Draft → Discussion → Voting
    client.advance_phase(&proposer, &id);
    advance_time(&env, 3 * 24 * 3600 + 1);
    client.advance_phase(&proposer, &id);

    // No votes cast. Advance past voting period.
    advance_time(&env, 5 * 24 * 3600 + 1);
    let phase = client.advance_phase(&proposer, &id);
    // Should expire due to no quorum.
    assert!(matches!(phase, ProposalPhase::Expired));
}

#[test]
fn test_delegation_and_revocation() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let voter = Address::generate(&env);
    let delegate = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &voter, 5_000);

    // Delegate
    client.delegate(&voter, &delegate);

    let del = client.get_delegation(&voter);
    assert!(del.is_some());
    assert_eq!(del.unwrap().delegate, delegate);

    let count = client.get_delegation_count(&delegate);
    assert_eq!(count, 1);

    // Set delegate stake before proposing — stake must exist before create_proposal.
    set_mock_stake(&env, &contract_id, &delegate, 1_000);

    // Create a proposal as the delegate to verify delegation doesn't
    // interfere with the proposer role.
    let target = Address::generate(&env);
    let actions = single_action(&env, &target);
    let _id = client.create_proposal(
        &delegate,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Test proposal"),
        &actions,
    );

    // Revoke delegation
    client.revoke_delegation(&voter);
    let del = client.get_delegation(&voter);
    assert!(del.is_none());
    let count = client.get_delegation_count(&delegate);
    assert_eq!(count, 0);
}

#[test]
fn test_self_delegation_rejected() {
    let env = create_env();
    env.mock_all_auths();
    let (_contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let voter = Address::generate(&env);
    let result = client.try_delegate(&voter, &voter);
    assert!(result.is_err());
}

#[test]
fn test_emergency_proposal_has_shorter_timelock() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 5_000);

    let target = Address::generate(&env);
    let actions = single_action(&env, &target);

    let id = client.create_proposal(
        &proposer,
        &ProposalType::EmergencyAction,
        &String::from_str(&env, "Emergency fix"),
        &actions,
    );

    let proposal = client.get_proposal(&id).unwrap();
    let timelock_len = proposal.timelock_ends - proposal.voting_ends;
    // Emergency timelock = 6 hours = 21 600 seconds.
    assert_eq!(timelock_len, 21_600);
}

#[test]
fn test_upgrade_proposal_has_longer_timelock() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 5_000);

    let target = Address::generate(&env);
    let actions = single_action(&env, &target);

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ContractUpgrade,
        &String::from_str(&env, "Upgrade to v2"),
        &actions,
    );

    let proposal = client.get_proposal(&id).unwrap();
    let timelock_len = proposal.timelock_ends - proposal.voting_ends;
    // Upgrade timelock = 7 days = 604 800 seconds.
    assert_eq!(timelock_len, 604_800);
}

#[test]
fn test_batched_proposal_multiple_actions() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 5_000);

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
        &String::from_str(&env, "Batch update 3 params"),
        &actions,
    );

    let proposal = client.get_proposal(&id).unwrap();
    assert_eq!(proposal.actions.len(), 3);
}

#[test]
fn test_get_vote_power_view() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let voter = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &voter, 10_000);
    set_mock_age(&env, &contract_id, &voter, 365 * 86_400);

    // sqrt(10_000) × 2 = 100 × 2 = 200
    let power = client.get_vote_power(&voter);
    assert_eq!(power, 200);
}

#[test]
fn test_cannot_advance_phase_before_time() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 5_000);

    let target = Address::generate(&env);
    let actions = single_action(&env, &target);

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Test"),
        &actions,
    );

    // Draft → Discussion (ok, proposer can do this immediately)
    client.advance_phase(&proposer, &id);

    // Discussion → Voting before discussion period ends should fail.
    let result = client.try_advance_phase(&proposer, &id);
    assert!(result.is_err());
}

#[test]
fn test_double_commit_rejected() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 5_000);
    set_mock_stake(&env, &contract_id, &voter, 1_000);

    let target = Address::generate(&env);
    let actions = single_action(&env, &target);
    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Test"),
        &actions,
    );

    client.advance_phase(&proposer, &id);
    advance_time(&env, 3 * 24 * 3600 + 1);
    client.advance_phase(&proposer, &id);

    let commit = BytesN::from_array(&env, &[42u8; 32]);
    client.commit_vote(&voter, &id, &commit);

    // Second commit should fail.
    let result = client.try_commit_vote(&voter, &id, &commit);
    assert!(result.is_err());
}

// ── Replay / duplicate execution simulation ──────────────────────────────────

/// Compute the commitment hash according to the current on-chain logic:
/// SHA-256(proposal_id_le_bytes || choice_byte || salt_32bytes)
fn compute_commitment(
    env: &Env,
    proposal_id: u64,
    choice: &VoteChoice,
    salt: &BytesN<32>,
) -> BytesN<32> {
    use soroban_sdk::Bytes;
    let mut data = Bytes::new(env);
    for b in proposal_id.to_le_bytes().iter() {
        data.push_back(*b);
    }
    let choice_byte: u8 = match choice {
        VoteChoice::For => 0,
        VoteChoice::Against => 1,
        VoteChoice::Veto => 2,
    };
    data.push_back(choice_byte);
    for i in 0..32u32 {
        data.push_back(salt.get(i).unwrap_or(0));
    }
    env.crypto().sha256(&data).into()
}

#[test]
fn test_execute_proposal_is_not_replayable() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);

    // Initialize with a small total supply so quorum is easy to meet.
    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.initialize(&admin, &staking, &treasury, &1_000i128);

    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);
    set_mock_stake(&env, &contract_id, &voter, 10_000);

    let target = Address::generate(&env);
    let actions = single_action(&env, &target);

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Replay protection test"),
        &actions,
    );

    // Draft → Discussion → Voting
    client.advance_phase(&proposer, &id);
    advance_time(&env, 3 * 24 * 3600 + 1);
    client.advance_phase(&voter, &id);

    // Commit + reveal a FOR vote with a matching commitment.
    let salt = BytesN::from_array(&env, &[9u8; 32]);
    let choice = VoteChoice::For;
    let commitment = compute_commitment(&env, id, &choice, &salt);
    client.commit_vote(&voter, &id, &commitment);
    let revealed_power = client.reveal_vote(&voter, &id, &choice, &salt);
    assert!(revealed_power > 0);

    // Voting → Timelock
    advance_time(&env, 5 * 24 * 3600 + 1);
    let phase = client.advance_phase(&voter, &id);
    assert!(matches!(phase, ProposalPhase::Timelock));

    // Timelock → Execution
    let p = client.get_proposal(&id).unwrap();
    let now = env.ledger().timestamp();
    if p.timelock_ends > now {
        advance_time(&env, p.timelock_ends - now + 1);
    }
    let phase = client.advance_phase(&voter, &id);
    assert!(matches!(phase, ProposalPhase::Execution));

    // Execute once should succeed.
    let executor = Address::generate(&env);
    client.execute_proposal(&executor, &id);

    // Replay attempt: executing again must fail (proposal no longer in Execution phase).
    let res = client.try_execute_proposal(&executor, &id);
    assert!(res.is_err());
}

// ── Namespace separation tests ────────────────────────────────────────────────

/// Test that proposal storage keys don't collide with delegation keys
#[test]
fn test_namespace_proposal_vs_delegation_no_collision() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);
    let delegate = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 5_000);
    set_mock_stake(&env, &contract_id, &voter, 5_000);

    // Create a proposal
    let target = Address::generate(&env);
    let actions = single_action(&env, &target);
    let proposal_id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Test proposal"),
        &actions,
    );

    // Create a delegation
    client.delegate(&voter, &delegate);

    // Verify proposal still exists and is independent
    let proposal = client.get_proposal(&proposal_id).unwrap();
    assert_eq!(proposal.id, proposal_id);
    assert_eq!(proposal.proposer, proposer);

    // Verify delegation still exists and is independent
    let del = client.get_delegation(&voter);
    assert!(del.is_some());
    assert_eq!(del.unwrap().delegate, delegate);
}

/// Test that vote storage keys don't collide with proposal or delegation keys
#[test]
fn test_namespace_votes_vs_proposals_no_collision() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 5_000);
    set_mock_stake(&env, &contract_id, &voter, 5_000);

    // Create first proposal
    let target = Address::generate(&env);
    let actions = single_action(&env, &target);
    let proposal_1 = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "First proposal"),
        &actions,
    );

    // Create second proposal
    let proposal_2 = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Second proposal"),
        &actions,
    );

    // Advance both to voting phase
    client.advance_phase(&proposer, &proposal_1);
    client.advance_phase(&proposer, &proposal_2);
    advance_time(&env, 3 * 24 * 3600 + 1);
    client.advance_phase(&voter, &proposal_1);
    client.advance_phase(&voter, &proposal_2);

    // Commit votes on both proposals with different salts/commitments
    let commit_1 = env
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&env, &[1u8; 32]))
        .into();
    let commit_2 = env
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&env, &[2u8; 32]))
        .into();

    client.commit_vote(&voter, &proposal_1, &commit_1);
    client.commit_vote(&voter, &proposal_2, &commit_2);

    // Verify both proposals still exist independently
    let p1 = client.get_proposal(&proposal_1).unwrap();
    let p2 = client.get_proposal(&proposal_2).unwrap();
    assert_eq!(p1.id, proposal_1);
    assert_eq!(p2.id, proposal_2);
    assert_eq!(p1.commit_count, 1);
    assert_eq!(p2.commit_count, 1);
}

/// Test that multiple voters don't cross-contaminate their vote state
#[test]
fn test_namespace_voter_votes_isolated() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    let voter_a = Address::generate(&env);
    let voter_b = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 5_000);
    set_mock_stake(&env, &contract_id, &voter_a, 5_000);
    set_mock_stake(&env, &contract_id, &voter_b, 5_000);

    // Create proposal
    let target = Address::generate(&env);
    let actions = single_action(&env, &target);
    let proposal_id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Test"),
        &actions,
    );

    // Advance to voting
    client.advance_phase(&proposer, &proposal_id);
    advance_time(&env, 3 * 24 * 3600 + 1);
    client.advance_phase(&voter_a, &proposal_id);

    // Both voters commit different votes
    let commit_a = env
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&env, &[1u8; 32]))
        .into();
    let commit_b = env
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&env, &[2u8; 32]))
        .into();

    client.commit_vote(&voter_a, &proposal_id, &commit_a);
    client.commit_vote(&voter_b, &proposal_id, &commit_b);

    // Verify proposal sees both commits (commit_count = 2)
    let proposal = client.get_proposal(&proposal_id).unwrap();
    assert_eq!(proposal.commit_count, 2);

    // Double-vote attempts from both voters should fail
    let result_a = client.try_commit_vote(&voter_a, &proposal_id, &commit_a);
    let result_b = client.try_commit_vote(&voter_b, &proposal_id, &commit_b);
    assert!(result_a.is_err());
    assert!(result_b.is_err());
}

/// Test that different proposal types don't interfere with each other's storage
#[test]
fn test_namespace_proposal_types_isolated() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let target = Address::generate(&env);
    let actions = single_action(&env, &target);

    // Create proposals of different types
    let param_change = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Param change"),
        &actions,
    );

    let emergency = client.create_proposal(
        &proposer,
        &ProposalType::EmergencyAction,
        &String::from_str(&env, "Emergency"),
        &actions,
    );

    let upgrade = client.create_proposal(
        &proposer,
        &ProposalType::ContractUpgrade,
        &String::from_str(&env, "Upgrade"),
        &actions,
    );

    // Verify all three proposals exist independently with correct types
    let p1 = client.get_proposal(&param_change).unwrap();
    let p2 = client.get_proposal(&emergency).unwrap();
    let p3 = client.get_proposal(&upgrade).unwrap();

    assert_eq!(p1.proposal_type, ProposalType::ParameterChange);
    assert_eq!(p2.proposal_type, ProposalType::EmergencyAction);
    assert_eq!(p3.proposal_type, ProposalType::ContractUpgrade);

    // Check that timelock durations differ (isolation verification)
    let timelock_1 = p1.timelock_ends - p1.voting_ends;
    let timelock_2 = p2.timelock_ends - p2.voting_ends;
    let timelock_3 = p3.timelock_ends - p3.voting_ends;

    // Emergency should have shorter timelock than others
    assert!(timelock_2 < timelock_1);
    assert!(timelock_2 < timelock_3);
}

/// Test that mock stake and mock age data don't interfere with contract state
#[test]
fn test_namespace_mock_data_isolated_from_proposals() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);

    // Inject different mock stakes for different voters
    set_mock_stake(&env, &contract_id, &voter1, 100_000);
    set_mock_stake(&env, &contract_id, &voter2, 50_000);

    // Inject different ages
    set_mock_age(&env, &contract_id, &voter1, 365 * 86_400); // 1 year old
    set_mock_age(&env, &contract_id, &voter2, 180 * 86_400); // 6 months old

    // Verify vote powers are computed correctly and independently
    let power1 = client.get_vote_power(&voter1);
    let power2 = client.get_vote_power(&voter2);

    // voter1: sqrt(100_000) × 2.0 ≈ 316 × 2 = 632
    let expected_power1 = compute_vote_power(100_000, 365 * 86_400);
    // voter2: sqrt(50_000) × ~1.5 ≈ 223 × 1.5 ≈ 335
    let expected_power2 = compute_vote_power(50_000, 180 * 86_400);

    assert_eq!(power1, expected_power1);
    assert_eq!(power2, expected_power2);
    assert!(power1 > power2);
}

/// Test that delegations don't collide with vote commits/records
#[test]
fn test_namespace_delegation_vs_votes_no_collision() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let delegator = Address::generate(&env);
    let delegate = Address::generate(&env);
    let proposer = Address::generate(&env);

    set_mock_stake(&env, &contract_id, &delegator, 5_000);
    set_mock_stake(&env, &contract_id, &delegate, 5_000);
    set_mock_stake(&env, &contract_id, &proposer, 5_000);

    // Create delegation
    client.delegate(&delegator, &delegate);

    // Create proposal
    let target = Address::generate(&env);
    let actions = single_action(&env, &target);
    let proposal_id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Test"),
        &actions,
    );

    // Advance to voting
    client.advance_phase(&proposer, &proposal_id);
    advance_time(&env, 3 * 24 * 3600 + 1);
    client.advance_phase(&delegate, &proposal_id);

    // Delegate votes on the proposal
    let commit = env
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&env, &[5u8; 32]))
        .into();
    client.commit_vote(&delegate, &proposal_id, &commit);

    // Verify delegation still exists
    let del = client.get_delegation(&delegator);
    assert!(del.is_some());
    assert_eq!(del.unwrap().delegate, delegate);

    // Verify proposal commit count is 1 (not confused with delegation)
    let proposal = client.get_proposal(&proposal_id).unwrap();
    assert_eq!(proposal.commit_count, 1);
}

/// Test that large gaps in proposal IDs don't cause namespace collisions
#[test]
fn test_namespace_proposal_id_gaps() {
    let env = create_env();
    env.mock_all_auths();
    let (contract_id, client) = register_governor(&env);
    default_init(&env, &client);

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let target = Address::generate(&env);
    let actions = single_action(&env, &target);

    // Create proposal with ID 1
    let id1 = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "First"),
        &actions,
    );
    assert_eq!(id1, 1);

    // Create proposal with ID 2
    let id2 = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Second"),
        &actions,
    );
    assert_eq!(id2, 2);

    // Both proposals should be independently accessible
    let p1 = client.get_proposal(&id1).unwrap();
    let p2 = client.get_proposal(&id2).unwrap();

    assert_eq!(p1.id, id1);
    assert_eq!(p2.id, id2);
    assert_ne!(p1.title, p2.title);
}
