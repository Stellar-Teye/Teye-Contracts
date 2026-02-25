//! Integration tests for nonce-based replay protection across
//! zk_verifier and identity contracts.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Vec};
use zk_verifier::verifier::{G1Point, G2Point, Proof};
use zk_verifier::{AccessRequest, ContractError, ZkVerifierContract, ZkVerifierContractClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup(env: &Env) -> (ZkVerifierContractClient<'static>, Address, Address) {
    env.mock_all_auths();
    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let user = Address::generate(env);
    client.initialize(&admin);
    (client, admin, user)
}

/// Build a proof that passes the mock verifier:
/// a.x[0]==1, c.x[0]==1, public_inputs[0][0]==1
fn valid_proof_and_inputs(env: &Env) -> (Proof, Vec<BytesN<32>>) {
    let mut ax = [0u8; 32];
    ax[0] = 1;
    let mut cx = [0u8; 32];
    cx[0] = 1;
    let mut pi = [0u8; 32];
    pi[0] = 1;
    // G2: all limbs non-zero so validate_proof_components passes
    let bx = [1u8; 32];

    let proof = Proof {
        a: G1Point {
            x: BytesN::from_array(env, &ax),
            y: BytesN::from_array(env, &ax),
        },
        b: G2Point {
            x: (BytesN::from_array(env, &bx), BytesN::from_array(env, &bx)),
            y: (BytesN::from_array(env, &bx), BytesN::from_array(env, &bx)),
        },
        c: G1Point {
            x: BytesN::from_array(env, &cx),
            y: BytesN::from_array(env, &ax),
        },
    };
    let mut inputs = Vec::new(env);
    inputs.push_back(BytesN::from_array(env, &pi));
    (proof, inputs)
}

fn make_request(
    env: &Env,
    user: Address,
    nonce: u64,
    proof: Proof,
    inputs: Vec<BytesN<32>>,
) -> AccessRequest {
    AccessRequest {
        user,
        resource_id: BytesN::from_array(env, &[1u8; 32]),
        proof,
        public_inputs: inputs,
        nonce,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn new_user_nonce_is_zero() {
    let env = Env::default();
    let (client, _, user) = setup(&env);
    assert_eq!(client.get_nonce(&user), 0u64);
}

#[test]
fn correct_nonce_accepted_and_incremented() {
    let env = Env::default();
    let (client, _, user) = setup(&env);
    let (proof, inputs) = valid_proof_and_inputs(&env);

    let req = make_request(&env, user.clone(), 0, proof, inputs);
    let result = client.try_verify_access(&req);
    assert!(result.is_ok(), "nonce=0 should be accepted");
    assert_eq!(
        client.get_nonce(&user),
        1u64,
        "nonce must advance after success"
    );
}

#[test]
fn replay_with_same_nonce_is_rejected() {
    let env = Env::default();
    let (client, _, user) = setup(&env);
    let (proof1, inputs1) = valid_proof_and_inputs(&env);
    let (proof2, inputs2) = valid_proof_and_inputs(&env);

    // First call succeeds with nonce=0.
    client
        .try_verify_access(&make_request(&env, user.clone(), 0, proof1, inputs1))
        .unwrap();

    // Replay: same nonce=0 must be rejected.
    let replay = client.try_verify_access(&make_request(&env, user.clone(), 0, proof2, inputs2));
    assert!(replay.is_err());
    assert_eq!(
        replay.unwrap_err(),
        Ok(ContractError::InvalidNonce),
        "replay must return InvalidNonce"
    );
}

#[test]
fn nonce_advances_monotonically() {
    let env = Env::default();
    let (client, _, user) = setup(&env);

    for expected_nonce in 0u64..5 {
        let (proof, inputs) = valid_proof_and_inputs(&env);
        let req = make_request(&env, user.clone(), expected_nonce, proof, inputs);
        client
            .try_verify_access(&req)
            .unwrap_or_else(|_| panic!("nonce={expected_nonce} should succeed"));
        assert_eq!(client.get_nonce(&user), expected_nonce + 1);
    }
}

#[test]
fn future_nonce_with_gap_is_rejected() {
    let env = Env::default();
    let (client, _, user) = setup(&env);
    let (proof, inputs) = valid_proof_and_inputs(&env);

    // Skip nonce=0 and try nonce=5 — must be rejected.
    let req = make_request(&env, user.clone(), 5, proof, inputs);
    let result = client.try_verify_access(&req);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Ok(ContractError::InvalidNonce));

    // Counter must NOT have advanced.
    assert_eq!(client.get_nonce(&user), 0u64);
}

#[test]
fn different_users_have_independent_nonces() {
    let env = Env::default();
    let (client, _, alice) = setup(&env);
    let bob = Address::generate(&env);

    // Advance Alice's nonce twice.
    for n in 0u64..2 {
        let (proof, inputs) = valid_proof_and_inputs(&env);
        client
            .try_verify_access(&make_request(&env, alice.clone(), n, proof, inputs))
            .unwrap();
    }

    assert_eq!(client.get_nonce(&alice), 2u64);
    // Bob starts at zero — unaffected by Alice's calls.
    assert_eq!(client.get_nonce(&bob), 0u64);

    // Bob's first call with nonce=0 must succeed.
    let (proof, inputs) = valid_proof_and_inputs(&env);
    let result = client.try_verify_access(&make_request(&env, bob.clone(), 0, proof, inputs));
    assert!(result.is_ok());
    assert_eq!(client.get_nonce(&bob), 1u64);
}

#[test]
fn failed_validation_does_not_advance_nonce() {
    let env = Env::default();
    let (client, _, user) = setup(&env);

    // Submit a completely zeroed proof — should fail validate_request.
    let z = [0u8; 32];
    let bad_proof = Proof {
        a: G1Point {
            x: BytesN::from_array(&env, &z),
            y: BytesN::from_array(&env, &z),
        },
        b: G2Point {
            x: (BytesN::from_array(&env, &z), BytesN::from_array(&env, &z)),
            y: (BytesN::from_array(&env, &z), BytesN::from_array(&env, &z)),
        },
        c: G1Point {
            x: BytesN::from_array(&env, &z),
            y: BytesN::from_array(&env, &z),
        },
    };
    let mut inputs = Vec::new(&env);
    let mut pi = [0u8; 32];
    pi[0] = 1;
    inputs.push_back(BytesN::from_array(&env, &pi));

    let req = make_request(&env, user.clone(), 0, bad_proof, inputs);
    let result = client.try_verify_access(&req);
    assert!(result.is_err(), "degenerate proof must be rejected");

    // Nonce must not have advanced because validate_request fires before nonce check.
    assert_eq!(client.get_nonce(&user), 0u64);
}
