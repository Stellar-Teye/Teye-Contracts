//! # Proof Soundness Tests — ZK Verifier
//!
//! Verifies that every category of malformed or forged proof is unconditionally
//! rejected by `Bn254Verifier::validate_proof_components` and by the
//! `ZkVerifierContract::verify_access` entry-point.
//!
//! Covers issue #480: "Implement tests using a library of intentionally
//! malformed ZK-SNARK proofs."
#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Vec};
use zk_verifier::vk::{G1Point, G2Point, VerificationKey};
use zk_verifier::{
    AccessRequest, Bn254Verifier, ContractError, Proof, ProofValidationError, ZkVerifier,
    ZkVerifierContract, ZkVerifierContractClient,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn zero32(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

fn ones32(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0xFF_u8; 32])
}

fn nonzero32(env: &Env) -> BytesN<32> {
    let mut b = [0u8; 32];
    b[0] = 1;
    BytesN::from_array(env, &b)
}

/// Build a G1 point with the given x and y bytes.
fn g1(env: &Env, x: BytesN<32>, y: BytesN<32>) -> G1Point {
    G1Point { x, y }
}

/// Build a G2 point where every limb equals `limb`.
fn g2_uniform(env: &Env, limb: BytesN<32>) -> G2Point {
    G2Point {
        x: (limb.clone(), limb.clone()),
        y: (limb.clone(), limb.clone()),
    }
}

/// A "valid" G2 point: all limbs non-zero and non-saturated.
fn valid_g2(env: &Env) -> G2Point {
    g2_uniform(env, nonzero32(env))
}

/// A structurally valid proof that passes `validate_proof_components`.
fn valid_proof(env: &Env) -> (Proof, Vec<BytesN<32>>) {
    let nz = nonzero32(env);
    let proof = Proof {
        a: g1(env, nz.clone(), nz.clone()),
        b: valid_g2(env),
        c: g1(env, nz.clone(), nz.clone()),
    };
    let mut inputs = Vec::new(env);
    inputs.push_back(nz);
    (proof, inputs)
}

/// A minimal contract client with a deployed, initialized contract.
fn setup_client(env: &Env) -> (ZkVerifierContractClient<'static>, Address, Address) {
    env.mock_all_auths();
    let id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(env, &id);
    let admin = Address::generate(env);
    client.initialize(&admin);

    // Set a trivial VK so the contract path up to proof verification is
    // reachable. The mock verifier only checks a.x[0]==1, c.x[0]==1,
    // public_inputs[0][0]==1 — we don't need a real pairing key.
    let z = zero32(env);
    let g1z = G1Point {
        x: z.clone(),
        y: z.clone(),
    };
    let g2z = G2Point {
        x: (z.clone(), z.clone()),
        y: (z.clone(), z.clone()),
    };
    let mut ic = Vec::new(env);
    ic.push_back(g1z.clone());
    client.set_verification_key(
        &admin,
        &VerificationKey {
            alpha_g1: g1z.clone(),
            beta_g2: g2z.clone(),
            gamma_g2: g2z.clone(),
            delta_g2: g2z.clone(),
            ic,
        },
    );

    let user = Address::generate(env);
    (client, admin, user)
}

fn make_request(
    env: &Env,
    user: Address,
    proof: Proof,
    public_inputs: Vec<BytesN<32>>,
) -> AccessRequest {
    let mut rid = [0u8; 32];
    rid[0] = 42;
    AccessRequest {
        user,
        resource_id: BytesN::from_array(env, &rid),
        proof,
        public_inputs,
        expires_at: 0,
        nonce: 0,
    }
}

// ── validate_proof_components unit tests ─────────────────────────────────────

#[test]
fn test_all_zero_a_returns_zeroed_component() {
    let env = Env::default();
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, zero32(&env), zero32(&env)),
        b: valid_g2(&env),
        c: g1(&env, nz.clone(), nz.clone()),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::ZeroedComponent),
        "all-zero a must return ZeroedComponent"
    );
}

#[test]
fn test_all_ff_a_returns_oversized_component() {
    let env = Env::default();
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, ones32(&env), ones32(&env)),
        b: valid_g2(&env),
        c: g1(&env, nz.clone(), nz.clone()),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::OversizedComponent),
        "saturated a must return OversizedComponent"
    );
}

