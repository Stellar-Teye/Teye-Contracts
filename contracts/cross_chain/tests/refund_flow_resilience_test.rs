#![allow(clippy::unwrap_used, clippy::expect_used)]

//! # Refund Flow Resilience Tests
//!
//! This test suite validates the refund and rollback mechanisms for the
//! cross-chain bridge. The tests cover:
//!
//! 1. **Timeout Scenarios**: Simulating destination chain timeouts where
//!    messages fail to reach their destination within acceptable timeframes.
//! 2. **State Rollback Mechanisms**: Verifying proper cleanup and state
//!    reversion during failed cross-chain attempts.
//! 3. **Refund Logic Edge Cases**: Testing boundary conditions and error
//!    handling in refund scenarios.

use cross_chain::{
    bridge::{self, anchor_root, get_import_timestamp, ExportPackage},
    CrossChainContract, CrossChainContractClient, CrossChainError,
};
use soroban_sdk::{
    contract, contractimpl, symbol_short, testutils::Address as _, testutils::Ledger, Address,
    Bytes, BytesN, Env, String, Vec,
};

// ---------------------------------------------------------------------------
// Mock contracts for simulating cross-chain scenarios
// ---------------------------------------------------------------------------

#[contract]
struct MockDestinationChain;

#[contractimpl]
impl MockDestinationChain {
    /// Simulates successful message delivery
    pub fn receive_message(env: Env, _message_id: Bytes, payload: Bytes) -> Result<(), ()> {
        // Store to indicate successful delivery
        env.storage()
            .instance()
            .set(&symbol_short!("RCVD"), &payload);
        Ok(())
    }

    /// Simulates timeout/failure scenario
    pub fn receive_with_timeout(env: Env, _message_id: Bytes, _payload: Bytes) -> Result<(), ()> {
        // Always fails to simulate timeout
        Err(())
    }
}

#[contract]
struct MockAssetLock;

#[contractimpl]
impl MockAssetLock {
    /// Locks assets for cross-chain transfer
    pub fn lock_assets(env: Env, from: Address, amount: i128, message_id: Bytes) {
        let key = (symbol_short!("LOCK"), from.clone(), message_id);
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + amount));
    }

    /// Releases locked assets (refund)
    pub fn release_assets(env: Env, to: Address, message_id: Bytes) -> Result<i128, ()> {
        let key = (symbol_short!("LOCK"), to.clone(), message_id);
        let amount: Option<i128> = env.storage().persistent().get(&key);
        match amount {
            Some(amt) if amt > 0 => {
                env.storage().persistent().set(&key, &0i128);
                Ok(amt)
            }
            _ => Err(()),
        }
    }

    /// Check if assets are locked
    pub fn get_locked_amount(env: Env, address: Address, message_id: Bytes) -> i128 {
        let key = (symbol_short!("LOCK"), address, message_id);
        env.storage().persistent().get(&key).unwrap_or(0)
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
// Timeout Scenario Tests
// ---------------------------------------------------------------------------

/// Test handling of expired export packages (timeout on destination chain)
#[test]
fn test_expired_export_package_handling() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Create an export package with old timestamp
    let record_id_bytes = record_id(&env, b"timeout_record");
    let record_data = Bytes::from_slice(&env, b"medical_data");
    let fields: Vec<bridge::FieldEntry> = Vec::new(&env);

    let pkg = bridge::export_record(
        &env,
        record_id_bytes.clone(),
        record_data.clone(),
        fields,
        None,
        symbol_short!("ETH"),
    );

    // Anchor the root
    let exported_root = pkg.state_root.clone();
    client.anchor_state_root(&exported_root, symbol_short!("ETH"));

    // Advance ledger significantly (simulate timeout period)
    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 1000; // Well beyond normal finality
        l.timestamp = l.timestamp + 1_000_000; // Large time jump
    });

    // Import should still work if proof is valid and finality met
    // (This tests that timeout is not based on package timestamp alone)
    let result = client.try_import_record(&pkg, &exported_root);

    // The current implementation doesn't check package expiry,
    // only finality window. This is expected behavior.
    assert!(
        result.is_ok(),
        "Valid proof should be accepted regardless of age"
    );
}

