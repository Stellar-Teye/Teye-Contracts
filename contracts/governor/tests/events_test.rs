#![cfg(test)]

extern crate std;

use governor::{
    proposal::{ProposalAction, ProposalPhase, ProposalType},
    voting::VoteChoice,
    GovernorContract, GovernorContractClient,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    vec, Address, BytesN, Env, IntoVal, String, Vec,
};

fn setup() -> (
    Env,
    Address,
    GovernorContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.initialize(&admin, &staking, &treasury, &100i128);

    (env, contract_id, client, admin, staking, treasury)
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
fn compute_commitment(
    env: &Env,
    proposal_id: u64,
    choice_byte: u8,
    salt: &BytesN<32>,
) -> BytesN<32> {
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

#[test]
fn test_proposal_created_event() {
    let (env, contract_id, client, _admin, _staking, _treasury) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let target = Address::generate(&env);
    let actions = vec![
        &env,
        ProposalAction {
            target: target.clone(),
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&env, &[0u8; 32]),
        },
    ];

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Create event"),
        &actions,
    );

    let binding = env.events().all();
    let all = binding.events();
    // Find the PROP_NEW event: topic (PROP_NEW, id) and payload (proposer, type, title)
    let mut found = false;
    for i in 0..all.len() {
        let e = all.get(i).unwrap();
        let debug = format!("{:?}", e);
        if debug.contains("PROP_NEW") && debug.contains("Create event") {
            found = true;
            break;
        }
    }
    assert!(found, "PROP_NEW event not found");
}

#[test]
fn test_phase_transition_event() {
    let (env, contract_id, client, _admin, _staking, _treasury) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let target = Address::generate(&env);
    let actions = vec![
        &env,
        ProposalAction {
            target: target.clone(),
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&env, &[0u8; 32]),
        },
    ];

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Phase event"),
        &actions,
    );

    // Advance Draft -> Discussion
    client.advance_phase(&proposer, &id);

    let binding = env.events().all();
    let all = binding.events();
    let mut found = false;
    for i in 0..all.len() {
        let e = all.get(i).unwrap();
        let debug = format!("{:?}", e);
        if debug.contains("PROP_PHS") && debug.contains("Discussion") {
            found = true;
            break;
        }
    }
    assert!(found, "PROP_PHS event not found");
}

#[test]
fn test_vote_commit_and_reveal_events() {
    let (env, contract_id, client, _admin, _staking, _treasury) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let target = Address::generate(&env);
    let actions = vec![
        &env,
        ProposalAction {
            target: target.clone(),
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&env, &[0u8; 32]),
        },
    ];

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Vote events"),
        &actions,
    );

    // Move to Voting phase
    client.advance_phase(&proposer, &id);
    advance_time(&env, 3 * 86_400 + 1);
    client.advance_phase(&proposer, &id);

    // Setup voter
    let voter = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &voter, 100_000);
    set_mock_age(&env, &contract_id, &voter, 365 * 86_400);

    let salt = BytesN::from_array(&env, &[0xAA; 32]);
    let commit = compute_commitment(&env, id, 0u8, &salt);
    client.commit_vote(&voter, &id, &commit);

    // Verify commit event
    let binding = env.events().all();
    let all = binding.events();
    let mut found_commit = false;
    for i in 0..all.len() {
        let e = all.get(i).unwrap();
        let debug = format!("{:?}", e);
        if debug.contains("VOTE_COM") {
            found_commit = true;
            break;
        }
    }
    assert!(found_commit, "VOTE_COM event not found");

    // Reveal and verify via view
    let _power = client.reveal_vote(&voter, &id, &VoteChoice::For, &salt);
    assert!(client.has_voted(&id, &voter));
}

#[test]
fn test_delegation_events() {
    let (env, contract_id, client, _admin, _staking, _treasury) = setup();

    let voter = Address::generate(&env);
    let delegate = Address::generate(&env);

    // Delegate
    client.delegate(&voter, &delegate);

    let binding = env.events().all();
    let all = binding.events();
    let mut found_set = false;
    for i in 0..all.len() {
        let e = all.get(i).unwrap();
        let debug = format!("{:?}", e);
        if debug.contains("DEL_SET") {
            found_set = true;
            break;
        }
    }
    assert!(found_set, "DEL_SET event not found");

    // Revoke
    client.revoke_delegation(&voter);

    let binding = env.events().all();
    let all = binding.events();
    let mut found_rev = false;
    for i in 0..all.len() {
        let e = all.get(i).unwrap();
        let debug = format!("{:?}", e);
        if debug.contains("DEL_REV") {
            found_rev = true;
            break;
        }
    }
    assert!(found_rev, "DEL_REV event not found");
}
