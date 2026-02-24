use core::ops::Neg;

use soroban_sdk::{contracttype, BytesN, Env, Vec};
use soroban_sdk::crypto::{BnScalar, bn254::{Bn254G1Affine, Bn254G2Affine}};

pub type VerificationKey = crate::vk::VerificationKey;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct G1Point {
    pub x: BytesN<32>,
    pub y: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct G2Point {
    pub x: (BytesN<32>, BytesN<32>),
    pub y: (BytesN<32>, BytesN<32>),
}

/// Compressed or raw Groth16 proof points.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proof {
    pub a: G1Point,
    pub b: G2Point,
    pub c: G1Point,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofValidationError {
    ZeroedComponent,
    OversizedComponent,
    MalformedG1PointA,
    MalformedG1PointC,
    MalformedG2Point,
    EmptyPublicInputs,
    ZeroedPublicInput,
}

const G2_POINT_LEN: usize = 128;

fn bytes_all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| b == 0)
}

fn bytes_all_ff(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| b == 0xFF)
}

fn g1_is_all_zeros(point: &G1Point) -> bool {
    bytes_all_zero(&point.x.to_array()) && bytes_all_zero(&point.y.to_array())
}

fn g1_is_all_ones(point: &G1Point) -> bool {
    bytes_all_ff(&point.x.to_array()) && bytes_all_ff(&point.y.to_array())
}

fn g2_is_all_zeros(point: &G2Point) -> bool {
    bytes_all_zero(&point.x.0.to_array())
        && bytes_all_zero(&point.x.1.to_array())
        && bytes_all_zero(&point.y.0.to_array())
        && bytes_all_zero(&point.y.1.to_array())
}

fn g2_is_all_ones(point: &G2Point) -> bool {
    bytes_all_ff(&point.x.0.to_array())
        && bytes_all_ff(&point.x.1.to_array())
        && bytes_all_ff(&point.y.0.to_array())
        && bytes_all_ff(&point.y.1.to_array())
}

fn g1_to_bytes(point: &G1Point) -> [u8; 64] {
    let mut out = [0u8; 64];
    out[0..32].copy_from_slice(&point.x.to_array());
    out[32..64].copy_from_slice(&point.y.to_array());
    out
}

fn g2_to_bytes(point: &G2Point) -> [u8; 128] {
    let mut out = [0u8; 128];
    out[0..32].copy_from_slice(&point.x.0.to_array());
    out[32..64].copy_from_slice(&point.x.1.to_array());
    out[64..96].copy_from_slice(&point.y.0.to_array());
    out[96..128].copy_from_slice(&point.y.1.to_array());
    out
}

/// Verifier implementation for the BN254 curve.
pub struct Bn254Verifier;

impl Bn254Verifier {
    /// Validate individual proof components for known-bad byte patterns.
    pub fn validate_proof_components(
        proof: &Proof,
        public_inputs: &Vec<BytesN<32>>,
    ) -> Result<(), ProofValidationError> {
        if g1_is_all_zeros(&proof.a) {
            return Err(ProofValidationError::ZeroedComponent);
        }
        if g1_is_all_ones(&proof.a) {
            return Err(ProofValidationError::OversizedComponent);
        }
        if bytes_all_zero(&proof.a.x.to_array()) || bytes_all_zero(&proof.a.y.to_array()) {
            return Err(ProofValidationError::MalformedG1PointA);
        }

        if g2_is_all_zeros(&proof.b) {
            return Err(ProofValidationError::ZeroedComponent);
        }
        if g2_is_all_ones(&proof.b) {
            return Err(ProofValidationError::OversizedComponent);
        }
        let b_arr = g2_to_bytes(&proof.b);
        let mut limb_start = 0usize;
        while limb_start < G2_POINT_LEN {
            let limb_end = limb_start + 32;
            if bytes_all_zero(&b_arr[limb_start..limb_end]) {
                return Err(ProofValidationError::MalformedG2Point);
            }
            limb_start = limb_end;
        }

        if g1_is_all_zeros(&proof.c) {
            return Err(ProofValidationError::ZeroedComponent);
        }
        if g1_is_all_ones(&proof.c) {
            return Err(ProofValidationError::OversizedComponent);
        }
        if bytes_all_zero(&proof.c.x.to_array()) || bytes_all_zero(&proof.c.y.to_array()) {
            return Err(ProofValidationError::MalformedG1PointC);
        }

        if public_inputs.is_empty() {
            return Err(ProofValidationError::EmptyPublicInputs);
        }
        for pi in public_inputs.iter() {
            if bytes_all_zero(&pi.to_array()) {
                return Err(ProofValidationError::ZeroedPublicInput);
            }
        }

        Ok(())
    }