#[test]
fn test_zero_ax_returns_malformed_g1_point_a() {
    let env = Env::default();
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, zero32(&env), nz.clone()), // x is zero, y is non-zero
        b: valid_g2(&env),
        c: g1(&env, nz.clone(), nz.clone()),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::MalformedG1PointA),
    );
}

#[test]
fn test_zero_ay_returns_malformed_g1_point_a() {
    let env = Env::default();
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, nz.clone(), zero32(&env)), // y is zero
        b: valid_g2(&env),
        c: g1(&env, nz.clone(), nz.clone()),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::MalformedG1PointA),
    );
}

#[test]
fn test_all_zero_b_returns_zeroed_component() {
    let env = Env::default();
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, nz.clone(), nz.clone()),
        b: g2_uniform(&env, zero32(&env)),
        c: g1(&env, nz.clone(), nz.clone()),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::ZeroedComponent),
    );
}

#[test]
fn test_all_ff_b_returns_oversized_component() {
    let env = Env::default();
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, nz.clone(), nz.clone()),
        b: g2_uniform(&env, ones32(&env)),
        c: g1(&env, nz.clone(), nz.clone()),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::OversizedComponent),
    );
}

#[test]
fn test_zero_limb_in_b_returns_malformed_g2_point() {
    let env = Env::default();
    let nz = nonzero32(&env);
    // x.0 is non-zero but x.1 is all zeros → malformed G2
    let proof = Proof {
        a: g1(&env, nz.clone(), nz.clone()),
        b: G2Point {
            x: (nz.clone(), zero32(&env)),
            y: (nz.clone(), nz.clone()),
        },
        c: g1(&env, nz.clone(), nz.clone()),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::MalformedG2Point),
    );
}

#[test]
fn test_all_zero_c_returns_zeroed_component() {
    let env = Env::default();
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, nz.clone(), nz.clone()),
        b: valid_g2(&env),
        c: g1(&env, zero32(&env), zero32(&env)),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::ZeroedComponent),
    );
}

#[test]
fn test_all_ff_c_returns_oversized_component() {
    let env = Env::default();
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, nz.clone(), nz.clone()),
        b: valid_g2(&env),
        c: g1(&env, ones32(&env), ones32(&env)),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::OversizedComponent),
    );
}

#[test]
fn test_zero_cx_returns_malformed_g1_point_c() {
    let env = Env::default();
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, nz.clone(), nz.clone()),
        b: valid_g2(&env),
        c: g1(&env, zero32(&env), nz.clone()),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::MalformedG1PointC),
    );
}

#[test]
fn test_zero_cy_returns_malformed_g1_point_c() {
    let env = Env::default();
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, nz.clone(), nz.clone()),
        b: valid_g2(&env),
        c: g1(&env, nz.clone(), zero32(&env)),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::MalformedG1PointC),
    );
}

#[test]
fn test_empty_public_inputs_rejected() {
    let env = Env::default();
    let nz = nonzero32(&env);
    let (proof, _) = valid_proof(&env);
    let empty: Vec<BytesN<32>> = Vec::new(&env);
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &empty),
        Err(ProofValidationError::EmptyPublicInputs),
    );
}

#[test]
fn test_zeroed_public_input_rejected() {
    let env = Env::default();
    let (proof, _) = valid_proof(&env);
    let mut inputs = Vec::new(&env);
    inputs.push_back(zero32(&env));
    assert_eq!(
        Bn254Verifier::validate_proof_components(&proof, &inputs),
        Err(ProofValidationError::ZeroedPublicInput),
    );
}

#[test]
fn test_valid_components_pass_validation() {
    let env = Env::default();
    let (proof, inputs) = valid_proof(&env);
    assert!(
        Bn254Verifier::validate_proof_components(&proof, &inputs).is_ok(),
        "structurally valid proof must pass component validation"
    );
}

// ── Contract-level soundness: verify_access rejects malformed proofs ─────────

#[test]
fn test_contract_rejects_zeroed_proof_a() {
    let env = Env::default();
    let (client, _, user) = setup_client(&env);
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, zero32(&env), zero32(&env)),
        b: valid_g2(&env),
        c: g1(&env, nz.clone(), nz.clone()),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    let req = make_request(&env, user, proof, inputs);
    let result = client.try_verify_access(&req);
    assert!(
        result.is_err(),
        "zeroed proof.a must be rejected by contract"
    );
}

