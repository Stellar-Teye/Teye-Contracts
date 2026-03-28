#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env};
use cross_chain::{CrossChainContract, CrossChainContractClient, CrossChainError};

#[test]
fn test_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    
    let init_result = client.try_initialize(&admin);
    assert!(init_result.is_ok());
}

#[test]
fn test_double_initialization_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    
    let init_result = client.try_initialize(&admin);
    assert_eq!(init_result.unwrap_err(), Ok(CrossChainError::AlreadyInitialized));
}
