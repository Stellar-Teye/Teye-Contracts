#![allow(clippy::unwrap_used, clippy::expect_used)]

use identity::{credential::CredentialError, IdentityContract, IdentityContractClient};
use soroban_sdk::{
    testutils::Address as _, testutils::Ledger as _, Address, Bytes, BytesN, Env, Vec,
};

fn setup() -> (Env, IdentityContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    client.initialize(&owner);
    (env, client, owner)
}

fn make_proof(env: &Env) -> (Bytes, Bytes, Bytes, Vec<BytesN<32>>) {
    let mut pi = [0u8; 32];
    pi[31] = 1;
    let mut public_inputs = Vec::new(env);
    public_inputs.push_back(BytesN::from_array(env, &pi));
    (
        Bytes::new(env),
        Bytes::new(env),
        Bytes::new(env),
        public_inputs,
    )
}

fn setup_verifier(env: &Env, client: &IdentityContractClient, owner: &Address) {
    use zk_verifier::vk::{G1Point, G2Point, VerificationKey};
    use zk_verifier::{ZkVerifierContract, ZkVerifierContractClient};

    let zk_id = env.register(ZkVerifierContract, ());
    let zk_client = ZkVerifierContractClient::new(env, &zk_id);
    let zk_admin = Address::generate(env);
    zk_client.initialize(&zk_admin);

    let mut g1_x = [0u8; 32];
    g1_x[31] = 1;
    let mut g1_y = [0u8; 32];
    g1_y[31] = 2;
    let g1 = G1Point {
        x: BytesN::from_array(env, &g1_x),
        y: BytesN::from_array(env, &g1_y),
    };
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
    client.set_zk_verifier(owner, &zk_id);
}

// ── Expiry rejection ──────────────────────────────────────────────────────────

#[test]
fn test_expired_credential_is_rejected() {
    let (env, client, _owner) = setup();
    env.ledger().set_timestamp(1000);

    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[1u8; 32]);
    let (pa, pb, pc, pi) = make_proof(&env);

    let result = client.try_verify_zk_credential(&user, &resource_id, &pa, &pb, &pc, &pi, &500u64);

    assert_eq!(
        result,
        Err(Ok(CredentialError::CredentialExpired)),
        "credential expired at 500 must be rejected at timestamp 1000"
    );
}

#[test]
fn test_credential_one_second_past_expiry_is_rejected() {
    let (env, client, _owner) = setup();
    env.ledger().set_timestamp(1001);

    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[2u8; 32]);
    let (pa, pb, pc, pi) = make_proof(&env);

    let result = client.try_verify_zk_credential(&user, &resource_id, &pa, &pb, &pc, &pi, &1000u64);

    assert_eq!(
        result,
        Err(Ok(CredentialError::CredentialExpired)),
        "credential expired at 1000 must be rejected at timestamp 1001"
    );
}

// ── Expiry boundary ───────────────────────────────────────────────────────────

#[test]
fn test_credential_at_exact_expiry_timestamp_is_valid() {
    let (env, client, owner) = setup();
    env.ledger().set_timestamp(1000);
    setup_verifier(&env, &client, &owner);

    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[3u8; 32]);
    let (pa, pb, pc, pi) = make_proof(&env);

    // timestamp == expires_at: 1000 > 1000 is false → not expired
    let result = client.try_verify_zk_credential(&user, &resource_id, &pa, &pb, &pc, &pi, &1000u64);

    assert_ne!(
        result,
        Err(Ok(CredentialError::CredentialExpired)),
        "credential with expires_at == current timestamp must not be treated as expired"
    );
}

#[test]
fn test_valid_credential_passes_expiry_check() {
    let (env, client, owner) = setup();
    env.ledger().set_timestamp(1000);
    setup_verifier(&env, &client, &owner);

    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[4u8; 32]);
    let (pa, pb, pc, pi) = make_proof(&env);

    let result = client.try_verify_zk_credential(&user, &resource_id, &pa, &pb, &pc, &pi, &2000u64);

    assert_ne!(
        result,
        Err(Ok(CredentialError::CredentialExpired)),
        "credential expiring at 2000 must pass expiry check at timestamp 1000"
    );
}

// ── Long-term credentials ─────────────────────────────────────────────────────

#[test]
fn test_long_term_credential_remains_valid_at_large_timestamp() {
    let (env, client, owner) = setup();
    env.ledger().set_timestamp(1_000_000_000);
    setup_verifier(&env, &client, &owner);

    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[5u8; 32]);
    let (pa, pb, pc, pi) = make_proof(&env);

    // u64::MAX expiry is never exceeded
    let result =
        client.try_verify_zk_credential(&user, &resource_id, &pa, &pb, &pc, &pi, &u64::MAX);

    assert_ne!(
        result,
        Err(Ok(CredentialError::CredentialExpired)),
        "long-term credential with u64::MAX expiry must remain valid"
    );
}

#[test]
fn test_long_term_credential_expires_when_time_passes() {
    let (env, client, _owner) = setup();

    let long_expiry: u64 = 500_000_000;
    env.ledger().set_timestamp(long_expiry + 1);

    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[6u8; 32]);
    let (pa, pb, pc, pi) = make_proof(&env);

    let result =
        client.try_verify_zk_credential(&user, &resource_id, &pa, &pb, &pc, &pi, &long_expiry);

    assert_eq!(
        result,
        Err(Ok(CredentialError::CredentialExpired)),
        "long-term credential must be rejected after its expiry time"
    );
}

// ── Renewal via rebind ────────────────────────────────────────────────────────

#[test]
fn test_credential_renewal_by_rebinding() {
    let (env, client, owner) = setup();

    let old_cred_id = BytesN::from_array(&env, &[10u8; 32]);
    let new_cred_id = BytesN::from_array(&env, &[11u8; 32]);

    // Bind the original credential while it is still valid.
    env.ledger().set_timestamp(500);
    client.bind_credential(&owner, &old_cred_id);
    assert!(client.is_credential_bound(&owner, &old_cred_id));

    // Advance past the old credential's expiry.
    env.ledger().set_timestamp(2000);

    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[7u8; 32]);
    let (pa, pb, pc, pi) = make_proof(&env);

    // Old credential (expires_at = 1000) is now expired.
    let expired_result =
        client.try_verify_zk_credential(&user, &resource_id, &pa, &pb, &pc, &pi, &1000u64);
    assert_eq!(
        expired_result,
        Err(Ok(CredentialError::CredentialExpired)),
        "old credential must be rejected after expiry"
    );

    // Renewal: unbind the expired credential, bind a renewed one.
    client.unbind_credential(&owner, &old_cred_id);
    assert!(!client.is_credential_bound(&owner, &old_cred_id));

    client.bind_credential(&owner, &new_cred_id);
    assert!(client.is_credential_bound(&owner, &new_cred_id));

    // New credential has a future expiry.
    let renewed_result =
        client.try_verify_zk_credential(&user, &resource_id, &pa, &pb, &pc, &pi, &10000u64);
    assert_ne!(
        renewed_result,
        Err(Ok(CredentialError::CredentialExpired)),
        "renewed credential must pass expiry check"
    );
}