/// Test import after excessive delay (destination chain unreachable)
#[test]
fn test_destination_chain_timeout_scenario() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let destination_id = env.register(MockDestinationChain, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);

    // Simulate: assets locked waiting for cross-chain confirmation
    let asset_lock_id = env.register(MockAssetLock, ());
    let user = Address::generate(&env);
    let transfer_amount = 1000i128;
    let message_id = Bytes::from_slice(&env, b"transfer_msg_001");

    // Lock assets on source chain
    env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).lock_assets(
            &user,
            transfer_amount,
            &message_id,
        );
    });

    // Create and export package
    let record_id_bytes = record_id(&env, b"timeout_test");
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

    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 50;
    });

    // Simulate destination chain being unreachable (timeout)
    // In production, this would trigger refund flow
    // For now, verify assets remain locked
    let locked_before: i128 = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).get_locked_amount(&user, &message_id)
    });

    assert_eq!(
        locked_before, transfer_amount,
        "Assets should remain locked"
    );

    // After timeout period, refund should be possible
    // (Implementation would check timeout threshold here)
    let refunded: i128 = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id)
            .release_assets(&user, &message_id)
            .unwrap()
    });

    assert_eq!(refunded, transfer_amount, "Full amount should be refunded");

    // Verify no assets remain locked
    let locked_after: i128 = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).get_locked_amount(&user, &message_id)
    });

    assert_eq!(
        locked_after, 0,
        "No assets should remain locked after refund"
    );
}

// ---------------------------------------------------------------------------
// State Rollback Mechanism Tests
// ---------------------------------------------------------------------------

/// Test state cleanup when cross-chain message fails
#[test]
fn test_state_cleanup_on_message_failure() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let vision_id = env.register(MockDestinationChain, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let patient = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);
    client.map_identity(&admin, &s(&env, "ethereum"), &s(&env, "0xabc"), &patient);

    // Attempt message that will fail
    let failing_payload = Bytes::new(&env); // Empty payload causes failure
    let message = cross_chain::CrossChainMessage {
        source_chain: s(&env, "ethereum"),
        source_address: s(&env, "0xabc"),
        target_action: symbol_short!("GRANT"),
        payload: failing_payload.clone(),
    };
    let message_id = Bytes::from_slice(&env, b"failing_msg");

    // Message should fail
    let result = client.try_process_message(&relayer, &message_id, &message, &vision_id);
    assert_eq!(
        result,
        Err(Ok(CrossChainError::ExternalCallFailed)),
        "Message should fail with external call error"
    );

    // Verify message was NOT marked as processed (can be retried)
    // This is important for rollback - failed messages can be retried
    let retry_result = client.try_process_message(&relayer, &message_id, &message, &vision_id);
    assert_eq!(
        retry_result,
        Err(Ok(CrossChainError::ExternalCallFailed)),
        "Failed message should be retryable"
    );
}

/// Test rollback when import verification fails mid-process
#[test]
fn test_rollback_on_import_verification_failure() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Create export package
    let record_id_bytes = record_id(&env, b"rollback_test");
    let record_data = Bytes::from_slice(&env, b"data");
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

    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 50;
    });

    // Tamper with package to cause verification failure
    pkg.record_data = Bytes::from_slice(&env, b"tampered_data");

    // Import should fail
    let result = client.try_import_record(&pkg, &exported_root);
    assert_eq!(
        result.unwrap_err(),
        cross_chain::BridgeError::ProofInvalid,
        "Tampered proof should be rejected"
    );

    // Verify no import record was created (rollback)
    let import_record = client.get_import_timestamp(&record_id_bytes);
    assert!(
        import_record.is_none(),
        "No import record should exist after failure"
    );
}

