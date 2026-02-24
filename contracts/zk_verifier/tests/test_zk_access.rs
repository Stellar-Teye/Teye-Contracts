#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(test)]

use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, BytesN, Env, Vec};
use zk_verifier::ZkAccessHelper;
use zk_verifier::{ContractError, Proof, ZkVerifierContract, ZkVerifierContractClient};

#[test]
fn test_valid_proof_verification_and_audit() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);
    let resource_id = [2u8; 32];

    // Create a mock valid proof (first byte must be 1 for a and c, pi[0] = 1).
    // Both 32-byte halves of G1 points and all four 32-byte limbs of G2 must be non-zero.
    let mut proof_a = [0u8; 64];
    proof_a[0] = 1;
    proof_a[32] = 0x02;
    let mut proof_b = [0u8; 128];
    proof_b[0] = 1;
    proof_b[32] = 0x02;
    proof_b[64] = 0x03;
    proof_b[96] = 0x04;
    let mut proof_c = [0u8; 64];
    proof_c[0] = 1;
    proof_c[32] = 0x02;
    let mut pi = [0u8; 32];
    pi[0] = 1;

    let request = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        resource_id,
        proof_a,
        proof_b,
        proof_c,
        &[&pi],
    );

    let is_valid = client.verify_access(&request);
    assert!(is_valid, "Valid proof should be verified successfully");

    // Check Audit Trail
    let audit_record = client.get_audit_record(&user, &BytesN::from_array(&env, &resource_id));
    assert!(audit_record.is_some(), "Audit record should exist");

    let record = audit_record.unwrap();
    assert_eq!(record.user, user);
    assert_eq!(record.resource_id.to_array(), resource_id);
    assert_eq!(record.timestamp, env.ledger().timestamp());
}

#[test]
fn test_invalid_proof_verification() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);
    let resource_id = [3u8; 32];

    // Create an invalid proof (first byte is 0 for a, but non-zero elsewhere
    // so it isn't degenerate)
    let mut proof_a = [0u8; 64];
    proof_a[1] = 0xff; // non-zero byte so not degenerate, but a[0]!=1 → verification fails
    proof_a[32] = 0x02;
    let mut proof_b = [0u8; 128];
    proof_b[0] = 1;
    proof_b[32] = 0x02;
    proof_b[64] = 0x03;
    proof_b[96] = 0x04;
    let mut proof_c = [0u8; 64];
    proof_c[0] = 1;
    proof_c[32] = 0x02;
    let mut pi = [0u8; 32];
    pi[0] = 1;

    let request = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        resource_id,
        proof_a,
        proof_b,
        proof_c,
        &[&pi],
    );

    let is_valid = client.verify_access(&request);
    assert!(!is_valid, "Invalid proof should be rejected");

    // Check Audit Trail (should NOT exist)
    let audit_record = client.get_audit_record(&user, &BytesN::from_array(&env, &resource_id));
    assert!(
        audit_record.is_none(),
        "Audit record should not exist for invalid proofs"
    );
}

#[test]
fn test_rate_limit_enforcement_and_reset() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Configure a small window for testing
    client.set_rate_limit_config(&admin, &2, &100);

    let user = Address::generate(&env);
    let resource_id = [4u8; 32];

    let mut proof_a = [0u8; 64];
    proof_a[0] = 1;
    proof_a[32] = 0x02;
    let mut proof_b = [0u8; 128];
    proof_b[0] = 1;
    proof_b[32] = 0x02;
    proof_b[64] = 0x03;
    proof_b[96] = 0x04;
    let mut proof_c = [0u8; 64];
    proof_c[0] = 1;
    proof_c[32] = 0x02;
    let mut pi = [0u8; 32];
    pi[0] = 1;

    let request = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        resource_id,
        proof_a,
        proof_b,
        proof_c,
        &[&pi],
    );

    // First two calls within the window should succeed
    assert!(client.verify_access(&request));
    assert!(client.verify_access(&request));

    // Third call should be rate limited
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(matches!(err, Ok(ContractError::RateLimited)));

    // Advance time beyond the window and ensure the limit resets
    let current = env.ledger().timestamp();
    env.ledger().set_timestamp(current + 101);

    let res_after_reset = client.try_verify_access(&request);
    assert!(res_after_reset.is_ok());
}

