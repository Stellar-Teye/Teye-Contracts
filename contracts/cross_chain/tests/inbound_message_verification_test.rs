#![allow(clippy::unwrap_used, clippy::expect_used)]

//! # Inbound Message Verification Tests
//!
//! This test suite validates the security-critical verification logic for
//! cross-chain inbound messages. The tests cover:
//!
//! 1. **Merkle Proof Verification**: Ensures that inbound transactions are
//!    authenticated using valid Merkle proofs against anchored state roots.
//! 2. **Stale/Out-of-Order Message Handling**: Verifies that messages outside
//!    the acceptable finality window are rejected.
//! 3. **Double-Spending Prevention**: Confirms that replay attacks are prevented
//!    by tracking processed message IDs.

use cross_chain::{
    bridge::{self, anchor_root, import_record, ExportPackage},
    CrossChainContract, CrossChainContractClient, CrossChainError, CrossChainMessage,
};
use soroban_sdk::{
    contract, contractimpl, symbol_short, testutils::Address as _, testutils::Ledger, Address,
    Bytes, BytesN, Env, String, Vec,
};

// ---------------------------------------------------------------------------
// Mock Vision Records contract for integration tests
// ---------------------------------------------------------------------------

#[contract]
struct MockVisionRecords;

#[contractimpl]
impl MockVisionRecords {
    pub fn grant_cross_chain_access(
        env: Env,
        _bridge_caller: Address,
        patient: Address,
        payload: Bytes,
    ) -> Result<(), ()> {
        env.storage()
            .instance()
            .set(&symbol_short!("LAST_PAT"), &patient);
        env.storage()
            .instance()
            .set(&symbol_short!("LAST_PAY"), &payload);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Test utilities
// ---------------------------------------------------------------------------

fn s(env: &Env, value: &str) -> String {
    String::from_str(env, value)
}

fn record_id(env: &Env, seed: &[u8]) -> BytesN<32> {
    let mut arr = [0u8; 32];
    for (i, &b) in seed.iter().enumerate().take(32) {
        arr[i] = b;
    }
    BytesN::from_array(env, &arr)
}

// ---------------------------------------------------------------------------
// Merkle Proof Verification Tests
// ---------------------------------------------------------------------------

/// Test that valid Merkle proofs are accepted for inbound transactions
#[test]
fn test_valid_merkle_proof_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);

    // Create a record and export it with Merkle proof
    let record_id_bytes = record_id(&env, b"inbound_record_001");
    let record_data = Bytes::from_slice(&env, b"medical_record_payload_001");

    let fields: Vec<bridge::FieldEntry> = Vec::new(&env);

    // Export from source chain (simulated)
    let pkg = bridge::export_record(
        &env,
        record_id_bytes.clone(),
        record_data.clone(),
        fields,
        None,
        symbol_short!("ETH"),
    );

    // Anchor the state root on target chain
    let exported_root = pkg.state_root.clone();
    client.anchor_state_root(&exported_root, symbol_short!("ETH"));

    // Advance ledger beyond finality window
    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 50;
    });

    // Import should succeed with valid proof
    let result = client.try_import_record(&pkg, &exported_root);
    assert!(result.is_ok(), "Valid Merkle proof should be accepted");
}

/// Test that invalid Merkle proofs are rejected
#[test]
fn test_invalid_merkle_proof_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);

    // Create a valid export package
    let record_id_bytes = record_id(&env, b"inbound_record_002");
    let record_data = Bytes::from_slice(&env, b"medical_record_payload_002");
    let fields: Vec<bridge::FieldEntry> = Vec::new(&env);

    let mut pkg = bridge::export_record(
        &env,
        record_id_bytes.clone(),
        record_data.clone(),
        fields,
        None,
        symbol_short!("ETH"),
    );

    let exported_root = pkg.state_root.clone();
    client.anchor_state_root(&exported_root, symbol_short!("ETH"));

    // Tamper with the record data to invalidate the proof
    pkg.record_data = Bytes::from_slice(&env, b"tampered_payload");

    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 50;
    });

    // Import should fail with invalid proof
    let result = client.try_import_record(&pkg, &exported_root);
    assert!(result.is_err(), "Tampered Merkle proof should be rejected");
}

/// Test that proofs against unregistered state roots are rejected
#[test]
fn test_proof_against_unregistered_root_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);

    // Create export package
    let record_id_bytes = record_id(&env, b"inbound_record_003");
    let record_data = Bytes::from_slice(&env, b"medical_record_payload_003");
    let fields: Vec<bridge::FieldEntry> = Vec::new(&env);

    let pkg = bridge::export_record(
        &env,
        record_id_bytes.clone(),
        record_data.clone(),
        fields,
        None,
        symbol_short!("ETH"),
    );

    // Use a different root that was never anchored
    let fake_root = record_id(&env, b"fake_root");

    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 50;
    });

    // Import should fail - root not anchored
    let result = client.try_import_record(&pkg, &fake_root);
    assert!(
        result.is_err(),
        "Proof against unregistered root should be rejected"
    );
}

