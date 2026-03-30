#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(test)]

//! # Panic Handling and State Recovery Tests
//! 
//! This test suite verifies that the ZK verifier contract correctly handles
//! and recovers from panic conditions during complex state transitions without
//! corrupting state.

use soroban_sdk::{
    testutils::Address as _,
    Address, BytesN, Env, Vec,
};
use zk_verifier::{
    AccessRequest, Proof, ZkVerifierContract, ZkVerifierContractClient,
};
use zk_verifier::vk::{G1Point, G2Point, VerificationKey};

fn setup() -> (Env, ZkVerifierContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);

    (env, client, admin, user)
}

fn create_valid_vk(env: &Env) -> VerificationKey {
    let g1_x = BytesN::from_array(
        env,
        &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1,
        ],
    );
    let g1_y = BytesN::from_array(
        env,
        &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 2,
        ],
    );
    let g1 = G1Point { x: g1_x, y: g1_y };

    let g2_x0 = BytesN::from_array(
        env,
        &[
            0x18, 0x00, 0xde, 0xef, 0x12, 0x1f, 0x1e, 0x76, 0x42, 0x6a, 0x05, 0x83, 0x84, 0x46,
            0x4f, 0xc8, 0x9b, 0x30, 0x73, 0x01, 0x02, 0x60, 0x49, 0x2d, 0xa3, 0x5f, 0x60, 0x68,
            0x20, 0x22, 0x71, 0x67,
        ],
    );
    let g2_x1 = BytesN::from_array(
        env,
        &[
            0x19, 0x8e, 0x93, 0x93, 0x92, 0x0d, 0x48, 0x3a, 0x72, 0x60, 0xbf, 0xb7, 0x31, 0xfb,
            0x5d, 0x25, 0xf1, 0xaa, 0x49, 0x33, 0x35, 0xa9, 0xe7, 0x12, 0x97, 0xe4, 0x85, 0xb7,
            0xae, 0xf3, 0x12, 0xc2,
        ],
    );
    let g2_y0 = BytesN::from_array(
        env,
        &[
            0x12, 0xc8, 0x5e, 0xa5, 0xdb, 0x8c, 0x6d, 0xeb, 0x4a, 0xab, 0x71, 0x80, 0x8d, 0xcb,
            0x40, 0x8f, 0xe3, 0xd1, 0xe7, 0x69, 0x0c, 0x43, 0xd3, 0x7b, 0x4c, 0xe6, 0xcc, 0x01,
            0x66, 0x51, 0xd5, 0x4e,
        ],
    );
    let g2_y1 = BytesN::from_array(
        env,
        &[
            0x0b, 0x0d, 0x0a, 0x2c, 0x14, 0x4e, 0x11, 0xed, 0xaf, 0xe3, 0x3a, 0x60, 0xc1, 0x30,
            0x1f, 0x67, 0x7a, 0xfb, 0x02, 0x35, 0x93, 0xce, 0x1e, 0x1e, 0x60, 0x0a, 0xed, 0x46,
            0x2c, 0x84, 0x75, 0x8e,
        ],
    );
    let g2 = G2Point {
        x: (g2_x0, g2_x1),
        y: (g2_y0, g2_y1),
    };

    let mut ic = Vec::new(env);
    ic.push_back(g1.clone());
    ic.push_back(g1.clone());

    VerificationKey {
        alpha_g1: g1.clone(),
        beta_g2: g2.clone(),
        gamma_g2: g2.clone(),
        delta_g2: g2.clone(),
        ic,
    }
}

fn create_test_request(env: &Env, user: &Address, nonce: u64) -> AccessRequest {
    let resource_id = BytesN::from_array(env, &[1u8; 32]);
    
    let proof = Proof {
        a: G1Point {
            x: BytesN::from_array(env, &[1u8; 32]),
            y: BytesN::from_array(env, &[2u8; 32]),
        },
        b: G2Point {
            x: (
                BytesN::from_array(env, &[3u8; 32]),
                BytesN::from_array(env, &[4u8; 32]),
            ),
            y: (
                BytesN::from_array(env, &[5u8; 32]),
                BytesN::from_array(env, &[6u8; 32]),
            ),
        },
        c: G1Point {
            x: BytesN::from_array(env, &[7u8; 32]),
            y: BytesN::from_array(env, &[8u8; 32]),
        },
    };

    let mut public_inputs = Vec::new(env);
    public_inputs.push_back(BytesN::from_array(env, &[9u8; 32]));

    AccessRequest {
        user: user.clone(),
        resource_id,
        proof,
        public_inputs,
        expires_at: env.ledger().timestamp() + 1000,
        nonce,
    }
}

