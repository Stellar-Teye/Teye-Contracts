#![allow(clippy::unwrap_used, clippy::expect_used)]
use crate::{CrossChainContract, CrossChainContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // Initialize should succeed
    assert_eq!(client.initialize(&admin), ());
}

#[test]
fn test_double_initialization_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Second initialization should fail
    assert_eq!(
        client.try_initialize(&admin),
        Err(Ok(CrossChainError::AlreadyInitialized))
    );
}

#[test]
fn test_add_relayer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);

    // Admin adding relayer should succeed
    assert_eq!(client.add_relayer(&admin, &relayer), ());
    assert!(client.is_relayer(&relayer));
}

#[test]
fn test_add_relayer_non_admin_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);

    // Non-admin caller should fail with Unauthorized
    assert_eq!(
        client.try_add_relayer(&non_admin, &relayer),
        Err(Ok(CrossChainError::Unauthorized))
    );
}

#[test]
fn test_map_identity() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let foreign_chain = String::from_str(&env, "ethereum");
    let foreign_address = String::from_str(&env, "0x12345");
    let local_patient = Address::generate(&env);

    assert_eq!(
        client.map_identity(&admin, &foreign_chain, &foreign_address, &local_patient),
        ()
    );

    let retrieved_address = client
        .get_local_address(&foreign_chain, &foreign_address)
        .unwrap();
    assert_eq!(retrieved_address, local_patient);
}

#[test]
fn test_map_identity_non_admin_fails() {
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

// Helper to set up a fully configured contract for process_message tests
fn setup_process_message_env() -> (
    Env,
    CrossChainContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let vision_contract = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);

    let foreign_chain = String::from_str(&env, "ethereum");
    let foreign_address = String::from_str(&env, "0xabc123");
    let local_patient = Address::generate(&env);
    client.map_identity(&admin, &foreign_chain, &foreign_address, &local_patient);

    (env, client, relayer, vision_contract, admin)
}

#[test]
fn test_process_message_grant_success() {
    let (env, client, relayer, vision_contract, _admin) = setup_process_message_env();

    let message_id = Bytes::from_slice(&env, &[1, 2, 3, 4]);
    let message = CrossChainMessage {
        source_chain: String::from_str(&env, "ethereum"),
        source_address: String::from_str(&env, "0xabc123"),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::new(&env),
    };

    // Should succeed
    assert_eq!(
        client.process_message(&relayer, &message_id, &message, &vision_contract),
        ()
    );
}

#[test]
fn test_process_message_replay_fails() {
    let (env, client, relayer, vision_contract, _admin) = setup_process_message_env();

    let message_id = Bytes::from_slice(&env, &[1, 2, 3, 4]);
    let message = CrossChainMessage {
        source_chain: String::from_str(&env, "ethereum"),
        source_address: String::from_str(&env, "0xabc123"),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::new(&env),
    };

    // First call succeeds
    client.process_message(&relayer, &message_id, &message, &vision_contract);

    // Replay should fail with AlreadyProcessed
    assert_eq!(
        client.try_process_message(&relayer, &message_id, &message, &vision_contract),
        Err(Ok(CrossChainError::AlreadyProcessed))
    );
}

#[test]
fn test_process_message_unknown_identity_fails() {
    let (env, client, relayer, vision_contract, _admin) = setup_process_message_env();

    let message_id = Bytes::from_slice(&env, &[5, 6, 7, 8]);
    let message = CrossChainMessage {
        source_chain: String::from_str(&env, "polygon"),
        source_address: String::from_str(&env, "0xunknown"),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::new(&env),
    };

    // Unmapped foreign identity should fail
    assert_eq!(
        client.try_process_message(&relayer, &message_id, &message, &vision_contract),
        Err(Ok(CrossChainError::UnknownIdentity))
    );
}

#[test]
fn test_process_message_unknown_identity_not_permanently_blocked() {
    let (env, client, relayer, vision_contract, admin) = setup_process_message_env();

    let message_id = Bytes::from_slice(&env, &[9, 10, 11, 12]);
    let message = CrossChainMessage {
        source_chain: String::from_str(&env, "polygon"),
        source_address: String::from_str(&env, "0xnewuser"),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::new(&env),
    };

    // First attempt fails because identity is not mapped
    assert_eq!(
        client.try_process_message(&relayer, &message_id, &message, &vision_contract),
        Err(Ok(CrossChainError::UnknownIdentity))
    );

    // Map the identity after the failed attempt
    let local_patient = Address::generate(&env);
    let foreign_chain = String::from_str(&env, "polygon");
    let foreign_address = String::from_str(&env, "0xnewuser");
    client.map_identity(&admin, &foreign_chain, &foreign_address, &local_patient);

    // Retry with the same message_id should now succeed (not AlreadyProcessed)
    assert_eq!(
        client.process_message(&relayer, &message_id, &message, &vision_contract),
        ()
    );
}

#[test]
fn test_process_message_unsupported_action_fails() {
    let (env, client, relayer, vision_contract, _admin) = setup_process_message_env();

    let message_id = Bytes::from_slice(&env, &[13, 14, 15, 16]);
    let message = CrossChainMessage {
        source_chain: String::from_str(&env, "ethereum"),
        source_address: String::from_str(&env, "0xabc123"),
        target_action: symbol_short!("REVOKE"),
        payload: Bytes::new(&env),
    };

    // Unsupported action should fail
    assert_eq!(
        client.try_process_message(&relayer, &message_id, &message, &vision_contract),
        Err(Ok(CrossChainError::UnsupportedAction))
    );
}

#[test]
fn test_process_message_non_relayer_fails() {
    let (env, client, _relayer, vision_contract, _admin) = setup_process_message_env();

    let non_relayer = Address::generate(&env);
    let message_id = Bytes::from_slice(&env, &[17, 18, 19, 20]);
    let message = CrossChainMessage {
        source_chain: String::from_str(&env, "ethereum"),
        source_address: String::from_str(&env, "0xabc123"),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::new(&env),
    };

    // Non-relayer caller should fail with Unauthorized
    assert_eq!(
        client.try_process_message(&non_relayer, &message_id, &message, &vision_contract),
        Err(Ok(CrossChainError::Unauthorized))
    );
}