#[test]
fn test_whitelist_enforcement_and_toggle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let allowed_user = Address::generate(&env);
    let blocked_user = Address::generate(&env);

    client.set_whitelist_enabled(&admin, &true);
    client.add_to_whitelist(&admin, &allowed_user);

    let resource_id = [7u8; 32];
    let mut proof_a = [0u8; 64];
    proof_a[0] = 1;
    proof_a[32] = 0x02;
    let mut proof_b = [0u8; 128];
    proof_b[0] = 1;
    proof_b[32] = 0x02;
    proof_b[64] = 0x03;
    proof_b[96] = 0x04;
    let mut proof_c = [0u8; 64];
    proof_c[0] = 1;
    proof_c[32] = 0x02;
    let mut pi = [0u8; 32];
    pi[0] = 1;

    let allowed_request = ZkAccessHelper::create_request(
        &env,
        allowed_user.clone(),
        resource_id,
        proof_a,
        proof_b,
        proof_c,
        &[&pi],
    );
    assert!(client.verify_access(&allowed_request));

    let blocked_request = ZkAccessHelper::create_request(
        &env,
        blocked_user,
        resource_id,
        proof_a,
        proof_b,
        proof_c,
        &[&pi],
    );
    let blocked = client.try_verify_access(&blocked_request);
    assert!(blocked.is_err());
    assert!(matches!(
        blocked.unwrap_err(),
        Ok(ContractError::Unauthorized)
    ));

    client.set_whitelist_enabled(&admin, &false);
    let allowed_when_disabled = client.try_verify_access(&blocked_request);
    assert!(allowed_when_disabled.is_ok());
}

#[test]
fn test_whitelist_admin_only_management() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);

    let add_res = client.try_add_to_whitelist(&non_admin, &user);
    assert!(add_res.is_err());
    assert!(matches!(
        add_res.unwrap_err(),
        Ok(ContractError::Unauthorized)
    ));

    let remove_res = client.try_remove_from_whitelist(&non_admin, &user);
    assert!(remove_res.is_err());
    assert!(matches!(
        remove_res.unwrap_err(),
        Ok(ContractError::Unauthorized)
    ));

    let toggle_res = client.try_set_whitelist_enabled(&non_admin, &true);
    assert!(toggle_res.is_err());
    assert!(matches!(
        toggle_res.unwrap_err(),
        Ok(ContractError::Unauthorized)
    ));
}

// ---------------------------------------------------------------------------
// Malformed / adversarial proof tests (issue #128)
// ---------------------------------------------------------------------------

/// Helper: set up an initialized contract client with mocked auth.
fn setup_client(env: &Env) -> ZkVerifierContractClient<'_> {
    env.mock_all_auths();
    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    client
}

/// Build a well-formed proof that passes structural validation.
fn valid_proof_parts() -> ([u8; 64], [u8; 128], [u8; 64], [u8; 32]) {
    let mut a = [0u8; 64];
    a[0] = 1;
    a[32] = 0x02;
    let mut b = [0u8; 128];
    b[0] = 1;
    b[32] = 0x02;
    b[64] = 0x03;
    b[96] = 0x04;
    let mut c = [0u8; 64];
    c[0] = 1;
    c[32] = 0x02;
    let mut pi = [0u8; 32];
    pi[0] = 1;
    (a, b, c, pi)
}

#[test]
fn test_zeroed_proof_a_returns_degenerate_error() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    // proof.a is all zeros → DegenerateProof
    let proof_a = [0u8; 64];
    let (_, b, c, pi) = valid_proof_parts();

    let request =
        ZkAccessHelper::create_request(&env, user, [5u8; 32], proof_a, b, c, &[&pi]);
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::DegenerateProof)
    ));
}

#[test]
fn test_zeroed_proof_b_returns_degenerate_error() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    let (a, _, c, pi) = valid_proof_parts();
    let proof_b = [0u8; 128]; // all zeros

    let request =
        ZkAccessHelper::create_request(&env, user, [5u8; 32], a, proof_b, c, &[&pi]);
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::DegenerateProof)
    ));
}

#[test]
fn test_zeroed_proof_c_returns_degenerate_error() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    let (a, b, _, pi) = valid_proof_parts();
    let proof_c = [0u8; 64]; // all zeros

    let request =
        ZkAccessHelper::create_request(&env, user, [5u8; 32], a, b, proof_c, &[&pi]);
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::DegenerateProof)
    ));
}

#[test]
fn test_oversized_proof_a_returns_error() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    // proof.a all 0xFF → OversizedProofComponent
    let proof_a = [0xFFu8; 64];
    let (_, b, c, pi) = valid_proof_parts();

    let request =
        ZkAccessHelper::create_request(&env, user, [5u8; 32], proof_a, b, c, &[&pi]);
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::OversizedProofComponent)
    ));
}