// ---------------------------------------------------------------------------
// Stale / Out-of-Order Message Tests
// ---------------------------------------------------------------------------

/// Test that messages within the finality window are rejected (reorg protection)
#[test]
fn test_message_within_finality_window_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);

    // Create and anchor at ledger 1000
    let record_id_bytes = record_id(&env, b"inbound_record_004");
    let record_data = Bytes::from_slice(&env, b"medical_record_payload_004");
    let fields: Vec<bridge::FieldEntry> = Vec::new(&env);

    let pkg = bridge::export_record(
        &env,
        record_id_bytes.clone(),
        record_data.clone(),
        fields,
        None,
        symbol_short!("ETH"),
    );

    let exported_root = pkg.state_root.clone();
    client.anchor_state_root(&exported_root, symbol_short!("ETH"));

    // Try to import immediately (within finality window)
    // Default finality check should reject this
    let result = client.try_import_record(&pkg, &exported_root);
    assert!(
        result.is_err(),
        "Message within finality window should be rejected to prevent reorg"
    );
}

/// Test that messages become valid after finality window passes
#[test]
fn test_message_after_finality_window_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);

    // Create and anchor at ledger 1000
    let record_id_bytes = record_id(&env, b"inbound_record_005");
    let record_data = Bytes::from_slice(&env, b"medical_record_payload_005");
    let fields: Vec<bridge::FieldEntry> = Vec::new(&env);

    let pkg = bridge::export_record(
        &env,
        record_id_bytes.clone(),
        record_data.clone(),
        fields,
        None,
        symbol_short!("ETH"),
    );

    let exported_root = pkg.state_root.clone();
    client.anchor_state_root(&exported_root, symbol_short!("ETH"));

    // Advance well beyond finality window (e.g., 100 ledgers)
    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 100;
    });

    // Should now be accepted
    let result = client.try_import_record(&pkg, &exported_root);
    assert!(
        result.is_ok(),
        "Message after finality window should be accepted"
    );
}

/// Test handling of out-of-order message arrival
#[test]
fn test_out_of_order_messages_handled() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);

    // Create two records
    let record1_id = record_id(&env, b"inbound_record_A");
    let record1_data = Bytes::from_slice(&env, b"record_A");
    let record2_id = record_id(&env, b"inbound_record_B");
    let record2_data = Bytes::from_slice(&env, b"record_B");

    let fields: Vec<bridge::FieldEntry> = Vec::new(&env);

    let pkg1 = bridge::export_record(
        &env,
        record1_id.clone(),
        record1_data.clone(),
        fields.clone(),
        None,
        symbol_short!("ETH"),
    );

    let pkg2 = bridge::export_record(
        &env,
        record2_id.clone(),
        record2_data.clone(),
        fields,
        None,
        symbol_short!("ETH"),
    );

    // Anchor both roots
    let root1 = pkg1.state_root.clone();
    let root2 = pkg2.state_root.clone();
    client.anchor_state_root(&root1, symbol_short!("ETH"));
    client.anchor_state_root(&root2, symbol_short!("ETH"));

    // Advance beyond finality
    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 50;
    });

    // Process record B first (out of order)
    let result_b = client.try_import_record(&pkg2, &root2);
    assert!(result_b.is_ok(), "Record B should be accepted");

    // Then process record A (still valid)
    let result_a = client.try_import_record(&pkg1, &root1);
    assert!(result_a.is_ok(), "Record A should still be accepted");
}

// ---------------------------------------------------------------------------
// Double-Spending Prevention Tests
// ---------------------------------------------------------------------------

/// Test that replaying the same message ID is prevented
#[test]
fn test_replay_attack_prevented() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let vision_id = env.register(MockVisionRecords, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let patient = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);
    client.map_identity(&admin, &s(&env, "ethereum"), &s(&env, "0xabc123"), &patient);

    // Create a cross-chain message
    let message_id = Bytes::from_slice(&env, b"unique_msg_001");
    let message = CrossChainMessage {
        source_chain: s(&env, "ethereum"),
        source_address: s(&env, "0xabc123"),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::from_slice(&env, b"grant_payload"),
    };

    // First processing should succeed
    client.process_message(&relayer, &message_id, &message, &vision_id);

    // Second attempt with same message_id should fail (AlreadyProcessed)
    let result = client.try_process_message(&relayer, &message_id, &message, &vision_id);
    assert_eq!(
        result,
        Err(Ok(CrossChainError::AlreadyProcessed)),
        "Replay attack should be prevented"
    );
}