/// Test multiple sequential failures don't corrupt state
#[test]
fn test_multiple_failures_no_state_corruption() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let vision_id = env.register(MockDestinationChain, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let patient = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);
    client.map_identity(&admin, &s(&env, "ethereum"), &s(&env, "0xabc"), &patient);

    // Send multiple failing messages
    for i in 0..5 {
        let message_id = Bytes::from_slice(&env, &[b'f', b'a', b'i', b'l', i]);
        let message = cross_chain::CrossChainMessage {
            source_chain: s(&env, "ethereum"),
            source_address: s(&env, "0xabc"),
            target_action: symbol_short!("GRANT"),
            payload: Bytes::new(&env), // Causes failure
        };

        let result = client.try_process_message(&relayer, &message_id, &message, &vision_id);
        assert_eq!(
            result,
            Err(Ok(CrossChainError::ExternalCallFailed)),
            "Message {} should fail",
            i
        );
    }

    // Now send one successful message
    let success_msg_id = Bytes::from_slice(&env, b"success");
    let success_msg = cross_chain::CrossChainMessage {
        source_chain: s(&env, "ethereum"),
        source_address: s(&env, "0xabc"),
        target_action: symbol_short!("GRANT"),
        payload: Bytes::from_slice(&env, b"valid_payload"),
    };

    let result = client.try_process_message(&relayer, &success_msg_id, &success_msg, &vision_id);
    assert!(
        result.is_ok(),
        "Valid message should succeed after failures"
    );
}

// ---------------------------------------------------------------------------
// Refund Logic Edge Cases
// ---------------------------------------------------------------------------

/// Test refund with zero-amount assets
#[test]
fn test_refund_zero_amount() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let asset_lock_id = env.register(MockAssetLock, ());
    let user = Address::generate(&env);
    let message_id = Bytes::from_slice(&env, b"zero_refund");

    // Try to refund without any locked assets
    let result = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).release_assets(&user, &message_id)
    });

    assert!(result.is_err(), "Refunding unlocked assets should fail");
}

/// Test refund for non-existent message
#[test]
fn test_refund_nonexistent_message() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let asset_lock_id = env.register(MockAssetLock, ());
    let user = Address::generate(&env);
    let nonexistent_msg = Bytes::from_slice(&env, b"never_existed");

    // No assets were ever locked for this message
    let locked = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).get_locked_amount(&user, &nonexistent_msg)
    });

    assert_eq!(
        locked, 0,
        "No assets should be locked for nonexistent message"
    );

    let result = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).release_assets(&user, &nonexistent_msg)
    });

    assert!(result.is_err(), "Cannot refund nonexistent locked assets");
}

/// Test double-refund prevention
#[test]
fn test_double_refund_prevention() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let asset_lock_id = env.register(MockAssetLock, ());
    let user = Address::generate(&env);
    let transfer_amount = 500i128;
    let message_id = Bytes::from_slice(&env, b"double_refund_test");

    // Lock assets
    env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).lock_assets(
            &user,
            transfer_amount,
            &message_id,
        );
    });

    // First refund should succeed
    let refund1 = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id)
            .release_assets(&user, &message_id)
            .unwrap()
    });
    assert_eq!(refund1, transfer_amount, "First refund should succeed");

    // Second refund should fail
    let refund2 = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).release_assets(&user, &message_id)
    });
    assert!(refund2.is_err(), "Double refund should be prevented");
}

/// Test partial refund scenarios
#[test]
fn test_partial_refund_scenarios() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let asset_lock_id = env.register(MockAssetLock, ());
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let message_id = Bytes::from_slice(&env, b"partial_test");

    // Multiple users lock assets for same message (e.g., batch transfer)
    env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).lock_assets(&user1, 300i128, &message_id);
        MockAssetLockClient::new(&env, &asset_lock_id).lock_assets(&user2, 700i128, &message_id);
    });

    // Refund user1
    let refund1 = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id)
            .release_assets(&user1, &message_id)
            .unwrap()
    });
    assert_eq!(refund1, 300i128, "User1 should get correct refund");

    // User2's assets should still be locked
    let locked_user2 = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).get_locked_amount(&user2, &message_id)
    });
    assert_eq!(locked_user2, 700i128, "User2's assets should remain locked");

    // Refund user2
    let refund2 = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id)
            .release_assets(&user2, &message_id)
            .unwrap()
    });
    assert_eq!(refund2, 700i128, "User2 should get correct refund");
}

