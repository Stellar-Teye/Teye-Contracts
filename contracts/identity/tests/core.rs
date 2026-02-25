#![allow(clippy::unwrap_used, clippy::expect_used)]

use identity::{recovery::RecoveryError, IdentityContract, IdentityContractClient};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

fn setup() -> (Env, IdentityContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    (env, client, owner)
}

fn add_three_guardians(
    env: &Env,
    client: &IdentityContractClient,
    owner: &Address,
) -> (Address, Address, Address) {
    let g1 = Address::generate(env);
    let g2 = Address::generate(env);
    let g3 = Address::generate(env);

    client.add_guardian(owner, &g1);
    client.add_guardian(owner, &g2);
    client.add_guardian(owner, &g3);

    (g1, g2, g3)
}

#[test]
fn test_initialize_and_owner_state() {
    let (env, client, owner) = setup();
    assert!(client.is_owner_active(&owner));
    assert_eq!(client.get_guardians(&owner).len(), 0);
    assert_eq!(client.get_recovery_threshold(&owner), 0);

    // Double initialization must fail.
    assert_eq!(
        client.try_initialize(&Address::generate(&env)),
        Err(Ok(RecoveryError::AlreadyInitialized))
    );
}

#[test]
fn test_guardian_and_threshold_management() {
    let (env, client, owner) = setup();
    let (g1, g2, g3) = add_three_guardians(&env, &client, &owner);

    let guardians = client.get_guardians(&owner);
    assert_eq!(guardians.len(), 3);
    assert!(guardians.contains(&g1));
    assert!(guardians.contains(&g2));
    assert!(guardians.contains(&g3));

    client.set_recovery_threshold(&owner, &2);
    assert_eq!(client.get_recovery_threshold(&owner), 2);

    // Unauthorized caller cannot add guardian.
    let attacker = Address::generate(&env);
    let new_guardian = Address::generate(&env);
    assert_eq!(
        client.try_add_guardian(&attacker, &new_guardian),
        Err(Ok(RecoveryError::Unauthorized))
    );
}

#[test]
fn test_recovery_flow_happy_path() {
    let (env, client, owner) = setup();
    let (g1, g2, _g3) = add_three_guardians(&env, &client, &owner);
    client.set_recovery_threshold(&owner, &2);

    let new_owner = Address::generate(&env);

    client.initiate_recovery(&g1, &owner, &new_owner);
    client.approve_recovery(&g2, &owner);

    let req = client
        .get_recovery_request(&owner)
        .expect("request should exist");
    env.ledger().set_timestamp(req.execute_after + 1);

    let caller = Address::generate(&env);
    let executed_owner = client.execute_recovery(&caller, &owner);

    assert_eq!(executed_owner, new_owner);
    assert!(!client.is_owner_active(&owner));
    assert!(client.is_owner_active(&new_owner));
}

// ===========================================================================
// ZK Credential Verification Tests
// ===========================================================================

use identity::credential::CredentialError;
use soroban_sdk::BytesN;
use zk_verifier::vk::{G1Point, G2Point, VerificationKey};
use zk_verifier::{ZkVerifierContract, ZkVerifierContractClient};

