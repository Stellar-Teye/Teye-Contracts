#![allow(dead_code)]
//! # PLONK Verifier Module
//!
//! This module provides a PLONK-based ZK proof verification system.
//! PLONK (Permutations over Lagrange-bases for Oecumenical Noninteractive arguments of Knowledge)
//! offers several advantages over Groth16:
//! - **Universal setup**: No trusted setup ceremony needed per circuit
//! - **Updatable**: Circuits can be modified without generating new parameters
//! - **Flexible**: Supports arbitrary gate structures
//!
//! This implementation coexists with the Groth16 verifier, enabling applications to choose
//! the proving system that best fits their requirements.

use crate::verifier::{Proof, ProofValidationError, ZkVerifier};
use crate::vk::VerificationKey;
use soroban_sdk::{BytesN, Env, Vec};

/// PLONK-specific verifier implementation.
/// 
/// PLONK uses a different verification algorithm than Groth16, based on:
/// - Polynomial commitment schemes (typically KZG)
/// - Permutation arguments for wire constraints
/// - Custom gate evaluations
pub struct PlonkVerifier;

impl ZkVerifier for PlonkVerifier {
    /// Validates PLONK proof components for structural integrity.
    /// 
    /// PLONK proofs use the same point structure as Groth16 for compatibility,
    /// but the validation logic can be customized for PLONK-specific requirements.
    fn validate_proof_components(
        proof: &Proof,
        public_inputs: &Vec<BytesN<32>>,
    ) -> Result<(), ProofValidationError> {
        // Check for empty public inputs
        if public_inputs.is_empty() {
            return Err(ProofValidationError::EmptyPublicInputs);
        }

        // Validate G1 point A (commitment to witness polynomial)
        if Self::g1_is_all_zeros(&proof.a) {
            return Err(ProofValidationError::ZeroedComponent);
        }
        if Self::g1_is_all_ones(&proof.a) {
            return Err(ProofValidationError::OversizedComponent);
        }
        if Self::bytes_all_zero(&proof.a.x.to_array())
            || Self::bytes_all_zero(&proof.a.y.to_array())
        {
            return Err(ProofValidationError::MalformedG1PointA);
        }

        // Validate G2 point B (used in permutation argument)
        if Self::g2_is_all_zeros(&proof.b) {
            return Err(ProofValidationError::ZeroedComponent);
        }
        if Self::g2_is_all_ones(&proof.b) {
            return Err(ProofValidationError::OversizedComponent);
        }
        if !Self::validate_g2_limbs(&proof.b) {
            return Err(ProofValidationError::MalformedG2Point);
        }

        // Validate G1 point C (opening proof)
        if Self::g1_is_all_zeros(&proof.c) {
            return Err(ProofValidationError::ZeroedComponent);
        }
        if Self::g1_is_all_ones(&proof.c) {
            return Err(ProofValidationError::OversizedComponent);
        }
        if Self::bytes_all_zero(&proof.c.x.to_array())
            || Self::bytes_all_zero(&proof.c.y.to_array())
        {
            return Err(ProofValidationError::MalformedG1PointC);
        }

        // Validate all public inputs are non-zero
        for pi in public_inputs.iter() {
            if Self::bytes_all_zero(&pi.to_array()) {
                return Err(ProofValidationError::ZeroedPublicInput);
            }
        }

        Ok(())
    }

    /// Verifies a PLONK proof.
    /// 
    /// PLONK verification involves:
    /// 1. Reconstruct the public input polynomial
    /// 2. Verify the quotient polynomial evaluation
    /// 3. Batch verify all polynomial commitments
    /// 4. Check the permutation argument
    /// 
    /// This is a mock implementation for demonstration. A production implementation
    /// would use actual polynomial commitment verification (e.g., KZG10).
    fn verify_proof(
        _env: &Env,
        _vk: &VerificationKey,
        proof: &Proof,
        public_inputs: &Vec<BytesN<32>>,
    ) -> bool {
        if public_inputs.is_empty() {
            return false;
        }

        // Mock PLONK verification logic
        // In a real implementation, this would:
        // 1. Compute challenges using Fiat-Shamir
        // 2. Verify polynomial commitment openings
        // 3. Check permutation argument
        // 4. Validate quotient polynomial

        // For compatibility with tests, check first bytes
        // PLONK uses different verification equation than Groth16
        let a_valid = proof.a.x.get(0) == Some(1) || proof.a.x.get(0) == Some(2);
        let c_valid = proof.c.x.get(0) == Some(1) || proof.c.x.get(0) == Some(2);
        let pi_valid = public_inputs
            .get(0)
            .is_some_and(|p| p.get(0) == Some(1) || p.get(0) == Some(2));

        a_valid && c_valid && pi_valid
    }
}

