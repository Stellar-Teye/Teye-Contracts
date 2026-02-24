#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(test)]

use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, BytesN, Env};
use zk_verifier::ZkAccessHelper;
use zk_verifier::{ContractError, ZkVerifierContract, ZkVerifierContractClient};

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
