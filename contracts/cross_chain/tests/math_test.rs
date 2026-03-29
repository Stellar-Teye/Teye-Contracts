#![allow(clippy::unwrap_used, clippy::expect_used)]

use cross_chain::{
    bridge::{export_record, import_record, AnchoredRoot, BridgeError},
    relay,
    StateRootAnchor,
};
use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::Ledger as _,
    BytesN, Env, Symbol,
};

const TTL_SAFETY_MARGIN: u32 = 600_000;

#[contract]
struct MathHarness;

#[contractimpl]
impl MathHarness {}

fn setup() -> (Env, soroban_sdk::Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MathHarness, ());
    (env, contract_id)
}

#[test]
fn test_get_latest_root_handles_high_ledger_values_without_overflow() {
    let (env, harness_id) = setup();
    let safe_sequence = u32::MAX - TTL_SAFETY_MARGIN;
    let chain_id: Symbol = symbol_short!("ETH");
    let root = BytesN::from_array(&env, &[0xAB; 32]);
    let anchor = StateRootAnchor {
        root: root.clone(),
        ledger_sequence: safe_sequence,
        chain_id: chain_id.clone(),
        anchored_at: 1,
    };

    env.as_contract(&harness_id, || {
        env.storage().persistent().set(
            &(symbol_short!("RELAYROOT"), chain_id.clone(), safe_sequence),
            &anchor,
        );
        env.storage()
            .persistent()
            .set(&(symbol_short!("RELAYLST"), chain_id.clone()), &safe_sequence);
    });

    let latest = env
        .as_contract(&harness_id, || relay::get_latest_root(&env, chain_id))
        .expect("latest root should be present");
    assert_eq!(latest.root, root);
    assert_eq!(latest.ledger_sequence, safe_sequence);
    assert_eq!(latest.anchored_at, 1);
}

#[test]
fn test_import_record_detects_finality_overflow_instead_of_wrapping() {
    let (env, harness_id) = setup();
    env.ledger().set_sequence_number(100);
    env.ledger().set_timestamp(1);

    let record_id = BytesN::from_array(&env, &[0x11; 32]);
    let package = export_record(
        &env,
        record_id,
        soroban_sdk::Bytes::from_slice(&env, &[0x55]),
        soroban_sdk::Vec::new(&env),
        None,
        symbol_short!("ETH"),
    );
    let root = package.state_root.clone();

    env.as_contract(&harness_id, || {
        let anchor_key = (symbol_short!("BRDG_RT"), root.clone());
        let anchor_record = AnchoredRoot {
            root: root.clone(),
            anchored_at: u32::MAX - 1,
            source_chain: symbol_short!("ETH"),
        };
        env.storage().persistent().set(&anchor_key, &anchor_record);
        assert_eq!(
            import_record(&env, package.clone(), root.clone(), 10),
            Err(BridgeError::ChainReorgDetected)
        );
    });
}

#[test]
fn test_export_record_preserves_max_timestamp_without_underflow() {
    let (env, _harness_id) = setup();
    env.ledger().set_timestamp(u64::MAX);

    let record_id = BytesN::from_array(&env, &[0x22; 32]);
    let package = export_record(
        &env,
        record_id.clone(),
        soroban_sdk::Bytes::from_slice(&env, &[0xAA]),
        soroban_sdk::Vec::new(&env),
        None,
        symbol_short!("ETH"),
    );

    assert_eq!(package.record_id, record_id);
    assert_eq!(package.timestamp, u64::MAX);
}
