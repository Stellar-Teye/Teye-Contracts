#![cfg(test)]

extern crate std;

use delegation::{DelegationContract, DelegationContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DelegationContract, ());
    let client = DelegationContractClient::new(&env, &contract_id);

    assert_eq!(client.get_admin(), None);

    let admin = Address::generate(&env);

    client.initialize(&admin);

    assert_eq!(client.get_admin(), Some(admin.clone()));
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_double_initialization_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DelegationContract, ());
    let client = DelegationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    client.initialize(&admin);
}
