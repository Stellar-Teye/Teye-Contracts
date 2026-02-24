#![allow(clippy::unwrap_used, clippy::expect_used)]
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String, Symbol,
};

use crate::{
    AllocationSummary, ProposalStatus, TreasuryConfig, TreasuryContract, TreasuryContractClient,
};

fn setup() -> (Env, TreasuryContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy a SAC token to act as treasury asset.
    let asset_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(asset_admin);
    let token_id = token_contract.address();

    // Deploy treasury contract.
    let contract_id = env.register(TreasuryContract, ());
    let client = TreasuryContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let signer1 = admin.clone();
    let signer2 = Address::generate(&env);

    let mut signers = soroban_sdk::Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    client.initialize(&admin, &token_id, &signers, &2);

    // Pre-fund treasury with tokens.
    StellarAssetClient::new(&env, &token_id)
        .mock_all_auths()
        .mint(&contract_id, &1_000_000i128);

    (env, client, signer1, signer2)
}

#[test]
fn test_initialize_and_get_config() {
    let (_env, client, signer1, signer2) = setup();

    let cfg: TreasuryConfig = client.get_config();
    assert_eq!(cfg.signers.len(), 2);
    assert_eq!(cfg.threshold, 2);

    // Ensure both signers are recorded.
    assert!(cfg.signers.iter().any(|s| s == signer1));
    assert!(cfg.signers.iter().any(|s| s == signer2));
}

#[test]
fn test_create_approve_and_execute_proposal() {
    let (env, client, signer1, signer2) = setup();

    env.ledger().set_timestamp(100);

    let recipient = Address::generate(&env);
    let amount = 500i128;
    let category = Symbol::new(&env, "OPS");
    let description = String::from_str(&env, "Operations budget");
    let expires_at = 1_000u64;

    // Create proposal (auto-approves by proposer).
    let proposal = client.create_proposal(
        &signer1,
        &recipient,
        &amount,
        &category,
        &description,
        &expires_at,
    );
    assert_eq!(proposal.amount, amount);
    assert_eq!(proposal.category, category);
    assert_eq!(proposal.status, ProposalStatus::Pending);
    assert_eq!(proposal.approvals.len(), 1);

    let id = proposal.id;

    // Second signer approves.
    client.approve_proposal(&signer2, &id);

    // Execute proposal once threshold approvals reached.
    client.execute_proposal(&signer1, &id);

    let updated = client.get_proposal(&id).unwrap();
    assert_eq!(updated.status, ProposalStatus::Executed);

    // Check recipient received funds.
    let cfg = client.get_config();
    let token_client = TokenClient::new(&env, &cfg.token);
    let balance = token_client.balance(&recipient);
    assert_eq!(balance, amount);

    // Allocation tracking should reflect the spend.
    let summary: AllocationSummary = client.get_allocation_for_category(&category);
    assert_eq!(summary.category, category);
    assert_eq!(summary.total_spent, amount);
}

#[test]
fn test_cannot_execute_expired_proposal() {
    let (env, client, signer1, signer2) = setup();

    env.ledger().set_timestamp(100);

    let recipient = Address::generate(&env);
    let amount = 100i128;
    let category = Symbol::new(&env, "R_AND_D");
    let description = String::from_str(&env, "Research grant");
    let expires_at = 150u64;

    let proposal = client.create_proposal(
        &signer1,
        &recipient,
        &amount,
        &category,
        &description,
        &expires_at,
    );
    let id = proposal.id;

    // Approve with second signer but advance beyond expiry.
    client.approve_proposal(&signer2, &id);
    env.ledger().set_timestamp(200);

    // This should now return Err(ProposalExpired).
    let res = client.try_execute_proposal(&signer1, &id);
    assert_eq!(res, Err(Ok(crate::ContractError::ProposalExpired)));
}
