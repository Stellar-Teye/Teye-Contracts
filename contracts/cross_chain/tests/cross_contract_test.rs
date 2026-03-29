#![allow(clippy::unwrap_used, clippy::expect_used)]

use cross_chain::{CrossChainContract, CrossChainContractClient, CrossChainError, CrossChainMessage};
use soroban_sdk::{
    contract, contracterror, contractimpl, symbol_short, testutils::Address as _, Address, Bytes,
    Env, String,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
enum MockVisionError {
    BadPayload = 1,
}

#[contract]
struct MockVisionRecords;

#[contractimpl]
impl MockVisionRecords {
    pub fn grant_cross_chain_access(
        env: Env,
        bridge_caller: Address,
        patient: Address,
        payload: Bytes,
    ) -> Result<(), MockVisionError> {
        bridge_caller.require_auth();

        if payload.len() == 0 {
            return Err(MockVisionError::BadPayload);
        }

        env.storage()
            .instance()
            .set(&symbol_short!("LAST_PAT"), &patient);
        env.storage()
            .instance()
            .set(&symbol_short!("LAST_PAY"), &payload);
        Ok(())
    }
}

fn s(env: &Env, value: &str) -> String {
    String::from_str(env, value)
}

fn setup() -> (
    Env,
    CrossChainContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let bridge_id = env.register(CrossChainContract, ());
    let vision_id = env.register(MockVisionRecords, ());

    let client = CrossChainContractClient::new(&env, &bridge_id);
    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let patient = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);
    client.map_identity(&admin, &s(&env, "ethereum"), &s(&env, "0xabc123"), &patient);

    (env, client, admin, relayer, patient, vision_id)
}

#[test]
fn test_process_message_invokes_mock_contract_and_forwards_payload() {
    let (env, client, _admin, relayer, patient, vision_id) = setup();
    let payload = Bytes::from_slice(&env, &[7, 8, 9]);
    let message = CrossChainMessage {
        source_chain: s(&env, "ethereum"),
        source_address: s(&env, "0xabc123"),
        target_action: symbol_short!("GRANT"),
        payload: payload.clone(),
    };
    let message_id = Bytes::from_slice(&env, &[1, 2, 3, 4]);

    client.process_message(&relayer, &message_id, &message, &vision_id);

    let stored_patient: Address = env
        .as_contract(&vision_id, || env.storage().instance().get(&symbol_short!("LAST_PAT")))
        .expect("patient should be stored");
    let stored_payload: Bytes = env
        .as_contract(&vision_id, || env.storage().instance().get(&symbol_short!("LAST_PAY")))
        .expect("payload should be stored");

    assert_eq!(stored_patient, patient);
    assert_eq!(stored_payload, payload);
}

#[test]
fn test_process_message_external_contract_error_returns_external_call_failed() {
    let (env, client, _admin, relayer, _patient, vision_id) = setup();
    let message = CrossChainMessage {
        source_chain: s(&env, "ethereum"),
        source_address: s(&env, "0xabc123"),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::new(&env),
    };
    let message_id = Bytes::from_slice(&env, &[9, 9, 9, 9]);

    assert_eq!(
        client.try_process_message(&relayer, &message_id, &message, &vision_id),
        Err(Ok(CrossChainError::ExternalCallFailed))
    );
}

#[test]
fn test_failed_external_call_does_not_mark_message_processed() {
    let (env, client, _admin, relayer, _patient, vision_id) = setup();
    let failing = CrossChainMessage {
        source_chain: s(&env, "ethereum"),
        source_address: s(&env, "0xabc123"),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::new(&env),
    };
    let success_payload = Bytes::from_slice(&env, &[1]);
    let succeeding = CrossChainMessage {
        source_chain: s(&env, "ethereum"),
        source_address: s(&env, "0xabc123"),
        target_action: symbol_short!("GRANT"),
        payload: success_payload.clone(),
    };
    let message_id = Bytes::from_slice(&env, &[5, 4, 3, 2]);

    assert_eq!(
        client.try_process_message(&relayer, &message_id, &failing, &vision_id),
        Err(Ok(CrossChainError::ExternalCallFailed))
    );

    client.process_message(&relayer, &message_id, &succeeding, &vision_id);

    let stored_payload: Bytes = env
        .as_contract(&vision_id, || env.storage().instance().get(&symbol_short!("LAST_PAY")))
        .expect("payload should be stored after retry");
    assert_eq!(stored_payload, success_payload);
}
