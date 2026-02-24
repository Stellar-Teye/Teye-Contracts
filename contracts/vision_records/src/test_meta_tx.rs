use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

use crate::*;

fn setup_env() -> (Env, VisionRecordsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

fn create_keypair(secret: &[u8; 32]) -> (SigningKey, [u8; 32]) {
    let signing_key = SigningKey::from_bytes(secret);
    let pubkey = signing_key.verifying_key().to_bytes();
    (signing_key, pubkey)
}

fn build_message_bytes(
    patient_pubkey: &[u8; 32],
    grantee_id: &[u8; 32],
    level: u32,
    expires_at: u64,
    nonce: u64,
) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.extend_from_slice(b"grant_access");
    msg.extend_from_slice(patient_pubkey);
    msg.extend_from_slice(grantee_id);
    msg.extend_from_slice(&level.to_be_bytes());
    msg.extend_from_slice(&expires_at.to_be_bytes());
    msg.extend_from_slice(&nonce.to_be_bytes());
    msg
}

#[test]
fn test_valid_meta_grant() {
    let (env, client, _admin) = setup_env();

    let (signing_key, patient_pub) = create_keypair(&[1u8; 32]);
    let (_grantee_sk, grantee_pub) = create_keypair(&[2u8; 32]);

    let patient = Address::generate(&env);
    let grantee = Address::generate(&env);
    let relayer = Address::generate(&env);

    let level: u32 = 1; // Read
    let expires_at: u64 = 1_000_000;
    let nonce: u64 = 42;

    env.ledger().with_mut(|li| {
        li.timestamp = 500_000;
    });

    let msg = build_message_bytes(&patient_pub, &grantee_pub, level, expires_at, nonce);
    let sig = signing_key.sign(&msg);

    let signed_grant = SignedGrant {
        patient: patient.clone(),
        patient_pubkey: BytesN::from_array(&env, &patient_pub),
        grantee: grantee.clone(),
        grantee_id: BytesN::from_array(&env, &grantee_pub),
        level,
        expires_at,
        nonce,
        signature: BytesN::from_array(&env, &sig.to_bytes()),
    };

    client.grant_access_meta(&relayer, &signed_grant);

    let access = client.check_access(&patient, &grantee);
    assert_eq!(access, AccessLevel::Read);
}

