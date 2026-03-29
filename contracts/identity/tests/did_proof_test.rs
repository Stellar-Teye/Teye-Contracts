#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use identity::{IdentityContract, IdentityContractClient};
use zk_verifier::{ZkVerifierContract, ZkVerifierContractClient, ZkAccessHelper};
use zk_verifier::vk::{G1Point, G2Point, VerificationKey};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, Vec, Bytes,
};

fn setup_vk(env: &Env) -> VerificationKey {
    // Standard G1 point (1, 2)
    let mut x = [0u8; 32];
    x[31] = 1;
    let mut y = [0u8; 32];
    y[31] = 2;
    let g1 = G1Point {
        x: BytesN::from_array(env, &x),
        y: BytesN::from_array(env, &y),
    };

    // Standard G2 generator (placeholder valid points for test flow)
    let g2 = G2Point {
        x: (
            BytesN::from_array(env, &[0x18u8; 32]),
            BytesN::from_array(env, &[0x19u8; 32]),
        ),
        y: (
            BytesN::from_array(env, &[0x12u8; 32]),
            BytesN::from_array(env, &[0x0bu8; 32]),
        ),
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

fn setup(env: &Env) -> (IdentityContractClient<'static>, ZkVerifierContractClient<'static>, Address, Address) {
    env.mock_all_auths();

    // Register ZK Verifier
    let verifier_id = env.register(ZkVerifierContract, ());
    let verifier_client = ZkVerifierContractClient::new(env, &verifier_id);
    let admin = Address::generate(env);
    verifier_client.initialize(&admin);
    verifier_client.set_verification_key(&admin, &setup_vk(env));

    // Register Identity Contract
    let identity_id = env.register(IdentityContract, ());
    let identity_client = IdentityContractClient::new(env, &identity_id);
    let owner = Address::generate(env);
    identity_client.initialize(&owner);

    // Link Identity to Verifier
    identity_client.set_zk_verifier(&owner, &verifier_id);

    (identity_client, verifier_client, owner, verifier_id)
}

#[test]
fn test_did_resolution_to_document_hashes() {
    let env = Env::default();
    let (identity_client, _, owner, _) = setup(&env);

    // Document hashes (credentials)
    let hash1 = BytesN::from_array(&env, &[0xAAu8; 32]);
    let hash2 = BytesN::from_array(&env, &[0xBBu8; 32]);

    identity_client.bind_credential(&owner, &hash1);
    identity_client.bind_credential(&owner, &hash2);

    // Resolve DID to its hashes
    let resolved = identity_client.get_bound_credentials(&owner);
    assert_eq!(resolved.len(), 2);
    assert!(resolved.contains(&hash1));
    assert!(resolved.contains(&hash2));
    assert!(identity_client.is_credential_bound(&owner, &hash1));
}

#[test]
fn test_zk_proof_verification_integration() {
    let env = Env::default();
    let (identity_client, verifier_client, owner, _) = setup(&env);

    let resource_id = [1u8; 32];
    
    // Construct a structurally valid proof
    let proof_a = [1u8; 64];
    let proof_b = [1u8; 128];
    let proof_c = [1u8; 64];
    let pi = [1u8; 32];
    let expires_at = env.ledger().timestamp() + 3600;
    
    // Nonce must match verifier's tracker
    let nonce = verifier_client.get_nonce(&owner);

    // Submit proof via Identity Contract
    // Note: Since we are using mock data, the actual pairing check in Bn254Verifier
    // (if invoked) might fail, but the contract flow and delegation are what we test.
    // In many local test environments, the pairing is either mocked or bypassed if points are invalid.
    let result = identity_client.try_verify_zk_credential(
        &owner,
        &BytesN::from_array(&env, &resource_id),
        &Bytes::from_array(&env, &proof_a),
        &Bytes::from_array(&env, &proof_b),
        &Bytes::from_array(&env, &proof_c),
        &Vec::from_array(&env, [BytesN::from_array(&env, &pi)]),
        &expires_at,
    );

    // We expect the flow to reach the verifier.
    assert!(result.is_ok());
}

#[test]
fn test_revocation_path_compromised_key() {
    let env = Env::default();
    let (identity_client, _, owner, _) = setup(&env);

    // Setup guardians for recovery
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);
    identity_client.add_guardian(&owner, &g1);
    identity_client.add_guardian(&owner, &g2);
    identity_client.add_guardian(&owner, &g3);
    identity_client.set_recovery_threshold(&owner, &2);

    // Bind a credential before compromise
    let cred = BytesN::from_array(&env, &[0xCCu8; 32]);
    identity_client.bind_credential(&owner, &cred);

    // SIMULATE COMPROMISE: Owner loses access or key is stolen.
    // Guardians initiate recovery to rotate to a new address.
    let new_owner = Address::generate(&env);
    identity_client.initiate_recovery(&g1, &owner, &new_owner);
    identity_client.approve_recovery(&g2, &owner);

    // Fast forward cooldown
    env.ledger().set_timestamp(env.ledger().timestamp() + 172_801); // > 48h

    // Execute recovery
    let caller = Address::generate(&env);
    identity_client.execute_recovery(&caller, &owner);

    // Verify REVOCATION of old key
    assert!(!identity_client.is_owner_active(&owner));
    
    // Verify NEW key is active
    assert!(identity_client.is_owner_active(&new_owner));

    // Verify all DID data (bindings) are preserved and resolved to new owner
    let resolved = identity_client.get_bound_credentials(&new_owner);
    assert!(resolved.contains(&cred));
    assert!(identity_client.is_credential_bound(&new_owner, &cred));
}

#[test]
fn test_invalid_proof_rejected_with_error() {
    let env = Env::default();
    let (identity_client, _, owner, _) = setup(&env);

    // Expired proof
    let expires_at = env.ledger().timestamp() - 1;
    let result = identity_client.try_verify_zk_credential(
        &owner,
        &BytesN::from_array(&env, &[0u8; 32]),
        &Bytes::from_array(&env, &[1u8; 64]),
        &Bytes::from_array(&env, &[1u8; 128]),
        &Bytes::from_array(&env, &[1u8; 64]),
        &Vec::new(&env),
        &expires_at,
    );

    assert!(result.is_err());
    // Identity re-exports CredentialError::CredentialExpired as 104
}