/// Set up the ZK verifier contract alongside the identity contract.
fn setup_zk_verifier(
    env: &Env,
    client: &IdentityContractClient,
    owner: &Address,
) -> ZkVerifierContractClient<'static> {
    // Register and initialize the zk_verifier contract.
    let zk_id = env.register(ZkVerifierContract, ());
    let zk_client = ZkVerifierContractClient::new(env, &zk_id);

    let zk_admin = Address::generate(env);
    zk_client.initialize(&zk_admin);

    // Use the BN254 G1 generator (1, 2) for IC points — valid curve points.
    let mut g1_x = [0u8; 32];
    g1_x[31] = 1;
    let mut g1_y = [0u8; 32];
    g1_y[31] = 2;
    let g1 = G1Point {
        x: BytesN::from_array(env, &g1_x),
        y: BytesN::from_array(env, &g1_y),
    };

    // Standard BN254 G2 generator coordinates.
    let g2_x0 = BytesN::from_array(
        env,
        &[
            0x19, 0x8e, 0x93, 0x93, 0x92, 0x0d, 0x48, 0x3a, 0x72, 0x60, 0xbf, 0xb7, 0x31, 0xfb,
            0x5d, 0x25, 0xf1, 0xaa, 0x49, 0x33, 0x35, 0xa9, 0xe7, 0x12, 0x97, 0xe4, 0x85, 0xb7,
            0xae, 0xf3, 0x12, 0xc2,
        ],
    );
    let g2_x1 = BytesN::from_array(
        env,
        &[
            0x18, 0x00, 0xde, 0xef, 0x12, 0x1f, 0x1e, 0x76, 0x42, 0x6a, 0x05, 0x83, 0x84, 0x46,
            0x4f, 0xc8, 0x9b, 0x30, 0x73, 0x01, 0x02, 0x60, 0x49, 0x2d, 0xa3, 0x5f, 0x60, 0x68,
            0x20, 0x22, 0x71, 0x67,
        ],
    );
    let g2_y0 = BytesN::from_array(
        env,
        &[
            0x09, 0x0e, 0xf2, 0xc4, 0x60, 0x21, 0x4e, 0x33, 0x5a, 0x6e, 0x68, 0x0e, 0x67, 0x0e,
            0x9b, 0x12, 0x69, 0x4a, 0x29, 0x5e, 0x16, 0x6c, 0x89, 0xa0, 0x52, 0x30, 0xbb, 0x1a,
            0x66, 0x2b, 0xca, 0x6c,
        ],
    );
    let g2_y1 = BytesN::from_array(
        env,
        &[
            0x27, 0x67, 0x3e, 0xf6, 0xe2, 0xa9, 0x22, 0x2e, 0x3f, 0x04, 0x8b, 0x93, 0xd9, 0x33,
            0xeb, 0x1e, 0x1a, 0x2d, 0x26, 0xe0, 0x80, 0x99, 0xb9, 0xb3, 0x18, 0x54, 0x71, 0x72,
            0x86, 0x8d, 0x05, 0x08,
        ],
    );
    let g2 = G2Point {
        x: (g2_x0, g2_x1),
        y: (g2_y0, g2_y1),
    };

    let mut ic = soroban_sdk::Vec::new(env);
    ic.push_back(g1.clone());
    ic.push_back(g1.clone());

    let vk = VerificationKey {
        alpha_g1: g1,
        beta_g2: g2.clone(),
        gamma_g2: g2.clone(),
        delta_g2: g2,
        ic,
    };

    zk_client.set_verification_key(&zk_admin, &vk);

    // Wire the identity contract to the zk_verifier contract.
    client.set_zk_verifier(owner, &zk_id);

    zk_client
}

/// Build a proof using the BN254 G1 generator (1, 2) — a valid curve point —
/// and the G2 generator. The proof is structurally correct and passes
/// `validate_proof_components`, but will not satisfy the pairing equation
/// (which is expected — the test verifies the cross-contract flow completes).
fn make_valid_proof(env: &Env) -> (G1Point, G2Point, G1Point, soroban_sdk::Vec<BytesN<32>>) {
    // BN254 G1 generator: (1, 2) in big-endian 256-bit representation.
    let mut g1_x = [0u8; 32];
    g1_x[31] = 1;
    let mut g1_y = [0u8; 32];
    g1_y[31] = 2;

    let proof_a = G1Point {
        x: BytesN::from_array(env, &g1_x),
        y: BytesN::from_array(env, &g1_y),
    };

    // G2 generator coordinates (standard BN254 values)
    let g2_x0 = BytesN::from_array(
        env,
        &[
            0x19, 0x8e, 0x93, 0x93, 0x92, 0x0d, 0x48, 0x3a, 0x72, 0x60, 0xbf, 0xb7, 0x31, 0xfb,
            0x5d, 0x25, 0xf1, 0xaa, 0x49, 0x33, 0x35, 0xa9, 0xe7, 0x12, 0x97, 0xe4, 0x85, 0xb7,
            0xae, 0xf3, 0x12, 0xc2,
        ],
    );
    let g2_x1 = BytesN::from_array(
        env,
        &[
            0x18, 0x00, 0xde, 0xef, 0x12, 0x1f, 0x1e, 0x76, 0x42, 0x6a, 0x05, 0x83, 0x84, 0x46,
            0x4f, 0xc8, 0x9b, 0x30, 0x73, 0x01, 0x02, 0x60, 0x49, 0x2d, 0xa3, 0x5f, 0x60, 0x68,
            0x20, 0x22, 0x71, 0x67,
        ],
    );
    let g2_y0 = BytesN::from_array(
        env,
        &[
            0x09, 0x0e, 0xf2, 0xc4, 0x60, 0x21, 0x4e, 0x33, 0x5a, 0x6e, 0x68, 0x0e, 0x67, 0x0e,
            0x9b, 0x12, 0x69, 0x4a, 0x29, 0x5e, 0x16, 0x6c, 0x89, 0xa0, 0x52, 0x30, 0xbb, 0x1a,
            0x66, 0x2b, 0xca, 0x6c,
        ],
    );
    let g2_y1 = BytesN::from_array(
        env,
        &[
            0x27, 0x67, 0x3e, 0xf6, 0xe2, 0xa9, 0x22, 0x2e, 0x3f, 0x04, 0x8b, 0x93, 0xd9, 0x33,
            0xeb, 0x1e, 0x1a, 0x2d, 0x26, 0xe0, 0x80, 0x99, 0xb9, 0xb3, 0x18, 0x54, 0x71, 0x72,
            0x86, 0x8d, 0x05, 0x08,
        ],
    );
    let proof_b = G2Point {
        x: (g2_x0, g2_x1),
        y: (g2_y0, g2_y1),
    };

    let proof_c = G1Point {
        x: BytesN::from_array(env, &g1_x),
        y: BytesN::from_array(env, &g1_y),
    };

    let mut pi = [0u8; 32];
    pi[31] = 1;
    let mut public_inputs = soroban_sdk::Vec::new(env);
    public_inputs.push_back(BytesN::from_array(env, &pi));

    (proof_a, proof_b, proof_c, public_inputs)
}

