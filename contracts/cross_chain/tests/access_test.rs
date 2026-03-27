#![allow(clippy::unwrap_used, clippy::expect_used)]

use cross_chain::{CrossChainContract, CrossChainContractClient, CrossChainError, CrossChainMessage};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Bytes, Env, String};

#[test]
fn test_add_relayer_non_admin_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);

    assert_eq!(
        client.try_add_relayer(&non_admin, &relayer),
        Err(Ok(CrossChainError::Unauthorized))
    );
}

#[test]
fn test_map_identity_non_admin_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);

    client.initialize(&admin);

    let foreign_chain = String::from_str(&env, "ethereum");
    let foreign_address = String::from_str(&env, "0x12345");
    let local_patient = Address::generate(&env);

    assert_eq!(
        client.try_map_identity(&non_admin, &foreign_chain, &foreign_address, &local_patient),
        Err(Ok(CrossChainError::Unauthorized))
    );
}

#[test]
fn test_process_message_non_relayer_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let random_user = Address::generate(&env);
    let vision_contract = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);

    let foreign_chain = String::from_str(&env, "ethereum");
    let foreign_address = String::from_str(&env, "0xabc123");
    let local_patient = Address::generate(&env);
    client.map_identity(&admin, &foreign_chain, &foreign_address, &local_patient);

    let message_id = Bytes::from_slice(&env, &[1, 2, 3, 4]);
    let message = CrossChainMessage {
        source_chain: foreign_chain.clone(),
        source_address: foreign_address.clone(),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::new(&env),
    };

    assert_eq!(
        client.try_process_message(&random_user, &message_id, &message, &vision_contract),
        Err(Ok(CrossChainError::Unauthorized))
    );
}