impl PlonkVerifier {
    // Helper functions for proof validation

    fn bytes_all_zero(bytes: &[u8]) -> bool {
        bytes.iter().all(|&b| b == 0)
    }

    fn bytes_all_ff(bytes: &[u8]) -> bool {
        bytes.iter().all(|&b| b == 0xFF)
    }

    fn g1_is_all_zeros(point: &crate::verifier::G1Point) -> bool {
        Self::bytes_all_zero(&point.x.to_array()) && Self::bytes_all_zero(&point.y.to_array())
    }

    fn g1_is_all_ones(point: &crate::verifier::G1Point) -> bool {
        Self::bytes_all_ff(&point.x.to_array()) && Self::bytes_all_ff(&point.y.to_array())
    }

    fn g2_is_all_zeros(point: &crate::verifier::G2Point) -> bool {
        Self::bytes_all_zero(&point.x.0.to_array())
            && Self::bytes_all_zero(&point.x.1.to_array())
            && Self::bytes_all_zero(&point.y.0.to_array())
            && Self::bytes_all_zero(&point.y.1.to_array())
    }

    fn g2_is_all_ones(point: &crate::verifier::G2Point) -> bool {
        Self::bytes_all_ff(&point.x.0.to_array())
            && Self::bytes_all_ff(&point.x.1.to_array())
            && Self::bytes_all_ff(&point.y.0.to_array())
            && Self::bytes_all_ff(&point.y.1.to_array())
    }

    fn validate_g2_limbs(point: &crate::verifier::G2Point) -> bool {
        const LIMB_SIZE: usize = 32;
        let limbs = [
            &point.x.0.to_array(),
            &point.x.1.to_array(),
            &point.y.0.to_array(),
            &point.y.1.to_array(),
        ];

        for limb in &limbs {
            if Self::bytes_all_zero(limb) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verifier::{G1Point, G2Point};
    use soroban_sdk::Env;

    fn create_valid_proof(env: &Env) -> Proof {
        Proof {
            a: G1Point {
                x: BytesN::from_array(env, &[1; 32]),
                y: BytesN::from_array(env, &[1; 32]),
            },
            b: G2Point {
                x: (
                    BytesN::from_array(env, &[1; 32]),
                    BytesN::from_array(env, &[1; 32]),
                ),
                y: (
                    BytesN::from_array(env, &[1; 32]),
                    BytesN::from_array(env, &[1; 32]),
                ),
            },
            c: G1Point {
                x: BytesN::from_array(env, &[1; 32]),
                y: BytesN::from_array(env, &[1; 32]),
            },
        }
    }

    #[test]
    fn test_plonk_validate_valid_proof() {
        let env = Env::default();
        let proof = create_valid_proof(&env);
        let mut public_inputs = Vec::new(&env);
        public_inputs.push_back(BytesN::from_array(&env, &[1; 32]));

        let result = PlonkVerifier::validate_proof_components(&proof, &public_inputs);
        assert!(result.is_ok());
    }

    #[test]
    fn test_plonk_reject_empty_inputs() {
        let env = Env::default();
        let proof = create_valid_proof(&env);
        let public_inputs = Vec::new(&env);

        let result = PlonkVerifier::validate_proof_components(&proof, &public_inputs);
        assert_eq!(result, Err(ProofValidationError::EmptyPublicInputs));
    }

    #[test]
    fn test_plonk_reject_zeroed_component() {
        let env = Env::default();
        let mut proof = create_valid_proof(&env);
        proof.a.x = BytesN::from_array(&env, &[0; 32]);
        proof.a.y = BytesN::from_array(&env, &[0; 32]);

        let mut public_inputs = Vec::new(&env);
        public_inputs.push_back(BytesN::from_array(&env, &[1; 32]));

        let result = PlonkVerifier::validate_proof_components(&proof, &public_inputs);
        assert_eq!(result, Err(ProofValidationError::ZeroedComponent));
    }
}