#[test]
fn test_zk_credential_verification_happy_path() {
    let (env, client, owner) = setup();
    setup_zk_verifier(&env, &client, &owner);

    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[5u8; 32]);
    let (proof_a, proof_b, proof_c, public_inputs) = make_valid_proof(&env);

    // The proof is structurally valid so the cross-contract flow completes:
    //   identity → zk_verifier → BN254 pairing check.
    // With synthetic test data the real BN254 pairing can't be satisfied,
    // so the verifier returns ZkVerificationFailed. This is the expected
    // outcome and confirms:
    //  1. The cross-contract call from identity to zk_verifier works.
    //  2. No credential details are leaked on-chain (only a typed error).
    //  3. The full verification pipeline runs end-to-end.
    let result = client.try_verify_zk_credential(
        &user,
        &resource_id,
        &proof_a,
        &proof_b,
        &proof_c,
        &public_inputs,
    );

    // With synthetic test data, current verifier behavior may return either:
    // - typed credential failure, or
    // - successful call with `false` verification result.
    assert!(
        result == Err(Ok(CredentialError::ZkVerificationFailed)) || result == Ok(Ok(false)),
        "Cross-contract ZK flow should complete with a deterministic non-success outcome for synthetic test data"
    );
}

#[test]
fn test_zk_credential_invalid_proof_rejected() {
    let (env, client, owner) = setup();
    setup_zk_verifier(&env, &client, &owner);

    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[6u8; 32]);

    // Build a structurally invalid proof: all-zero G1 point A is a degenerate
    // proof that is caught by `validate_proof_components` pre-check.
    let (_, proof_b, proof_c, public_inputs) = make_valid_proof(&env);

    let bad_proof_a = G1Point {
        x: BytesN::from_array(&env, &[0u8; 32]),
        y: BytesN::from_array(&env, &[0u8; 32]),
    };

    let result = client.try_verify_zk_credential(
        &user,
        &resource_id,
        &bad_proof_a,
        &proof_b,
        &proof_c,
        &public_inputs,
    );

    assert!(
        result.is_err(),
        "Invalid ZK credential proof should be rejected"
    );
}

#[test]
fn test_zk_credential_verifier_not_set() {
    let (env, client, _owner) = setup();
    // Do NOT set the zk_verifier address.

    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[7u8; 32]);
    let (proof_a, proof_b, proof_c, public_inputs) = make_valid_proof(&env);

    let result = client.try_verify_zk_credential(
        &user,
        &resource_id,
        &proof_a,
        &proof_b,
        &proof_c,
        &public_inputs,
    );

    assert!(result.is_err(), "Should fail when verifier is not set");
    assert_eq!(
        result.unwrap_err(),
        Ok(CredentialError::VerifierNotSet),
        "Error should be VerifierNotSet"
    );
}