#[test]
fn test_oversized_proof_b_returns_error() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    let (a, _, c, pi) = valid_proof_parts();
    let proof_b = [0xFFu8; 128]; // all 0xFF

    let request =
        ZkAccessHelper::create_request(&env, user, [5u8; 32], a, proof_b, c, &[&pi]);
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::OversizedProofComponent)
    ));
}

#[test]
fn test_oversized_proof_c_returns_error() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    let (a, b, _, pi) = valid_proof_parts();
    let proof_c = [0xFFu8; 64]; // all 0xFF

    let request =
        ZkAccessHelper::create_request(&env, user, [5u8; 32], a, b, proof_c, &[&pi]);
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::OversizedProofComponent)
    ));
}

#[test]
fn test_truncated_g1_point_a_second_half_zero() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    // First half valid, second half all-zero → MalformedG1Point
    let mut proof_a = [0u8; 64];
    proof_a[0] = 1;
    // second half stays 0
    let (_, b, c, pi) = valid_proof_parts();

    let request =
        ZkAccessHelper::create_request(&env, user, [5u8; 32], proof_a, b, c, &[&pi]);
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::MalformedG1Point)
    ));
}

#[test]
fn test_truncated_g1_point_c_second_half_zero() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    let (a, b, _, pi) = valid_proof_parts();
    // First half valid, second half all-zero → MalformedG1Point
    let mut proof_c = [0u8; 64];
    proof_c[0] = 1;

    let request =
        ZkAccessHelper::create_request(&env, user, [5u8; 32], a, b, proof_c, &[&pi]);
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::MalformedG1Point)
    ));
}

#[test]
fn test_truncated_g2_point_single_limb_zero() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    let (a, _, c, pi) = valid_proof_parts();
    // G2 with first limb valid but third limb all-zero → MalformedG2Point
    let mut proof_b = [0u8; 128];
    proof_b[0] = 0x01;
    proof_b[32] = 0x02;
    // limb at [64..96] stays zero
    proof_b[96] = 0x04;

    let request =
        ZkAccessHelper::create_request(&env, user, [5u8; 32], a, proof_b, c, &[&pi]);
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::MalformedG2Point)
    ));
}

#[test]
fn test_zeroed_public_input_returns_error() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    let (a, b, c, _) = valid_proof_parts();
    let zeroed_pi = [0u8; 32]; // all zeros

    let request =
        ZkAccessHelper::create_request(&env, user, [5u8; 32], a, b, c, &[&zeroed_pi]);
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::ZeroedPublicInput)
    ));
}

#[test]
fn test_empty_public_inputs_returns_error() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    let (a, b, c, _) = valid_proof_parts();
    // Build AccessRequest directly with an empty public_inputs vec
    let request = zk_verifier::AccessRequest {
        user,
        resource_id: BytesN::from_array(&env, &[5u8; 32]),
        proof: Proof {
            a: BytesN::from_array(&env, &a),
            b: BytesN::from_array(&env, &b),
            c: BytesN::from_array(&env, &c),
        },
        public_inputs: Vec::new(&env),
    };

    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::EmptyPublicInputs)
    ));
}

#[test]
fn test_too_many_public_inputs_returns_error() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    let (a, b, c, _) = valid_proof_parts();
    // Build 17 public inputs (limit is 16)
    let mut pi_vec = Vec::new(&env);
    for i in 0u8..17 {
        let mut pi = [0u8; 32];
        pi[0] = i + 1; // non-zero
        pi_vec.push_back(BytesN::from_array(&env, &pi));
    }

    let request = zk_verifier::AccessRequest {
        user,
        resource_id: BytesN::from_array(&env, &[5u8; 32]),
        proof: Proof {
            a: BytesN::from_array(&env, &a),
            b: BytesN::from_array(&env, &b),
            c: BytesN::from_array(&env, &c),
        },
        public_inputs: pi_vec,
    };

    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::TooManyPublicInputs)
    ));
}

#[test]
fn test_mixed_valid_and_zeroed_public_input() {
    let env = Env::default();
    let client = setup_client(&env);
    let user = Address::generate(&env);

    let (a, b, c, pi_good) = valid_proof_parts();
    let pi_bad = [0u8; 32]; // zeroed

    // First input valid, second zeroed → ZeroedPublicInput
    let request =
        ZkAccessHelper::create_request(&env, user, [5u8; 32], a, b, c, &[&pi_good, &pi_bad]);
    let res = client.try_verify_access(&request);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::ZeroedPublicInput)
    ));
}
