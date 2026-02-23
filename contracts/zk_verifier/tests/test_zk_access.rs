#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};
use zk_verifier::ZkAccessHelper;
use zk_verifier::{ContractError, ZkVerifierContract, ZkVerifierContractClient};

#[test]
fn test_valid_proof_verification_and_audit() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

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

// Input validation tests 

#[test]
fn test_empty_public_inputs_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let resource_id = [4u8; 32];

    let mut proof_a = [0u8; 64];
    proof_a[0] = 1;
    let mut proof_b = [0u8; 128];
    proof_b[0] = 1;
    let mut proof_c = [0u8; 64];
    proof_c[0] = 1;

    // No public inputs at all
    let request = ZkAccessHelper::create_request(
        &env,
        user.clone(),
        resource_id,
        proof_a,
        proof_b,
        proof_c,
        &[], // empty
    );

    let result = client.try_verify_access(&request);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::EmptyPublicInputs),
        _ => unreachable!("Expected EmptyPublicInputs error"),
    }
}

#[test]
fn test_degenerate_proof_a_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let resource_id = [5u8; 32];

    // proof.a is all zeros → degenerate
    let proof_a = [0u8; 64];
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

    let result = client.try_verify_access(&request);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::DegenerateProof),
        _ => unreachable!("Expected DegenerateProof error"),
    }
}

#[test]
fn test_degenerate_proof_b_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let resource_id = [6u8; 32];

    let mut proof_a = [0u8; 64];
    proof_a[0] = 1;
    // proof.b is all zeros → degenerate
    let proof_b = [0u8; 128];
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

    let result = client.try_verify_access(&request);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::DegenerateProof),
        _ => unreachable!("Expected DegenerateProof error"),
    }
}

#[test]
fn test_degenerate_proof_c_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let resource_id = [7u8; 32];

    let mut proof_a = [0u8; 64];
    proof_a[0] = 1;
    let mut proof_b = [0u8; 128];
    proof_b[0] = 1;
    // proof.c is all zeros → degenerate
    let proof_c = [0u8; 64];
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

    let result = client.try_verify_access(&request);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::DegenerateProof),
        _ => unreachable!("Expected DegenerateProof error"),
    }
}