#[test]
fn test_contract_rejects_oversized_proof_b() {
    let env = Env::default();
    let (client, _, user) = setup_client(&env);
    let nz = nonzero32(&env);
    let proof = Proof {
        a: g1(&env, nz.clone(), nz.clone()),
        b: g2_uniform(&env, ones32(&env)),
        c: g1(&env, nz.clone(), nz.clone()),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(nz);
    let req = make_request(&env, user, proof, inputs);
    let result = client.try_verify_access(&req);
    assert!(
        result.is_err(),
        "saturated proof.b must be rejected by contract"
    );
}

#[test]
fn test_contract_rejects_empty_public_inputs_via_validate_request() {
    let env = Env::default();
    let (client, _, user) = setup_client(&env);
    let (proof, _) = valid_proof(&env);
    let empty: Vec<BytesN<32>> = Vec::new(&env);
    let req = make_request(&env, user, proof, empty);
    let result = client.try_verify_access(&req);
    match result {
        Err(Ok(e)) => assert!(
            matches!(e, ContractError::EmptyPublicInputs),
            "expected EmptyPublicInputs, got {e:?}"
        ),
        _ => panic!("expected contract error for empty public inputs"),
    }
}

#[test]
fn test_contract_rejects_too_many_public_inputs() {
    let env = Env::default();
    let (client, _, user) = setup_client(&env);
    let (proof, _) = valid_proof(&env);
    let mut inputs = Vec::new(&env);
    let nz = nonzero32(&env);
    for _ in 0..17 {
        inputs.push_back(nz.clone());
    }
    let req = make_request(&env, user, proof, inputs);
    let result = client.try_verify_access(&req);
    match result {
        Err(Ok(e)) => assert!(
            matches!(e, ContractError::TooManyPublicInputs),
            "expected TooManyPublicInputs, got {e:?}"
        ),
        _ => panic!("expected TooManyPublicInputs error"),
    }
}

#[test]
fn test_contract_rejects_zeroed_public_input() {
    let env = Env::default();
    let (client, _, user) = setup_client(&env);
    let nz = nonzero32(&env);
    // Structurally valid proof but zeroed public input
    let proof = Proof {
        a: g1(&env, nz.clone(), nz.clone()),
        b: valid_g2(&env),
        c: g1(&env, nz.clone(), nz.clone()),
    };
    let mut inputs = Vec::new(&env);
    inputs.push_back(zero32(&env));
    let req = make_request(&env, user, proof, inputs);
    let result = client.try_verify_access(&req);
    assert!(result.is_err(), "zeroed public input must be rejected");
}

// ── Recursive proof batch: first invalid proof short-circuits ─────────────────

#[test]
fn test_recursive_batch_rejects_on_first_invalid_proof() {
    let env = Env::default();
    env.mock_all_auths();
    let vk_id = env.register(ZkVerifierContract, ());
    let vk_client = ZkVerifierContractClient::new(&env, &vk_id);
    let admin = Address::generate(&env);
    vk_client.initialize(&admin);

    let nz = nonzero32(&env);
    let z = zero32(&env);

    let (valid, inputs_valid) = valid_proof(&env);
    let invalid = Proof {
        a: g1(&env, z.clone(), z.clone()), // zeroed — fails validation
        b: valid_g2(&env),
        c: g1(&env, nz.clone(), nz.clone()),
    };
    let mut inputs_invalid = Vec::new(&env);
    inputs_invalid.push_back(nz.clone());

    let vk = {
        let g1z = G1Point {
            x: z.clone(),
            y: z.clone(),
        };
        let g2z = G2Point {
            x: (z.clone(), z.clone()),
            y: (z.clone(), z.clone()),
        };
        let mut ic = Vec::new(&env);
        ic.push_back(g1z.clone());
        VerificationKey {
            alpha_g1: g1z.clone(),
            beta_g2: g2z.clone(),
            gamma_g2: g2z.clone(),
            delta_g2: g2z.clone(),
            ic,
        }
    };

    let mut proofs: Vec<Proof> = Vec::new(&env);
    proofs.push_back(invalid);
    proofs.push_back(valid);

    let mut batched: Vec<Vec<BytesN<32>>> = Vec::new(&env);
    batched.push_back(inputs_invalid);
    batched.push_back(inputs_valid);

    let result = Bn254Verifier::verify_recursive_proof(&env, &vk, &proofs, &batched);
    assert!(!result, "batch must fail when first proof is invalid");
}
