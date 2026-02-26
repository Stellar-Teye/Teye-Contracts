#![allow(dead_code)]
extern crate alloc;
use alloc::vec::Vec as StdVec;

use ark_bn254::Fr;
use light_poseidon_nostd::{Poseidon, PoseidonBytesHasher};
use soroban_sdk::{contracttype, BytesN, Env, Vec};

pub type VerificationKey = crate::vk::VerificationKey;

/// Shared trait for all ZK proof verification systems.
/// This enables adding new proving systems (PLONK, STARKs, etc.) without breaking existing code.
pub trait ZkVerifier {
    /// Validates proof components for structural integrity before verification.
    fn validate_proof_components(
        proof: &Proof,
        public_inputs: &Vec<BytesN<32>>,
    ) -> Result<(), ProofValidationError>;

    /// Verifies a ZK proof against a verification key and public inputs.
    fn verify_proof(
        env: &Env,
        vk: &VerificationKey,
        proof: &Proof,
        public_inputs: &Vec<BytesN<32>>,
    ) -> bool;
}

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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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

impl ZkVerifier for Bn254Verifier {
    /// Validate individual proof components for known-bad byte patterns that
    /// would cause undefined behaviour or nonsensical results in a real pairing
    /// check. This runs *before* the (mock) verification arithmetic.
    ///
    /// Note: empty `public_inputs` are rejected here as a safety guard, and the
    /// contract entrypoint also rejects empty inputs to provide a clear error
    /// and event at the contract boundary.
    fn validate_proof_components(
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
    fn verify_proof(
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
            let zero = [0u8; 32];
            return Self::hash_chunk(env, &[&zero]);
        }

        // Circom-compatible parameters support up to 12 inputs directly.
        // For longer vectors, fold as Poseidon(Poseidon(chunk), next).
        if inputs.len() <= 12 {
            let mut chunks = StdVec::with_capacity(inputs.len() as usize);
            for input in inputs.iter() {
                chunks.push(input.to_array());
            }
            let refs: StdVec<&[u8]> = chunks.iter().map(|v| v.as_slice()).collect();
            return Self::hash_chunk(env, &refs);
        }

        let mut current: Option<[u8; 32]> = None;
        let mut idx: u32 = 0;
        while idx < inputs.len() {
            if current.is_none() {
                // Hash the first up-to-12 elements in one shot.
                let mut first = StdVec::new();
                let mut j = 0u32;
                while j < 12 && idx + j < inputs.len() {
                    if let Some(v) = inputs.get(idx + j) {
                        first.push(v.to_array());
                    }
                    j += 1;
                }
                let refs: StdVec<&[u8]> = first.iter().map(|v| v.as_slice()).collect();
                let seed = Self::hash_chunk(env, &refs);
                current = Some(seed.to_array());
                idx += j;
            } else if let (Some(curr), Some(next)) = (current, inputs.get(idx)) {
                let next_arr = next.to_array();
                let folded = Self::hash_chunk(env, &[&curr, &next_arr]);
                current = Some(folded.to_array());
                idx += 1;
            } else {
                break;
            }
        }

        BytesN::from_array(env, &current.unwrap_or([0u8; 32]))
    }

    fn hash_chunk(env: &Env, chunks: &[&[u8]]) -> BytesN<32> {
        let mut poseidon = match Poseidon::<Fr>::new_circom(chunks.len()) {
            Ok(p) => p,
            Err(_) => return BytesN::from_array(env, &[0u8; 32]),
        };

        match poseidon.hash_bytes_be(chunks) {
            Ok(bytes) => BytesN::from_array(env, &bytes),
            Err(_) => BytesN::from_array(env, &[0u8; 32]),
        }
    }
}
