#![cfg(test)]

extern crate std;

use governor::proposal::{ProposalAction, ProposalPhase, ProposalType};
use governor::{GovernorContract, GovernorContractClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    vec, Address, BytesN, Env, String, Vec,
};

fn setup() -> (Env, Address, GovernorContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);
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

#[test]
fn test_discussion_to_voting_requires_time() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 5_000);

    let target = Address::generate(&env);
    let actions = vec![
        &env,
        ProposalAction {
            target,
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&env, &[0u8; 32]),
        },
    ];

    let id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Time test"),
        &actions,
    );

    // Draft -> Discussion (immediate)
    client.advance_phase(&proposer, &id);

    // Attempt Discussion -> Voting immediately should fail
    let res = client.try_advance_phase(&proposer, &id);
    assert!(res.is_err());

    // Advance past discussion length (3 days) then succeed
    advance_time(&env, 3 * 86_400 + 1);
    let res2 = client.try_advance_phase(&proposer, &id);
    assert!(res2.is_ok());
}

#[test]
fn test_timelock_requires_timestamp_to_expire() {
    let (env, contract_id, client, proposer) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 250_000_000);

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
        &String::from_str(&env, "Timelock test"),
        &actions,
    );

    // Move to Discussion -> Voting
    client.advance_phase(&proposer, &id);
    advance_time(&env, 3 * 86_400 + 1);
    client.advance_phase(&proposer, &id);

    // Setup two voters to meet quorum and pass
    let v1 = Address::generate(&env);
    let v2 = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &v1, 250_000_000);
    set_mock_stake(&env, &contract_id, &v2, 250_000_000);
    set_mock_age(&env, &contract_id, &v1, 365 * 86_400);
    set_mock_age(&env, &contract_id, &v2, 365 * 86_400);

    // Commit & Reveal For votes
    let salt1 = BytesN::from_array(&env, &[0xAA; 32]);
    let salt2 = BytesN::from_array(&env, &[0xBB; 32]);
    let commit1 = {
        use soroban_sdk::Bytes;
        let mut data = Bytes::new(&env);
        for b in id.to_le_bytes().iter() {
            data.push_back(*b);
        }
        data.push_back(0u8);
        for i in 0..32u32 {
            data.push_back(salt1.get(i).unwrap_or(0));
        }
        env.crypto().sha256(&data).into()
    };
    let commit2 = {
        use soroban_sdk::Bytes;
        let mut data = Bytes::new(&env);
        for b in id.to_le_bytes().iter() {
            data.push_back(*b);
        }
        data.push_back(0u8);
        for i in 0..32u32 {
            data.push_back(salt2.get(i).unwrap_or(0));
        }
        env.crypto().sha256(&data).into()
    };

    client.commit_vote(&v1, &id, &commit1);
    client.commit_vote(&v2, &id, &commit2);
    client.reveal_vote(&v1, &id, &governor::voting::VoteChoice::For, &salt1);
    client.reveal_vote(&v2, &id, &governor::voting::VoteChoice::For, &salt2);

    // Move Voting -> Timelock
    advance_time(&env, 5 * 86_400 + 1);
    client.advance_phase(&proposer, &id);

    // Now in Timelock. Attempt to advance before timelock end should error TimelockNotExpired
    let res = client.try_advance_phase(&proposer, &id);
    assert!(res.is_err());

    // Read timelock_ends and advance ledger to after it
    let prop = client.get_proposal(&id).unwrap();
    let t_end = prop.timelock_ends;
    env.ledger().set_timestamp(t_end + 1);

    // Now advancing should succeed to Execution
    let res2 = client.try_advance_phase(&proposer, &id);
    assert!(res2.is_ok());
    // Verify stored proposal phase is Execution
    let prop = client.get_proposal(&id).unwrap();
    assert!(matches!(prop.phase, ProposalPhase::Execution));
}
