#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(test)]

use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, BytesN, Env, Vec};
use zk_verifier::ZkAccessHelper;
use zk_verifier::vk::{G1Point, G2Point, VerificationKey};
use zk_verifier::{ContractError, ZkVerifierContract, ZkVerifierContractClient};

fn setup_vk(env: &Env) -> VerificationKey {
    // Valid BN254 G1 point: (1, 2) is on y^2 = x^3 + 3
    let g1_x = BytesN::from_array(env, &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    let g1_y = BytesN::from_array(env, &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
    let g1 = G1Point { x: g1_x, y: g1_y };

    // Valid BN254 G2 point (approximate for test, needs to be on curve)
    // For G2: y^2 = x^3 + 3/(9+i) in some representations, or y^2 = x^3 + 3
    // Let's use the known G2 generator if possible, or a point from a reliable source.
    // G2 Generator (from many sources):
    // x = 0x1800deef121f1e76426a058384464fc89b3073010260492da35f606820227167 + 0x198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2 * i
    // y = 0x12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc016651d54e + 0x12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc016651d54e * i
    // This is too much to type. I'll use a simpler valid point if i can find one.
    // Actually, I'll use the "Infinity" point if the host allows it, or a very simple one.
    // Let's try to use a real G2 point for (1, 2) if possible? No.
    // I'll use a hardcoded G2 generator point.
    let g2_x0 = BytesN::from_array(env, &[
        0x18, 0x00, 0xde, 0xef, 0x12, 0x1f, 0x1e, 0x76, 0x42, 0x6a, 0x05, 0x83, 0x84, 0x46, 0x4f, 0xc8,
        0x9b, 0x30, 0x73, 0x01, 0x02, 0x60, 0x49, 0x2d, 0xa3, 0x5f, 0x60, 0x68, 0x20, 0x22, 0x71, 0x67
    ]);
    let g2_x1 = BytesN::from_array(env, &[
        0x19, 0x8e, 0x93, 0x93, 0x92, 0x0d, 0x48, 0x3a, 0x72, 0x60, 0xbf, 0xb7, 0x31, 0xfb, 0x5d, 0x25,
        0xf1, 0xaa, 0x49, 0x33, 0x35, 0xa9, 0xe7, 0x12, 0x97, 0xe4, 0x85, 0xb7, 0xae, 0xf3, 0x12, 0xc2
    ]);
    let g2_y0 = BytesN::from_array(env, &[
        0x12, 0xc8, 0x5e, 0xa5, 0xdb, 0x8c, 0x6d, 0xeb, 0x4a, 0xab, 0x71, 0x80, 0x8d, 0xcb, 0x40, 0x8f,
        0xe3, 0xd1, 0xe7, 0x69, 0x0c, 0x43, 0xd3, 0x7b, 0x4c, 0xe6, 0xcc, 0x01, 0x66, 0x51, 0xd5, 0x4e
    ]);
    let g2_y1 = BytesN::from_array(env, &[
        0x0b, 0x0d, 0x0a, 0x2c, 0x14, 0x4e, 0x11, 0xed, 0xaf, 0xe3, 0x3a, 0x60, 0xc1, 0x30, 0x1f, 0x67,
        0x7a, 0xfb, 0x02, 0x35, 0x93, 0xce, 0x1e, 0x1e, 0x60, 0x0a, 0xed, 0x46, 0x2c, 0x84, 0x75, 0x8e
    ]);
    let g2 = G2Point { x: (g2_x0, g2_x1), y: (g2_y0, g2_y1) };

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

#[test]
fn test_valid_proof_verification_and_audit() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let vk = setup_vk(&env);
    client.set_verification_key(&admin, &vk);

    let user = Address::generate(&env);
    let resource_id = [2u8; 32];

    // Create a mock valid proof (first byte must be 1 for a and c, pi[0] = 1)
    let mut proof_a = [0u8; 64];
    proof_a[0] = 1;
    let mut proof_b = [0u8; 128];
    proof_b[0] = 1; // non-zero so it passes degenerate check
    let mut proof_c = [0u8; 64];
    proof_c[0] = 1;
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

    let vk = setup_vk(&env);
    client.set_verification_key(&admin, &vk);

    let user = Address::generate(&env);
    let resource_id = [3u8; 32];

    // Create an invalid proof (first byte is 0 for a, but non-zero elsewhere
    // so it isn't degenerate)
    let mut proof_a = [0u8; 64];
    proof_a[1] = 0xff; // non-zero byte so not degenerate, but a[0]!=1 → verification fails
    let mut proof_b = [0u8; 128];
    proof_b[0] = 1;
    let mut proof_c = [0u8; 64];
    proof_c[0] = 1;
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

    let vk = setup_vk(&env);
    client.set_verification_key(&admin, &vk);

    // Configure a small window for testing
    client.set_rate_limit_config(&admin, &2, &100);

    let user = Address::generate(&env);
    let resource_id = [4u8; 32];

    let mut proof_a = [0u8; 64];
    proof_a[0] = 1;
    let mut proof_b = [0u8; 128];
    proof_b[0] = 1;
    let mut proof_c = [0u8; 64];
    proof_c[0] = 1;
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

    let vk = setup_vk(&env);
    client.set_verification_key(&admin, &vk);

    let allowed_user = Address::generate(&env);
    let blocked_user = Address::generate(&env);

    client.set_whitelist_enabled(&admin, &true);
    client.add_to_whitelist(&admin, &allowed_user);

    let resource_id = [7u8; 32];
    let mut proof_a = [0u8; 64];
    proof_a[0] = 1;
    let mut proof_b = [0u8; 128];
    proof_b[0] = 1;
    let mut proof_c = [0u8; 64];
    proof_c[0] = 1;
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

// ===========================================================================
// Edge-case tests — empty inputs, zeroed proofs, oversized inputs, malformed
// ===========================================================================

#[test]
fn test_empty_public_inputs_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);

    // Build request with NO public inputs.
    let request = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        [10u8; 32],
        {
            let mut a = [0u8; 64];
            a[0] = 1;
            a
        },
        {
            let mut b = [0u8; 128];
            b[0] = 1;
            b
        },
        {
            let mut c = [0u8; 64];
            c[0] = 1;
            c
        },
        &[], // empty public inputs
    );

    let res = client.try_verify_access(&request);
    assert!(res.is_err(), "Empty public inputs must be rejected");
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::EmptyPublicInputs)
    ));
}