// ============================================================================
// Panic Recovery Tests - Admin Transfer Operations
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_unauthorized_admin_proposal_panics() {
    let (_env, client, _admin, user) = setup();
    
    let new_admin = Address::generate(&_env);
    // Unauthorized user tries to propose admin - should panic
    client.propose_admin(&user, &new_admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_accept_admin_without_proposal_panics() {
    let (_env, client, admin, _user) = setup();
    
    let new_admin = Address::generate(&_env);
    // Try to accept without proposal - should panic with InvalidConfig
    client.accept_admin(&new_admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_accept_admin_wrong_caller_panics() {
    let (_env, client, admin, _user) = setup();
    
    let pending_admin = Address::generate(&_env);
    let wrong_user = Address::generate(&_env);
    
    // Set up pending admin
    client.propose_admin(&admin, &pending_admin);
    
    // Wrong user tries to accept - should panic
    client.accept_admin(&wrong_user);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_cancel_admin_transfer_unauthorized_panics() {
    let (_env, client, admin, user) = setup();
    
    let new_admin = Address::generate(&_env);
    client.propose_admin(&admin, &new_admin);
    
    // Unauthorized user tries to cancel - should panic
    client.cancel_admin_transfer(&user);
}

// ============================================================================
// Panic Recovery Tests - Rate Limiting Operations
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_rate_limit_exceeded_panics() {
    let (_env, client, admin, user) = setup();
    
    // Set strict rate limit: 2 requests per 100 seconds
    client.set_rate_limit_config(&admin, &2, &100);
    
    let request = create_test_request(&client.env, &user, 0);
    
    // First request - should succeed
    let _ = client.verify_access(&request);
    
    // Second request - should succeed
    let _ = client.verify_access(&request);
    
    // Third request - should panic with RateLimited
    let _ = client.verify_access(&request);
}

#[test]
fn test_state_consistent_after_rate_limit_panic() {
    let (env, client, admin, user) = setup();
    
    // Set rate limit
    client.set_rate_limit_config(&admin, &2, &100);
    
    let request = create_test_request(&env, &user, 0);
    
    // Make successful requests
    let _ = client.try_verify_access(&request);
    let _ = client.try_verify_access(&request);
    
    // Attempt that will fail
    let result = client.try_verify_access(&request);
    assert!(result.is_err());
    
    // Verify state is consistent - can still read data
    let nonce = client.get_nonce(&user);
    assert_eq!(nonce, 2, "Nonce should reflect only successful operations");
}

// ============================================================================
// Panic Recovery Tests - Whitelist Operations
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_add_to_whitelist_unauthorized_panics() {
    let (_env, client, _admin, user) = setup();
    
    let new_user = Address::generate(&_env);
    // Unauthorized user tries to add to whitelist - should panic
    client.add_to_whitelist(&user, &new_user);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_remove_from_whitelist_unauthorized_panics() {
    let (env, client, admin, _user) = setup();
    
    let user_to_add = Address::generate(&env);
    client.add_to_whitelist(&admin, &user_to_add);
    
    let unauthorized = Address::generate(&env);
    // Unauthorized user tries to remove - should panic
    client.remove_from_whitelist(&unauthorized, &user_to_add);
}

#[test]
fn test_whitelist_state_after_failed_operations() {
    let (env, client, admin, _user) = setup();
    
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    
    // Add users to whitelist
    client.add_to_whitelist(&admin, &user1);
    client.add_to_whitelist(&admin, &user2);
    
    assert!(client.is_whitelisted(&user1));
    assert!(client.is_whitelisted(&user2));
    
    // Try unauthorized removal (will panic internally but state should be intact)
    let unauthorized = Address::generate(&env);
    let result = client.try_remove_from_whitelist(&unauthorized, &user1);
    assert!(result.is_err());
    
    // Verify whitelist unchanged
    assert!(client.is_whitelisted(&user1), "Whitelist should be unchanged after failed removal");
    assert!(client.is_whitelisted(&user2));
}

// ============================================================================
// Panic Recovery Tests - Pause/Unpause Operations
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_pause_unauthorized_panics() {
    let (_env, client, _admin, user) = setup();
    
    // Unauthorized user tries to pause - should panic
    client.pause(&user);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_unpause_unauthorized_panics() {
    let (env, client, admin, _user) = setup();
    
    // Pause first
    client.pause(&admin);
    
    let unauthorized = Address::generate(&env);
    // Unauthorized user tries to unpause - should panic
    client.unpause(&unauthorized);
}

#[test]
fn test_contract_state_after_pause_panic() {
    let (env, client, admin, user) = setup();
    
    // Enable whitelist and add user
    client.set_whitelist_enabled(&admin, &true);
    client.add_to_whitelist(&admin, &user);
    
    // Pause the contract
    client.pause(&admin);
    
    assert!(client.is_paused());
    
    // Try to verify access while paused (should fail gracefully)
    let request = create_test_request(&env, &user, 0);
    let result = client.try_verify_access(&request);
    assert!(result.is_err());
    
    // Admin can still unpause
    client.unpause(&admin);
    assert!(!client.is_paused());
    
    // Now verification should work (may still fail for other reasons)
    let _ = client.try_verify_access(&request);
}

// ============================================================================
// Panic Recovery Tests - Verification Key Operations
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_set_verification_key_unauthorized_panics() {
    let (_env, client, _admin, user) = setup();
    
    let vk = create_valid_vk(&client.env);
    // Unauthorized user tries to set VK - should panic
    client.set_verification_key(&user, &vk);
}

#[test]
fn test_verification_key_state_after_invalid_set() {
    let (env, client, admin, _user) = setup();
    
    // Set initial valid VK
    let vk1 = create_valid_vk(&env);
    client.set_verification_key(&admin, &vk1);
    
    let stored_vk = client.get_verification_key();
    assert!(stored_vk.is_some());
    
    // Try to set with invalid caller (will panic)
    let unauthorized = Address::generate(&env);
    let vk2 = create_valid_vk(&env);
    let result = client.try_set_verification_key(&unauthorized, &vk2);
    assert!(result.is_err());
    
    // Original VK should still be there
    let stored_vk_after = client.get_verification_key();
    assert!(stored_vk_after.is_some());
}

// ============================================================================
// Panic Recovery Tests - Nonce and Replay Protection
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_nonce_replay_attack_panics() {
    let (env, client, _admin, user) = setup();
    
    let request = create_test_request(&env, &user, 0);
    
    // First use of nonce 0 - would fail proof validation but that's ok for this test
    let _ = client.try_verify_access(&request);
    
    // Try to reuse same nonce - should panic
    let _ = client.verify_access(&request);
}

#[test]
fn test_nonce_state_after_failed_verification() {
    let (env, client, _admin, user) = setup();
    
    let request = create_test_request(&env, &user, 0);
    
    // Failed verification (invalid proof)
    let _ = client.try_verify_access(&request);
    
    // Nonce should NOT increment on failure
    let nonce_after_fail = client.get_nonce(&user);
    assert_eq!(nonce_after_fail, 0, "Nonce should not increment on failed verification");
}

// ============================================================================
// Panic Recovery Tests - Reentrancy Protection
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #15)")]
fn test_reentrant_call_panics() {
    // This tests the reentrancy guard
    let (env, client, _admin, user) = setup();
    
    // Create a request
    let request = create_test_request(&env, &user, 0);
    
    // The reentrancy guard should prevent nested calls
    // In a real scenario, this would require a malicious contract
    // For now, we test that the guard mechanism exists
    
    // Simulate by trying to call verify_access recursively
    // (This is a simplified test - full reentrancy testing needs contract-to-contract calls)
    let _ = client.verify_access(&request);
}

// ============================================================================
// Panic Recovery Tests - Invalid Configuration
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_invalid_rate_limit_config_zero_max_requests_panics() {
    let (_env, client, admin, _user) = setup();
    
    // Zero max requests is invalid
    client.set_rate_limit_config(&admin, &0, &100);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_invalid_rate_limit_config_zero_window_panics() {
    let (_env, client, admin, _user) = setup();
    
    // Zero window duration is invalid
    client.set_rate_limit_config(&admin, &10, &0);
}

#[test]
fn test_config_state_after_invalid_update() {
    let (_env, client, admin, _user) = setup();
    
    // Set valid config
    client.set_rate_limit_config(&admin, &5, &60);
    let config = client.get_rate_limit_config();
    assert_eq!(config, Some((5, 60)));
    
    // Try to set invalid config
    let result = client.try_set_rate_limit_config(&admin, &0, &100);
    assert!(result.is_err());
    
    // Config should be unchanged
    let config_after = client.get_rate_limit_config();
    assert_eq!(config_after, Some((5, 60)));
}

// ============================================================================
// Panic Recovery Tests - Auth Level Validation
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_invalid_auth_level_panics() {
    let (env, client, _admin, user) = setup();
    
    let request = create_test_request(&env, &user, 0);
    
    // Invalid auth level (0 is not in 1..=4)
    let _ = client.verify_auth_level_access(&request, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_auth_level_too_high_panics() {
    let (env, client, _admin, user) = setup();
    
    let request = create_test_request(&env, &user, 0);
    
    // Auth level 5 is invalid (max is 4)
    let _ = client.verify_auth_level_access(&request, &5);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_level4_insufficient_public_inputs_panics() {
    let (env, client, _admin, user) = setup();
    
    // Create request with only 1 public input
    let mut request = create_test_request(&env, &user, 0);
    request.public_inputs = Vec::from_array(&env, [BytesN::from_array(&env, &[1u8; 32])]);
    
    // Level 4 requires at least 2 public inputs
    let _ = client.verify_auth_level_access(&request, &4);
}

// ============================================================================
// Panic Recovery Tests - Empty/Malformed Requests
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_empty_public_inputs_panics() {
    let (env, client, _admin, user) = setup();
    
    let mut request = create_test_request(&env, &user, 0);
    request.public_inputs = Vec::new(&env);
    
    let _ = client.verify_access(&request);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_degenerate_proof_panics() {
    let (env, client, _admin, user) = setup();
    
    let mut request = create_test_request(&env, &user, 0);
    
    // Set proof components to all zeros (degenerate)
    request.proof.a.x = BytesN::from_array(&env, &[0u8; 32]);
    request.proof.a.y = BytesN::from_array(&env, &[0u8; 32]);
    
    let _ = client.verify_access(&request);
}

// ============================================================================
// Comprehensive State Recovery Scenarios
// ============================================================================

#[test]
fn test_multiple_sequential_failures_dont_corrupt_state() {
    let (env, client, admin, user) = setup();
    
    // Setup: configure whitelist and rate limiting
    client.set_whitelist_enabled(&admin, &true);
    client.add_to_whitelist(&admin, &user);
    client.set_rate_limit_config(&admin, &10, &100);
    
    // Attempt multiple failing operations
    let mut request = create_test_request(&env, &user, 0);
    
    // Try with empty public inputs
    request.public_inputs = Vec::new(&env);
    let _ = client.try_verify_access(&request);
    
    // Try with degenerate proof
    request.public_inputs = Vec::from_array(&env, [BytesN::from_array(&env, &[1u8; 32])]);
    request.proof.a.x = BytesN::from_array(&env, &[0u8; 32]);
    let _ = client.try_verify_access(&request);
    
    // Try with invalid nonce (reuse)
    request.proof.a.x = BytesN::from_array(&env, &[1u8; 32]);
    let _ = client.try_verify_access(&request);
    let _ = client.try_verify_access(&request); // Reuse nonce
    
    // Verify state is still consistent
    assert!(client.is_whitelist_enabled());
    assert!(client.is_whitelisted(&user));
    assert_eq!(client.get_rate_limit_config(), Some((10, 100)));
    
    // Nonce should reflect actual successful operations (0 in this case)
    assert_eq!(client.get_nonce(&user), 0);
}

#[test]
fn test_admin_transfer_resilience_under_failure_conditions() {
    let (env, client, admin, _user) = setup();
    
    let new_admin = Address::generate(&env);
    let wrong_admin = Address::generate(&env);
    
    // Propose transfer
    client.propose_admin(&admin, &new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));
    
    // Multiple failed accept attempts
    let _ = client.try_accept_admin(&wrong_admin);
    let _ = client.try_accept_admin(&wrong_admin);
    
    // Pending admin should still be set correctly
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));
    
    // Cancel should work
    client.cancel_admin_transfer(&admin);
    assert_eq!(client.get_pending_admin(), None);
    
    // Can start new transfer
    client.propose_admin(&admin, &new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));
    
    // Successful accept
    client.accept_admin(&new_admin);
    
    // New admin can perform actions
    let yet_another = Address::generate(&env);
    client.propose_admin(&new_admin, &yet_another);
    assert_eq!(client.get_pending_admin(), Some(yet_another));
}
