#![allow(clippy::unwrap_used)]

use identity::{recovery::RecoveryError, IdentityContract, IdentityContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_unauthenticated_admin_calls() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);

    client.initialize(&owner);

    // To test unauthorized explicitly we can use try_add_guardian with a random attacker
    let attacker = Address::generate(&env);
    let g1 = Address::generate(&env);

    // Unauthenticated user attempting to perform admin actions
    let result = client.try_add_guardian(&attacker, &g1);

    assert_eq!(
        result,
        Err(Ok(RecoveryError::Unauthorized)),
        "Unauthorized call did not revert correctly"
    );
}