#[test]
#[should_panic(expected = "HostError")]
fn test_invalid_signature_rejected() {
    let (env, client, _admin) = setup_env();

    let (_signing_key, patient_pub) = create_keypair(&[1u8; 32]);
    let (_grantee_sk, grantee_pub) = create_keypair(&[2u8; 32]);

    let patient = Address::generate(&env);
    let grantee = Address::generate(&env);
    let relayer = Address::generate(&env);

    let level: u32 = 1;
    let expires_at: u64 = 1_000_000;
    let nonce: u64 = 100;

    env.ledger().with_mut(|li| {
        li.timestamp = 500_000;
    });

    // Sign with a different key to produce an invalid signature
    let (wrong_key, _) = create_keypair(&[99u8; 32]);
    let msg = build_message_bytes(&patient_pub, &grantee_pub, level, expires_at, nonce);
    let bad_sig = wrong_key.sign(&msg);

    let signed_grant = SignedGrant {
        patient: patient.clone(),
        patient_pubkey: BytesN::from_array(&env, &patient_pub),
        grantee: grantee.clone(),
        grantee_id: BytesN::from_array(&env, &grantee_pub),
        level,
        expires_at,
        nonce,
        signature: BytesN::from_array(&env, &bad_sig.to_bytes()),
    };

    client.grant_access_meta(&relayer, &signed_grant);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #26)")]
fn test_expired_meta_tx_rejected() {
    let (env, client, _admin) = setup_env();

    let (signing_key, patient_pub) = create_keypair(&[1u8; 32]);
    let (_grantee_sk, grantee_pub) = create_keypair(&[2u8; 32]);

    let patient = Address::generate(&env);
    let grantee = Address::generate(&env);
    let relayer = Address::generate(&env);

    let level: u32 = 2; // Write
    let expires_at: u64 = 100;
    let nonce: u64 = 200;

    // Set ledger time past the expires_at
    env.ledger().with_mut(|li| {
        li.timestamp = 500;
    });

    let msg = build_message_bytes(&patient_pub, &grantee_pub, level, expires_at, nonce);
    let sig = signing_key.sign(&msg);

    let signed_grant = SignedGrant {
        patient: patient.clone(),
        patient_pubkey: BytesN::from_array(&env, &patient_pub),
        grantee: grantee.clone(),
        grantee_id: BytesN::from_array(&env, &grantee_pub),
        level,
        expires_at,
        nonce,
        signature: BytesN::from_array(&env, &sig.to_bytes()),
    };

    // MetaTxExpired = 26
    client.grant_access_meta(&relayer, &signed_grant);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #27)")]
fn test_replayed_nonce_rejected() {
    let (env, client, _admin) = setup_env();

    let (signing_key, patient_pub) = create_keypair(&[1u8; 32]);
    let (_grantee_sk, grantee_pub) = create_keypair(&[2u8; 32]);

    let patient = Address::generate(&env);
    let grantee = Address::generate(&env);
    let relayer = Address::generate(&env);

    let level: u32 = 1;
    let expires_at: u64 = 1_000_000;
    let nonce: u64 = 777;

    env.ledger().with_mut(|li| {
        li.timestamp = 500_000;
    });

    let msg = build_message_bytes(&patient_pub, &grantee_pub, level, expires_at, nonce);
    let sig = signing_key.sign(&msg);

    let signed_grant = SignedGrant {
        patient: patient.clone(),
        patient_pubkey: BytesN::from_array(&env, &patient_pub),
        grantee: grantee.clone(),
        grantee_id: BytesN::from_array(&env, &grantee_pub),
        level,
        expires_at,
        nonce,
        signature: BytesN::from_array(&env, &sig.to_bytes()),
    };

    // First call succeeds
    client.grant_access_meta(&relayer, &signed_grant);

    // Second call with same nonce panics: NonceAlreadyUsed = 27
    client.grant_access_meta(&relayer, &signed_grant);
}

#[test]
fn test_meta_grant_full_access() {
    let (env, client, _admin) = setup_env();

    let (signing_key, patient_pub) = create_keypair(&[10u8; 32]);
    let (_grantee_sk, grantee_pub) = create_keypair(&[20u8; 32]);

    let patient = Address::generate(&env);
    let grantee = Address::generate(&env);
    let relayer = Address::generate(&env);

    let level: u32 = 3; // Full
    let expires_at: u64 = 2_000_000;
    let nonce: u64 = 1;

    env.ledger().with_mut(|li| {
        li.timestamp = 100_000;
    });

    let msg = build_message_bytes(&patient_pub, &grantee_pub, level, expires_at, nonce);
    let sig = signing_key.sign(&msg);

    let signed_grant = SignedGrant {
        patient: patient.clone(),
        patient_pubkey: BytesN::from_array(&env, &patient_pub),
        grantee: grantee.clone(),
        grantee_id: BytesN::from_array(&env, &grantee_pub),
        level,
        expires_at,
        nonce,
        signature: BytesN::from_array(&env, &sig.to_bytes()),
    };

    client.grant_access_meta(&relayer, &signed_grant);

    let access = client.check_access(&patient, &grantee);
    assert_eq!(access, AccessLevel::Full);
}

#[test]
fn test_meta_grant_different_nonces() {
    let (env, client, _admin) = setup_env();

    let (signing_key, patient_pub) = create_keypair(&[5u8; 32]);
    let (_grantee_sk, grantee_pub) = create_keypair(&[6u8; 32]);

    let patient = Address::generate(&env);
    let grantee = Address::generate(&env);
    let relayer = Address::generate(&env);

    env.ledger().with_mut(|li| {
        li.timestamp = 100_000;
    });

    // First grant with nonce 1
    let msg1 = build_message_bytes(&patient_pub, &grantee_pub, 1, 2_000_000, 1);
    let sig1 = signing_key.sign(&msg1);

    let grant1 = SignedGrant {
        patient: patient.clone(),
        patient_pubkey: BytesN::from_array(&env, &patient_pub),
        grantee: grantee.clone(),
        grantee_id: BytesN::from_array(&env, &grantee_pub),
        level: 1,
        expires_at: 2_000_000,
        nonce: 1,
        signature: BytesN::from_array(&env, &sig1.to_bytes()),
    };
    client.grant_access_meta(&relayer, &grant1);

    // Second grant with nonce 2 (upgrades to Write)
    let msg2 = build_message_bytes(&patient_pub, &grantee_pub, 2, 2_000_000, 2);
    let sig2 = signing_key.sign(&msg2);

    let grant2 = SignedGrant {
        patient: patient.clone(),
        patient_pubkey: BytesN::from_array(&env, &patient_pub),
        grantee: grantee.clone(),
        grantee_id: BytesN::from_array(&env, &grantee_pub),
        level: 2,
        expires_at: 2_000_000,
        nonce: 2,
        signature: BytesN::from_array(&env, &sig2.to_bytes()),
    };
    client.grant_access_meta(&relayer, &grant2);

    let access = client.check_access(&patient, &grantee);
    assert_eq!(access, AccessLevel::Write);
}