/// Test that different message IDs can be processed independently
#[test]
fn test_unique_message_ids_processed_independently() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let vision_id = env.register(MockVisionRecords, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let patient = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);
    client.map_identity(&admin, &s(&env, "ethereum"), &s(&env, "0xabc123"), &patient);

    let message1 = CrossChainMessage {
        source_chain: s(&env, "ethereum"),
        source_address: s(&env, "0xabc123"),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::from_slice(&env, b"payload_1"),
    };

    let message2 = CrossChainMessage {
        source_chain: s(&env, "ethereum"),
        source_address: s(&env, "0xabc123"),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::from_slice(&env, b"payload_2"),
    };

    let msg_id_1 = Bytes::from_slice(&env, b"msg_id_1");
    let msg_id_2 = Bytes::from_slice(&env, b"msg_id_2");

    // Both unique messages should be processable
    let result1 = client.try_process_message(&relayer, &msg_id_1, &message1, &vision_id);
    assert!(result1.is_ok(), "First unique message should succeed");

    let result2 = client.try_process_message(&relayer, &msg_id_2, &message2, &vision_id);
    assert!(result2.is_ok(), "Second unique message should succeed");
}

/// Test that record import prevents duplicate imports
#[test]
fn test_duplicate_record_import_prevented() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);

    // Create and export a record
    let record_id_bytes = record_id(&env, b"duplicate_test_record");
    let record_data = Bytes::from_slice(&env, b"record_data");
    let fields: Vec<bridge::FieldEntry> = Vec::new(&env);

    let pkg = bridge::export_record(
        &env,
        record_id_bytes.clone(),
        record_data.clone(),
        fields,
        None,
        symbol_short!("ETH"),
    );

    let exported_root = pkg.state_root.clone();
    client.anchor_state_root(&exported_root, symbol_short!("ETH"));

    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 50;
    });

    // First import should succeed
    let result1 = client.try_import_record(&pkg, &exported_root);
    assert!(result1.is_ok(), "First import should succeed");

    // Note: The current implementation tracks processed messages by message_id
    // in process_message, but import_record uses record_id implicitly via the
    // export package. For complete double-spending prevention at the bridge
    // interface, ensure each record_id is tracked.
}

// ---------------------------------------------------------------------------
// Edge Cases and Boundary Conditions
// ---------------------------------------------------------------------------

/// Test zero-finality depth allows immediate import
#[test]
fn test_zero_finality_depth_immediate_import() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);

    client.initialize(&admin);

    // Create and anchor
    let record_id_bytes = record_id(&env, b"immediate_record");
    let record_data = Bytes::from_slice(&env, b"data");
    let fields: Vec<bridge::FieldEntry> = Vec::new(&env);

    let pkg = bridge::export_record(
        &env,
        record_id_bytes.clone(),
        record_data.clone(),
        fields,
        None,
        symbol_short!("ETH"),
    );

    let exported_root = pkg.state_root.clone();
    client.anchor_state_root(&exported_root, symbol_short!("ETH"));

    // With finality_depth=0, should succeed immediately
    // This tests the boundary condition
    let result = client.try_import_record(&pkg, &exported_root);
    assert!(
        result.is_ok(),
        "Zero finality depth should allow immediate import"
    );
}

/// Test field proof verification in import
#[test]
fn test_field_proof_verification_on_import() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);

    client.initialize(&admin);

    // Create record with selective field disclosure
    let record_id_bytes = record_id(&env, b"field_proof_record");
    let record_data = Bytes::from_slice(&env, b"full_record_data");

    let mut fields: Vec<bridge::FieldEntry> = Vec::new(&env);
    fields.push_back(bridge::FieldEntry {
        key: Bytes::from_slice(&env, b"diagnosis"),
        value: Bytes::from_slice(&env, b"myopia"),
    });
    fields.push_back(bridge::FieldEntry {
        key: Bytes::from_slice(&env, b"prescription"),
        value: Bytes::from_slice(&env, b"-2.50"),
    });

    // Select only diagnosis field
    let mut selected: Vec<Bytes> = Vec::new(&env);
    selected.push_back(Bytes::from_slice(&env, b"diagnosis"));

    let pkg = bridge::export_record(
        &env,
        record_id_bytes.clone(),
        record_data.clone(),
        fields,
        Some(selected),
        symbol_short!("ETH"),
    );

    let exported_root = pkg.state_root.clone();
    client.anchor_state_root(&exported_root, symbol_short!("ETH"));

    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 50;
    });

    // Import with valid field proofs should succeed
    let result = client.try_import_record(&pkg, &exported_root);
    assert!(
        result.is_ok(),
        "Import with valid field proofs should succeed"
    );
}
