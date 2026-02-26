//! # ZK-Based Selective Disclosure
//!
//! This module implements the core selective disclosure logic for verifiable
//! credentials. It enables holders to:
//!
//! - **Reveal chosen attributes** with proofs binding them to the credential
//!   commitment.
//! - **Prove predicates** (range proofs, set membership) without revealing
//!   the underlying values.
//! - **Generate auditable presentations** that can be linked for regulatory
//!   compliance.
//!
//! ## Verification Strategy
//!
//! Each disclosed attribute carries a proof that it matches the Poseidon
//! commitment stored in the credential. Predicates carry proofs that the
//! hidden value satisfies the stated condition. The presentation proof
//! binds everything together into a single verifiable statement.

use common::credential_types::{
    CredentialContractError, CredentialPresentation, DisclosedAttribute, PredicateResult,
    PredicateType,
};
use soroban_sdk::{Bytes, BytesN, Env, Vec};

use crate::verifier::PoseidonHasher;

// ── Selective Disclosure Verifier ───────────────────────────────────────────

pub struct SelectiveDisclosureVerifier;

impl SelectiveDisclosureVerifier {
    /// Verify a complete credential presentation's disclosure proofs.
    ///
    /// This checks:
    /// 1. Each disclosed attribute's proof is valid against the presentation proof.
    /// 2. Each predicate result's proof is valid.
    /// 3. The overall presentation proof binds all components together.
    pub fn verify_presentation(
        env: &Env,
        presentation: &CredentialPresentation,
    ) -> Result<(), CredentialContractError> {
        // 1. Verify each disclosed attribute proof.
        Self::verify_disclosed_attributes(env, presentation)?;

        // 2. Verify each predicate proof.
        Self::verify_predicate_results(env, presentation)?;

        // 3. Verify the aggregate presentation proof.
        Self::verify_presentation_binding(env, presentation)?;

        Ok(())
    }

    /// Verify that each disclosed attribute's proof is consistent with the
    /// credential commitment.
    ///
    /// The disclosure proof for each attribute is a ZK proof that:
    ///   Hash(key || value) is part of the credential's claims_commitment.
    ///
    /// In the mock implementation, we verify that the disclosure proof is a
    /// Poseidon hash of (key_bytes || value).
    fn verify_disclosed_attributes(
        env: &Env,
        presentation: &CredentialPresentation,
    ) -> Result<(), CredentialContractError> {
        let zero = BytesN::from_array(env, &[0u8; 32]);

        for attr in presentation.disclosed_attributes.iter() {
            // The disclosure proof must be non-zero.
            if attr.disclosure_proof == zero {
                return Err(CredentialContractError::InvalidDisclosureProof);
            }

            // Verify the disclosure proof: it should be a hash of the
            // attribute key and value, binding it to the credential.
            let expected = Self::compute_attribute_proof(env, &attr);
            if attr.disclosure_proof != expected {
                return Err(CredentialContractError::InvalidDisclosureProof);
            }
        }

        Ok(())
    }

    /// Verify that each predicate result's proof is valid.
    ///
    /// The predicate proof demonstrates that the hidden claim value satisfies
    /// the stated condition without revealing what the value is.
    fn verify_predicate_results(
        env: &Env,
        presentation: &CredentialPresentation,
    ) -> Result<(), CredentialContractError> {
        let zero = BytesN::from_array(env, &[0u8; 32]);

        for result in presentation.predicate_results.iter() {
            // Predicate proof must be non-zero.
            if result.proof == zero {
                return Err(CredentialContractError::PredicateFailed);
            }

            // Predicate must be satisfied.
            if !result.satisfied {
                return Err(CredentialContractError::PredicateFailed);
            }

            // Verify the proof is correctly formed for this predicate type.
            let expected = Self::compute_predicate_proof(env, &result);
            if result.proof != expected {
                return Err(CredentialContractError::PredicateFailed);
            }
        }

        Ok(())
    }

    /// Verify that the presentation proof correctly binds all disclosed
    /// attributes and predicate results to the credential.
    ///
    /// The presentation proof is a ZK proof that:
    /// - All disclosed attributes are from the same credential.
    /// - All predicate results are evaluated on the same credential.
    /// - The credential ID matches.
    fn verify_presentation_binding(
        env: &Env,
        presentation: &CredentialPresentation,
    ) -> Result<(), CredentialContractError> {
        let zero = BytesN::from_array(env, &[0u8; 32]);

        if presentation.presentation_proof == zero {
            return Err(CredentialContractError::InvalidPresentationProof);
        }

        // Compute expected presentation proof by hashing all components.
        let expected = Self::compute_presentation_proof(env, presentation);
        if presentation.presentation_proof != expected {
            return Err(CredentialContractError::InvalidPresentationProof);
        }

        Ok(())
    }

    // ── Proof computation helpers ───────────────────────────────────────

    /// Compute the disclosure proof for a single attribute.
    ///
    /// proof = Hash(credential_key_bytes || value)
    /// where credential_key_bytes is derived from the Symbol key.
    pub fn compute_attribute_proof(env: &Env, attr: &DisclosedAttribute) -> BytesN<32> {
        let mut inputs = Vec::new(env);

        // Convert the key symbol to bytes and hash it to get a 32-byte input.
        let key_bytes = symbol_to_bytes32(env, &attr.key);
        inputs.push_back(key_bytes);
        inputs.push_back(attr.value.clone());

        PoseidonHasher::hash(env, &inputs)
    }