#[test]
fn test_zeroed_proof_bytes_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);
    let pi = [1u8; 32];

    // proof_a is all zeros → degenerate
    let request_zero_a = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        [11u8; 32],
        [0u8; 64],
        {
            let mut b = [0u8; 128];
            b[0] = 1;
            b
        },
        {
            let mut c = [0u8; 64];
            c[0] = 1;
            c
        },
        &[&pi],
    );
    let res_a = client.try_verify_access(&request_zero_a);
    assert!(
        res_a.is_err(),
        "All-zero proof.a must be rejected as degenerate"
    );
    assert!(matches!(
        res_a.unwrap_err(),
        Ok(ContractError::DegenerateProof)
    ));

    // proof_b is all zeros → degenerate
    let request_zero_b = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        [12u8; 32],
        {
            let mut a = [0u8; 64];
            a[0] = 1;
            a
        },
        [0u8; 128],
        {
            let mut c = [0u8; 64];
            c[0] = 1;
            c
        },
        &[&pi],
    );
    let res_b = client.try_verify_access(&request_zero_b);
    assert!(
        res_b.is_err(),
        "All-zero proof.b must be rejected as degenerate"
    );
    assert!(matches!(
        res_b.unwrap_err(),
        Ok(ContractError::DegenerateProof)
    ));

    // proof_c is all zeros → degenerate
    let request_zero_c = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        [13u8; 32],
        {
            let mut a = [0u8; 64];
            a[0] = 1;
            a
        },
        {
            let mut b = [0u8; 128];
            b[0] = 1;
            b
        },
        [0u8; 64],
        &[&pi],
    );
    let res_c = client.try_verify_access(&request_zero_c);
    assert!(
        res_c.is_err(),
        "All-zero proof.c must be rejected as degenerate"
    );
    assert!(matches!(
        res_c.unwrap_err(),
        Ok(ContractError::DegenerateProof)
    ));
}

