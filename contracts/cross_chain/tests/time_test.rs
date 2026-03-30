#![cfg(test)]

extern crate std;

use cross_chain::bridge::{
    anchor_root, export_record, get_anchored_root, import_record, BridgeError,
};
use cross_chain::CrossChainContract;
use soroban_sdk::{
    symbol_short,
    testutils::Ledger,
    vec, BytesN, Env,
};

#[test]
fn test_chain_reorg_protection() {
    let env = Env::default();
    let contract_id = env.register(CrossChainContract, ());
    
    let mut ledger_info = env.ledger().get();
    ledger_info.timestamp = 1000;
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info);

    let root = BytesN::from_array(&env, &[1u8; 32]);
    let source_chain = symbol_short!("ETH");

    env.as_contract(&contract_id, || {
        anchor_root(&env, root.clone(), source_chain.clone());
    });

    let record_id = BytesN::from_array(&env, &[2u8; 32]);
    let record_data = soroban_sdk::Bytes::from_slice(&env, b"test data");
    
    let package = export_record(
        &env,
        record_id,
        record_data,
        vec![&env],
        None,
        source_chain,
    );

    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 105;
    env.ledger().set(ledger_info);

    let result = env.as_contract(&contract_id, || {
        import_record(&env, package.clone(), root.clone(), 10)
    });
    assert_eq!(result, Err(BridgeError::ChainReorgDetected));

    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 110;
    env.ledger().set(ledger_info);

    let result = env.as_contract(&contract_id, || {
        import_record(&env, package.clone(), root.clone(), 10)
    });
    // If it fails with ProofInvalid, it means we got past ChainReorgDetected
    match result {
        Ok(_) => {},
        Err(BridgeError::ProofInvalid) => {},
        Err(e) => panic!("Expected Ok or ProofInvalid, got {:?}", e),
    }

    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 111;
    env.ledger().set(ledger_info);

    let result = env.as_contract(&contract_id, || {
        import_record(&env, package, root, 10)
    });
    match result {
        Ok(_) => {},
        Err(BridgeError::ProofInvalid) => {},
        Err(e) => panic!("Expected Ok or ProofInvalid, got {:?}", e),
    }
}

#[test]
fn test_timestamp_manipulation() {
    let env = Env::default();
    let _contract_id = env.register(CrossChainContract, ());
    
    let mut ledger_info = env.ledger().get();
    ledger_info.timestamp = 1000;
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info);

    let record_id = BytesN::from_array(&env, &[2u8; 32]);
    let record_data = soroban_sdk::Bytes::from_slice(&env, b"test data");
    let source_chain = symbol_short!("ETH");

    let package = export_record(
        &env,
        record_id.clone(),
        record_data.clone(),
        vec![&env],
        None,
        source_chain.clone(),
    );
    assert_eq!(package.timestamp, 1000);

    let mut ledger_info = env.ledger().get();
    ledger_info.timestamp = 2000;
    env.ledger().set(ledger_info);
    
    let record_id2 = BytesN::from_array(&env, &[3u8; 32]);
    let package2 = export_record(
        &env,
        record_id2,
        record_data,
        vec![&env],
        None,
        source_chain,
    );
    assert_eq!(package2.timestamp, 2000);
}

#[test]
fn test_anchored_root_persistence() {
    let env = Env::default();
    let contract_id = env.register(CrossChainContract, ());
    
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info);

    let root = BytesN::from_array(&env, &[1u8; 32]);
    let source_chain = symbol_short!("ETH");

    env.as_contract(&contract_id, || {
        anchor_root(&env, root.clone(), source_chain.clone());
    });
    
    let anchored = env.as_contract(&contract_id, || {
        get_anchored_root(&env, root).unwrap()
    });
    assert_eq!(anchored.anchored_at, 100);
    assert_eq!(anchored.source_chain, source_chain);
    
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 200;
    env.ledger().set(ledger_info);

    let root2 = BytesN::from_array(&env, &[2u8; 32]);
    env.as_contract(&contract_id, || {
        anchor_root(&env, root2.clone(), source_chain.clone());
    });
    
    let anchored2 = env.as_contract(&contract_id, || {
        get_anchored_root(&env, root2).unwrap()
    });
    assert_eq!(anchored2.anchored_at, 200);
}