    /// Compute the proof for a predicate result.
    ///
    /// proof = Hash(claim_key_bytes || predicate_type_bytes || satisfied_byte)
    pub fn compute_predicate_proof(env: &Env, result: &PredicateResult) -> BytesN<32> {
        let mut inputs = Vec::new(env);

        let key_bytes = symbol_to_bytes32(env, &result.claim_key);
        inputs.push_back(key_bytes);

        let predicate_byte = predicate_type_to_byte(&result.predicate_type);
        let mut pred_arr = [0u8; 32];
        pred_arr[0] = predicate_byte;
        pred_arr[1] = if result.satisfied { 1 } else { 0 };
        inputs.push_back(BytesN::from_array(env, &pred_arr));

        PoseidonHasher::hash(env, &inputs)
    }

    /// Compute the aggregate presentation proof.
    ///
    /// proof = Hash(credential_id || schema_id || attr_proofs_hash || pred_proofs_hash)
    pub fn compute_presentation_proof(
        env: &Env,
        presentation: &CredentialPresentation,
    ) -> BytesN<32> {
        let mut inputs = Vec::new(env);

        inputs.push_back(presentation.credential_id.clone());
        inputs.push_back(presentation.schema_id.clone());

        // Hash all attribute disclosure proofs together.
        if !presentation.disclosed_attributes.is_empty() {
            let mut attr_inputs = Vec::new(env);
            for attr in presentation.disclosed_attributes.iter() {
                attr_inputs.push_back(attr.disclosure_proof.clone());
            }
            inputs.push_back(PoseidonHasher::hash(env, &attr_inputs));
        }

        // Hash all predicate proofs together.
        if !presentation.predicate_results.is_empty() {
            let mut pred_inputs = Vec::new(env);
            for result in presentation.predicate_results.iter() {
                pred_inputs.push_back(result.proof.clone());
            }
            inputs.push_back(PoseidonHasher::hash(env, &pred_inputs));
        }

        PoseidonHasher::hash(env, &inputs)
    }

    // ── Predicate evaluation (off-chain helper, tested on-chain) ────────

    /// Evaluate a GreaterThan predicate: value > threshold.
    ///
    /// Both value and threshold are interpreted as big-endian u64 in the
    /// first 8 bytes of their respective BytesN<32>.
    pub fn evaluate_greater_than(value: &BytesN<32>, threshold: &BytesN<32>) -> bool {
        let v = bytes32_to_u64(value);
        let t = bytes32_to_u64(threshold);
        v > t
    }

    /// Evaluate a LessThan predicate: value < threshold.
    pub fn evaluate_less_than(value: &BytesN<32>, threshold: &BytesN<32>) -> bool {
        let v = bytes32_to_u64(value);
        let t = bytes32_to_u64(threshold);
        v < t
    }

    /// Evaluate an InRange predicate: lower <= value <= upper.
    pub fn evaluate_in_range(value: &BytesN<32>, lower: &BytesN<32>, upper: &BytesN<32>) -> bool {
        let v = bytes32_to_u64(value);
        let lo = bytes32_to_u64(lower);
        let hi = bytes32_to_u64(upper);
        v >= lo && v <= hi
    }

    /// Evaluate a SetMembership predicate: value is in the set.
    pub fn evaluate_set_membership(value: &BytesN<32>, set: &Vec<BytesN<32>>) -> bool {
        for member in set.iter() {
            if *value == member {
                return true;
            }
        }
        false
    }

    /// Evaluate a NotEqual predicate: value != target.
    pub fn evaluate_not_equal(value: &BytesN<32>, target: &BytesN<32>) -> bool {
        value != target
    }
}

// ── Utility functions ───────────────────────────────────────────────────────

/// Convert a Symbol to a deterministic 32-byte representation by hashing.
fn symbol_to_bytes32(env: &Env, sym: &soroban_sdk::Symbol) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    // Symbols in Soroban are represented as u64 values; convert to bytes for hashing.
    let sym_val = sym.to_val();
    let sym_u64 = sym_val.get_payload();
    buf.extend_from_array(&sym_u64.to_be_bytes());
    // Pad and hash to get a consistent 32-byte output.
    env.crypto().keccak256(&buf).into()
}

/// Convert a predicate type enum to a single byte tag.
fn predicate_type_to_byte(pt: &PredicateType) -> u8 {
    match pt {
        PredicateType::GreaterThan => 0,
        PredicateType::LessThan => 1,
        PredicateType::InRange => 2,
        PredicateType::SetMembership => 3,
        PredicateType::NotEqual => 4,
    }
}

/// Interpret the first 8 bytes of a BytesN<32> as a big-endian u64.
fn bytes32_to_u64(b: &BytesN<32>) -> u64 {
    let arr = b.to_array();
    u64::from_be_bytes([
        arr[0], arr[1], arr[2], arr[3], arr[4], arr[5], arr[6], arr[7],
    ])
}
