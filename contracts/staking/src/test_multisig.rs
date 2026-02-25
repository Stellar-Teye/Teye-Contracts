extern crate std;

use crate::{ContractError, StakingContract, StakingContractClient};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, BytesN, Env, Vec};

fn setup() -> (Env, StakingContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(StakingContract, ());
    let client = StakingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let stake_token = Address::generate(&env);
    let reward_token = Address::generate(&env);

    client.initialize(
        &admin,
        &stake_token,
        &reward_token,
        &1000,  // reward_rate
        &86400, // lock_period
    );

    (env, client, admin)
}

#[test]
fn test_legacy_admin_without_multisig() {
    let (_, client, admin) = setup();

    // Verify single-admin operations succeed when multisig is NOT configured
    // Pass proposal_id = 0 to leverage the legacy bypass
    client.set_reward_rate(&admin, &2000, &0);
    assert_eq!(client.get_reward_rate(), 2000);

    client.set_lock_period(&admin, &43200, &0);
    assert_eq!(client.get_lock_period(), 43200);
}

#[test]
fn test_multisig_happy_path() {
    let (env, client, admin) = setup();

    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);
    let signer3 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());
    signers.push_back(signer3.clone());

    // Configure 2-of-3 multisig
    client.configure_multisig(&admin, &signers, &2);

    let cfg = client.get_multisig_config().unwrap();
    assert_eq!(cfg.threshold, 2);
    assert_eq!(cfg.signers.len(), 3);

    // Signer 1 proposes a rate change
    let action = symbol_short!("RWD_RATE");
    let data_hash = BytesN::from_array(&env, &[1u8; 32]);
    let proposal_id = client.propose_admin_action(&signer1, &action, &data_hash);

    let prop = client.get_proposal(&proposal_id).unwrap();
    assert_eq!(prop.action, action);
    assert_eq!(prop.proposer, signer1);
    assert_eq!(prop.approvals.len(), 1); // Proposer counts as first approval

    // Attempting to execute with only 1 approval should fail
    let res = client.try_set_reward_rate(&admin, &3000, &proposal_id);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::MultisigRequired);

    // Signer 2 approves
    client.approve_admin_action(&signer2, &proposal_id);

    let prop_after = client.get_proposal(&proposal_id).unwrap();
    assert_eq!(prop_after.approvals.len(), 2);

    // Execution should now succeed
    client.set_reward_rate(&admin, &3000, &proposal_id);
    assert_eq!(client.get_reward_rate(), 3000);

    // Verify proposal is marked executed
    let prop_executed = client.get_proposal(&proposal_id).unwrap();
    assert!(prop_executed.executed);

    // Cannot execute the same proposal again
    let res_double = client.try_set_reward_rate(&admin, &3000, &proposal_id);
    assert_eq!(
        res_double.unwrap_err().unwrap(),
        ContractError::MultisigRequired
    );
}

#[test]
fn test_multisig_reject_unauthorized_proposer() {
    let (env, client, admin) = setup();
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    // Configure 2-of-2 multisig
    client.configure_multisig(&admin, &signers, &2);

    let unauthorized = Address::generate(&env);
    let action = symbol_short!("RWD_RATE");
    let data_hash = BytesN::from_array(&env, &[1u8; 32]);

    let res = client.try_propose_admin_action(&unauthorized, &action, &data_hash);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::MultisigError);
}

#[test]
fn test_multisig_reject_unauthorized_approver() {
    let (env, client, admin) = setup();
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    client.configure_multisig(&admin, &signers, &2);

    let action = symbol_short!("RWD_RATE");
    let data_hash = BytesN::from_array(&env, &[1u8; 32]);
    let proposal_id = client.propose_admin_action(&signer1, &action, &data_hash);

    let unauthorized = Address::generate(&env);
    let res = client.try_approve_admin_action(&unauthorized, &proposal_id);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::MultisigError);
}

#[test]
fn test_duplicate_approval_rejected() {
    let (env, client, admin) = setup();
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    client.configure_multisig(&admin, &signers, &2);

    let action = symbol_short!("RWD_RATE");
    let data_hash = BytesN::from_array(&env, &[1u8; 32]);
    let proposal_id = client.propose_admin_action(&signer1, &action, &data_hash);

    // Proposer already implicitly approved
    let res = client.try_approve_admin_action(&signer1, &proposal_id);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::MultisigError);
}