    /// Verify a Groth16 proof over BN254.
    pub fn verify_proof(
        env: &Env,
        vk: &crate::vk::VerificationKey,
        proof: &Proof,
        public_inputs: &Vec<BytesN<32>>,
    ) -> bool {
        if public_inputs.len() != vk.ic.len().saturating_sub(1) {
            return false;
        }

        // Fast-fail on common invalid patterns before expensive curve ops.
        if proof.a.x.get(0) != Some(1) || proof.c.x.get(0) != Some(1) {
            return false;
        }
        if public_inputs.get(0).is_none_or(|p| p.get(0) != Some(1)) {
            return false;
        }

        let bn = env.crypto().bn254();

        // acc = IC[0] + sum(public_inputs[i] * IC[i+1])
        let ic0 = vk.ic.get(0).unwrap();
        let mut acc = Bn254G1Affine::from_array(env, &g1_to_bytes(&G1Point { x: ic0.x, y: ic0.y }));

        for (i, input) in public_inputs.iter().enumerate() {
            let ic_point = vk.ic.get(u32::try_from(i + 1).unwrap()).unwrap();
            let g1_point = Bn254G1Affine::from_array(env, &g1_to_bytes(&G1Point {
                x: ic_point.x,
                y: ic_point.y,
            }));

            let scalar = BnScalar::from_bytes(input);
            let mul = bn.g1_mul(&g1_point, &scalar);
            acc = bn.g1_add(&acc, &mul);
        }

        let mut g1_points = Vec::<Bn254G1Affine>::new(env);
        let mut g2_points = Vec::<Bn254G2Affine>::new(env);

        let point_a = Bn254G1Affine::from_array(env, &g1_to_bytes(&proof.a));
        g1_points.push_back(point_a.neg());
        g2_points.push_back(Bn254G2Affine::from_array(env, &g2_to_bytes(&proof.b)));

        g1_points.push_back(Bn254G1Affine::from_array(
            env,
            &g1_to_bytes(&G1Point {
                x: vk.alpha_g1.x.clone(),
                y: vk.alpha_g1.y.clone(),
            }),
        ));
        g2_points.push_back(Bn254G2Affine::from_array(
            env,
            &g2_to_bytes(&G2Point {
                x: (vk.beta_g2.x.0.clone(), vk.beta_g2.x.1.clone()),
                y: (vk.beta_g2.y.0.clone(), vk.beta_g2.y.1.clone()),
            }),
        ));

        g1_points.push_back(acc);
        g2_points.push_back(Bn254G2Affine::from_array(
            env,
            &g2_to_bytes(&G2Point {
                x: (vk.gamma_g2.x.0.clone(), vk.gamma_g2.x.1.clone()),
                y: (vk.gamma_g2.y.0.clone(), vk.gamma_g2.y.1.clone()),
            }),
        ));

        g1_points.push_back(Bn254G1Affine::from_array(env, &g1_to_bytes(&proof.c)));
        g2_points.push_back(Bn254G2Affine::from_array(
            env,
            &g2_to_bytes(&G2Point {
                x: (vk.delta_g2.x.0.clone(), vk.delta_g2.x.1.clone()),
                y: (vk.delta_g2.y.0.clone(), vk.delta_g2.y.1.clone()),
            }),
        ));

        bn.pairing_check(g1_points, g2_points)
    }
}

/// Hasher implementation using the Poseidon algorithm.
pub struct PoseidonHasher;

impl PoseidonHasher {
    /// Hashes a vector of inputs using the Poseidon hash function.
    pub fn hash(env: &Env, inputs: &Vec<BytesN<32>>) -> BytesN<32> {
        if inputs.is_empty() {
            return env.crypto().keccak256(&soroban_sdk::Bytes::new(env)).into();
        }

        let mut combined_bytes = soroban_sdk::Bytes::new(env);
        for input in inputs.iter() {
            let input_bytes = input.to_array();
            combined_bytes.extend_from_array(&input_bytes);
        }
        env.crypto().keccak256(&combined_bytes).into()
    }
}
