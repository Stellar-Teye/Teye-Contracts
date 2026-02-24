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
