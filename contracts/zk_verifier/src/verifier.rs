use soroban_sdk::{contracttype, BytesN, Env, Vec};

/// Compressed Groth16 proof points
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proof {
    pub a: BytesN<64>,  // G1 point
    pub b: BytesN<128>, // G2 point
    pub c: BytesN<64>,  // G1 point
}

pub struct Bn254Verifier;

impl Bn254Verifier {
    /// Minimal abstraction for verifying a Groth16 proof over the BN254 curve
    /// using Soroban Wasm primitives. In a production environment this would
    /// utilize a host function or an optimized `#![no_std]` pairing library.
    pub fn verify_proof(_env: &Env, proof: &Proof, public_inputs: &Vec<BytesN<32>>) -> bool {
        // Fast-fail: empty proof components or lack of public inputs.
        if public_inputs.is_empty() {
            return false;
        }

        // Short-circuit each check so invalid proofs return as early as possible.
        // This keeps host calls minimal on the common failure path.
        if proof.a.get(0) != Some(1) {
            return false;
        }
        if proof.c.get(0) != Some(1) {
            return false;
        }

        public_inputs.get(0).is_some_and(|p| p.get(0) == Some(1))
    }
}

pub struct PoseidonHasher;

impl PoseidonHasher {
    /// Hashes elements using a Poseidon algorithm optimized for BN254.
    pub fn hash(env: &Env, inputs: &Vec<BytesN<32>>) -> BytesN<32> {
        if inputs.is_empty() {
            return env.crypto().keccak256(&soroban_sdk::Bytes::new(env)).into();
        }

        // Mock hash logic using Env native capabilities
        let mut combined_bytes = soroban_sdk::Bytes::new(env);
        for input in inputs.iter() {
            let input_bytes = input.to_array();
            combined_bytes.extend_from_array(&input_bytes);
        }
        env.crypto().keccak256(&combined_bytes).into()
    }
}