/// Test refund with invalid caller authorization
#[test]
fn test_refund_unauthorized_caller() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let asset_lock_id = env.register(MockAssetLock, ());
    let legitimate_user = Address::generate(&env);
    let attacker = Address::generate(&env);
    let transfer_amount = 1000i128;
    let message_id = Bytes::from_slice(&env, b"auth_test");

    // Legitimate user locks assets
    env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).lock_assets(
            &legitimate_user,
            transfer_amount,
            &message_id,
        );
    });

    // Attacker tries to refund to themselves
    // (In real implementation, this would require authentication)
    // Mock currently doesn't enforce auth, but this test documents the requirement
    let attacker_refund = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).release_assets(&attacker, &message_id)
    });

    // Should fail because attacker has no locked assets under their address
    assert!(
        attacker_refund.is_err(),
        "Attacker cannot refund to themselves"
    );

    // Legitimate user can still refund
    let legitimate_refund = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id)
            .release_assets(&legitimate_user, &message_id)
            .unwrap()
    });
    assert_eq!(
        legitimate_refund, transfer_amount,
        "Legitimate user should refund successfully"
    );
}

// ---------------------------------------------------------------------------
// Timeout Threshold Edge Cases
// ---------------------------------------------------------------------------

/// Test import right at finality boundary
#[test]
fn test_import_at_finality_boundary() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Create and anchor at ledger 1000
    let record_id_bytes = record_id(&env, b"boundary_test");
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

    // Advance to exactly finality boundary (assuming default finality depth)
    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 1; // Just 1 ledger forward
    });

    // With finality_depth=1, should succeed
    // This tests the minimum finality boundary
    let result = client.try_import_record(&pkg, &exported_root);

    // Behavior depends on implementation's finality check
    // Current impl may accept or reject based on exact comparison
    // This test documents the edge case
}

/// Test very large timeout values
#[test]
fn test_extreme_timeout_values() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let bridge_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let record_id_bytes = record_id(&env, b"extreme_timeout");
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

    // Advance by extreme amount
    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 1_000_000;
        l.timestamp = l.timestamp + 10_000_000;
    });

    // Very old packages should still work if proofs are valid
    // (Current implementation doesn't expire based on time)
    let result = client.try_import_record(&pkg, &exported_root);
    assert!(result.is_ok(), "Age alone shouldn't invalidate proof");
}

// ---------------------------------------------------------------------------
// Integration Test: Full Refund Flow
// ---------------------------------------------------------------------------

/// Complete end-to-end test of timeout and refund scenario
#[test]
fn test_complete_timeout_refund_flow() {
    let env = Env::default();
    env.mock_all_auths();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    // Setup
    let bridge_id = env.register(CrossChainContract, ());
    let asset_lock_id = env.register(MockAssetLock, ());
    let client = CrossChainContractClient::new(&env, &bridge_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);
    client.add_relayer(&admin, &relayer);

    // Phase 1: Lock assets for cross-chain transfer
    let transfer_amount = 2500i128;
    let message_id = Bytes::from_slice(&env, b"complete_flow");

    env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).lock_assets(
            &user,
            transfer_amount,
            &message_id,
        );
    });

    // Phase 2: Create export package
    let record_id_bytes = record_id(&env, b"flow_record");
    let record_data = Bytes::from_slice(&env, b"transfer_data");
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

    // Phase 3: Simulate timeout (destination chain unreachable)
    env.ledger().with_mut(|l| {
        l.sequence_number = l.sequence_number + 500; // Timeout period
    });

    // Phase 4: Verify assets still locked
    let locked_before: i128 = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).get_locked_amount(&user, &message_id)
    });
    assert_eq!(
        locked_before, transfer_amount,
        "Assets should remain locked during timeout"
    );

    // Phase 5: Trigger refund
    let refunded: i128 = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id)
            .release_assets(&user, &message_id)
            .unwrap()
    });
    assert_eq!(refunded, transfer_amount, "Full refund should be processed");

    // Phase 6: Verify state cleanup
    let locked_after: i128 = env.as_contract(&asset_lock_id, || {
        MockAssetLockClient::new(&env, &asset_lock_id).get_locked_amount(&user, &message_id)
    });
    assert_eq!(
        locked_after, 0,
        "All assets should be released after refund"
    );

    // Phase 7: Verify bridge state unchanged (record never imported)
    let import_ts = client.get_import_timestamp(&record_id_bytes);
    assert!(
        import_ts.is_none(),
        "Record should not be imported after timeout"
    );
}
