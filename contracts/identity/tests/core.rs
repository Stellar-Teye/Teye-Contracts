#![allow(clippy::unwrap_used, clippy::expect_used)]

use identity::{recovery::RecoveryError, IdentityContract, IdentityContractClient};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

fn setup() -> (Env, IdentityContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    (env, client, owner)
}

fn add_three_guardians(
    env: &Env,
    client: &IdentityContractClient,
    owner: &Address,
) -> (Address, Address, Address) {
    let g1 = Address::generate(env);
    let g2 = Address::generate(env);
    let g3 = Address::generate(env);

    client.add_guardian(owner, &g1);
    client.add_guardian(owner, &g2);
    client.add_guardian(owner, &g3);

    (g1, g2, g3)
}

#[test]
fn test_initialize_and_owner_state() {
    let (env, client, owner) = setup();
    assert!(client.is_owner_active(&owner));
    assert_eq!(client.get_guardians(&owner).len(), 0);
    assert_eq!(client.get_recovery_threshold(&owner), 0);

    // Double initialization must fail.
    assert_eq!(
        client.try_initialize(&Address::generate(&env)),
        Err(Ok(RecoveryError::AlreadyInitialized))
    );
}

#[test]
fn test_guardian_and_threshold_management() {
    let (env, client, owner) = setup();
    let (g1, g2, g3) = add_three_guardians(&env, &client, &owner);

    let guardians = client.get_guardians(&owner);
    assert_eq!(guardians.len(), 3);
    assert!(guardians.contains(&g1));
    assert!(guardians.contains(&g2));
    assert!(guardians.contains(&g3));

    client.set_recovery_threshold(&owner, &2);
    assert_eq!(client.get_recovery_threshold(&owner), 2);

    // Unauthorized caller cannot add guardian.
    let attacker = Address::generate(&env);
    let new_guardian = Address::generate(&env);
    assert_eq!(
        client.try_add_guardian(&attacker, &new_guardian),
        Err(Ok(RecoveryError::Unauthorized))
    );
}

#[test]
fn test_recovery_flow_happy_path() {
    let (env, client, owner) = setup();
    let (g1, g2, _g3) = add_three_guardians(&env, &client, &owner);
    client.set_recovery_threshold(&owner, &2);

    let new_owner = Address::generate(&env);

    client.initiate_recovery(&g1, &owner, &new_owner);
    client.approve_recovery(&g2, &owner);

    let req = client
        .get_recovery_request(&owner)
        .expect("request should exist");
    env.ledger().set_timestamp(req.execute_after + 1);

    let caller = Address::generate(&env);
    let executed_owner = client.execute_recovery(&caller, &owner);

    assert_eq!(executed_owner, new_owner);
    assert!(!client.is_owner_active(&owner));
    assert!(client.is_owner_active(&new_owner));
}