#[test]
fn test_all_proof_components_zeroed_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);
    let pi = [1u8; 32];

    let request = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        [14u8; 32],
        [0u8; 64],  // all zero a
        [0u8; 128], // all zero b
        [0u8; 64],  // all zero c
        &[&pi],
    );
    let res = client.try_verify_access(&request);
    assert!(res.is_err(), "Fully zeroed proof must be rejected");
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::DegenerateProof)
    ));
}

#[test]
fn test_oversized_public_inputs_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);

    // Build 17 public inputs (MAX_PUBLIC_INPUTS = 16)
    let inputs: std::vec::Vec<[u8; 32]> = (0..17)
        .map(|i| {
            let mut buf = [0u8; 32];
            buf[0] = if i == 0 { 1 } else { (i % 255 + 1) as u8 };
            buf
        })
        .collect();
    let input_refs: std::vec::Vec<&[u8; 32]> = inputs.iter().collect();

    let request = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        [15u8; 32],
        {
            let mut a = [0u8; 64];
            a[0] = 1;
            a
        },
        {
            let mut b = [0u8; 128];
            b[0] = 1;
            b
        },
        {
            let mut c = [0u8; 64];
            c[0] = 1;
            c
        },
        &input_refs,
    );
    let res = client.try_verify_access(&request);
    assert!(res.is_err(), "More than 16 public inputs must be rejected");
    assert!(matches!(
        res.unwrap_err(),
        Ok(ContractError::TooManyPublicInputs)
    ));
}

#[test]
fn test_malformed_proof_first_byte_not_one() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);
    let pi = [1u8; 32];

    // proof_a first byte is 0xFF (not 0x01) but not all zeros → passes
    // validation but fails the mock verifier check.
    let mut bad_a = [0u8; 64];
    bad_a[0] = 0xFF;
    let request = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        [16u8; 32],
        bad_a,
        {
            let mut b = [0u8; 128];
            b[0] = 1;
            b
        },
        {
            let mut c = [0u8; 64];
            c[0] = 1;
            c
        },
        &[&pi],
    );

    let is_valid = client.verify_access(&request);
    assert!(
        !is_valid,
        "Proof with a[0] != 0x01 should fail verification"
    );
}

#[test]
fn test_malformed_public_input_first_byte_not_one() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);

    // public input first byte is 0x00 → verifier rejects
    let bad_pi = [0u8; 32];
    let request = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        [17u8; 32],
        {
            let mut a = [0u8; 64];
            a[0] = 1;
            a
        },
        {
            let mut b = [0u8; 128];
            b[0] = 1;
            b
        },
        {
            let mut c = [0u8; 64];
            c[0] = 1;
            c
        },
        &[&bad_pi],
    );

    let is_valid = client.verify_access(&request);
    assert!(
        !is_valid,
        "Public input with pi[0] == 0x00 should fail verification"
    );
}

#[test]
fn test_exactly_max_public_inputs_accepted() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);

    // Exactly 16 inputs (the maximum) should be accepted.
    let inputs: std::vec::Vec<[u8; 32]> = (0..16)
        .map(|i| {
            let mut buf = [0u8; 32];
            buf[0] = if i == 0 { 1 } else { (i % 255 + 1) as u8 };
            buf
        })
        .collect();
    let input_refs: std::vec::Vec<&[u8; 32]> = inputs.iter().collect();

    let request = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        [18u8; 32],
        {
            let mut a = [0u8; 64];
            a[0] = 1;
            a
        },
        {
            let mut b = [0u8; 128];
            b[0] = 1;
            b
        },
        {
            let mut c = [0u8; 64];
            c[0] = 1;
            c
        },
        &input_refs,
    );

    let is_valid = client.verify_access(&request);
    assert!(
        is_valid,
        "Exactly MAX_PUBLIC_INPUTS (16) should be accepted"
    );
}
