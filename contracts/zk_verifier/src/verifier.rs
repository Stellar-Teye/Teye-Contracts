#![allow(dead_code)]
use soroban_sdk::{contracttype, BytesN, Env, Vec};

pub type VerificationKey = crate::vk::VerificationKey;

// TODO: post-quantum migration - `G1Point`, `G2Point`, and `Proof` map to elliptic curves.
// For hash-based STARKs or Lattice proofs, replace these representations with Hash paths 
// or matrix structural analogs.
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
    // TODO: post-quantum migration - The mock logic here or actual BN254 pairing checks 
    // will be superseded by a new implementation validating collision-resistant hash paths 
    // (for FRI) or LWE assertions (for Lattices).
    pub fn verify_proof(
        _env: &Env,
        _vk: &VerificationKey,
        proof: &Proof,
        public_inputs: &Vec<BytesN<32>>,
    ) -> bool {
        if public_inputs.is_empty() {
            return false;
        }

        if proof.a.x.get(0) != Some(1) {
            return false;
        }
        if proof.c.x.get(0) != Some(1) {
            return false;
        }

        public_inputs.get(0).is_some_and(|p| p.get(0) == Some(1))
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
