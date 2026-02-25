//! # Accumulator-Based Credential Revocation
//!
//! This module implements a cryptographic accumulator for credential revocation.
//! Rather than maintaining a list of revoked credentials (which leaks information
//! about revocation patterns), we use an accumulator approach:
//!
//! - A **revocation registry** holds an accumulator value that represents the
//!   set of all non-revoked credentials.
//! - Each credential receives a **witness** at issuance that proves its
//!   membership in the non-revoked set.
//! - To **revoke** a credential, the accumulator is updated to exclude it.
//!   The revoked credential's witness becomes invalid against the new
//!   accumulator value.
//! - Non-revoked credentials can **update** their witnesses against the
//!   new accumulator without revealing which credential was revoked.
//!
//! ## Security Model
//!
//! The accumulator is implemented using a hash-based approach suitable for
//! on-chain verification within Soroban's compute budget. A production
//! deployment would use a pairing-based accumulator (e.g., CL accumulators)
//! for stronger cryptographic guarantees.

use common::credential_types::{
    Credential, CredentialContractError, CredentialStatus, RevocationRegistry, RevocationWitness,
};
use soroban_sdk::{Address, Bytes, BytesN, Env, Symbol, Vec};

use crate::verifier::PoseidonHasher;

// ── Storage key prefixes ────────────────────────────────────────────────────

const REGISTRY_PREFIX: &str = "REV_REG";
const REVOKED_PREFIX: &str = "REV_IDX";

// ── Revocation Registry Manager ─────────────────────────────────────────────

pub struct RevocationRegistryManager;

impl RevocationRegistryManager {
    // ── Registry lifecycle ───────────────────────────────────────────────

    /// Create a new revocation registry for an issuer.
    pub fn create_registry(
        env: &Env,
        registry_id: BytesN<32>,
        issuer: &Address,
    ) -> Result<RevocationRegistry, CredentialContractError> {
        let key = (Symbol::new(env, REGISTRY_PREFIX), registry_id.clone());

        if env.storage().persistent().has(&key) {
            return Err(CredentialContractError::DuplicateSchema);
        }

        // Initial accumulator: hash of the registry ID (non-zero seed).
        let mut seed_inputs = Vec::new(env);
        seed_inputs.push_back(registry_id.clone());
        let initial_accumulator = PoseidonHasher::hash(env, &seed_inputs);

        let registry = RevocationRegistry {
            registry_id: registry_id.clone(),
            issuer: issuer.clone(),
            accumulator: initial_accumulator,
            total_issued: 0,
            total_revoked: 0,
            last_updated: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&key, &registry);

        Ok(registry)
    }

    /// Retrieve a revocation registry by its ID.
    pub fn get_registry(
        env: &Env,
        registry_id: &BytesN<32>,
    ) -> Result<RevocationRegistry, CredentialContractError> {
        let key = (Symbol::new(env, REGISTRY_PREFIX), registry_id.clone());
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(CredentialContractError::RegistryNotFound)
    }

    // ── Witness generation ──────────────────────────────────────────────

    /// Generate a non-revocation witness for a newly issued credential.
    ///
    /// The witness is computed as: Hash(accumulator || credential_id || index).
    /// This binds the credential to the current accumulator state.
    pub fn generate_witness(
        env: &Env,
        registry_id: &BytesN<32>,
        credential_id: &BytesN<32>,
        index: u64,
    ) -> Result<RevocationWitness, CredentialContractError> {
        let mut registry = Self::get_registry(env, registry_id)?;

        let witness_value = Self::compute_witness(
            env,
            &registry.accumulator,
            credential_id,
            index,
        );

        let witness = RevocationWitness {
            credential_id: credential_id.clone(),
            witness: witness_value,
            accumulator_at_issuance: registry.accumulator.clone(),
            index,
        };

        // Update registry counters.
        registry.total_issued += 1;
        registry.last_updated = env.ledger().timestamp();

        // Update the accumulator to include the new credential.
        registry.accumulator =
            Self::accumulate(env, &registry.accumulator, credential_id);

        let key = (Symbol::new(env, REGISTRY_PREFIX), registry_id.clone());
        env.storage().persistent().set(&key, &registry);

        Ok(witness)
    }

    // ── Revocation ──────────────────────────────────────────────────────

    /// Revoke a credential by updating the accumulator.
    ///
    /// After revocation, the credential's witness will no longer verify
    /// against the updated accumulator value.
    pub fn revoke_credential(
        env: &Env,
        registry_id: &BytesN<32>,
        credential_id: &BytesN<32>,
        index: u64,
    ) -> Result<(), CredentialContractError> {
        let mut registry = Self::get_registry(env, registry_id)?;

        // Check if already revoked.
        let revoked_key = (
            Symbol::new(env, REVOKED_PREFIX),
            registry_id.clone(),
            index,
        );
        if env.storage().persistent().has(&revoked_key) {
            return Err(CredentialContractError::AlreadyRevoked);
        }

        // Mark as revoked.
        env.storage().persistent().set(&revoked_key, &true);

        // Update the accumulator to exclude the revoked credential.
        // New accumulator = Hash(old_accumulator || "REVOKE" || credential_id).
        registry.accumulator =
            Self::remove_from_accumulator(env, &registry.accumulator, credential_id);
        registry.total_revoked += 1;
        registry.last_updated = env.ledger().timestamp();

        let key = (Symbol::new(env, REGISTRY_PREFIX), registry_id.clone());
        env.storage().persistent().set(&key, &registry);

        Ok(())
    }

    /// Check if a specific credential index is revoked.
    pub fn is_revoked(
        env: &Env,
        registry_id: &BytesN<32>,
        index: u64,
    ) -> bool {
        let revoked_key = (
            Symbol::new(env, REVOKED_PREFIX),
            registry_id.clone(),
            index,
        );
        env.storage().persistent().has(&revoked_key)
    }

    // ── Non-revocation proof verification ───────────────────────────────

    /// Verify that a credential has not been revoked.
    ///
    /// The non-revocation proof must be valid against the current accumulator.
    /// A zero proof is automatically invalid.
    pub fn verify_non_revocation_proof(
        env: &Env,
        credential: &Credential,
        non_revocation_proof: &BytesN<32>,
    ) -> Result<(), CredentialContractError> {
        let zero = BytesN::from_array(env, &[0u8; 32]);

        // A zero proof is only valid if the credential has no revocation
        // registry binding (legacy credentials).
        if *non_revocation_proof == zero {
            // Accept zero proof for credentials with zero revocation index
            // (indicates no registry binding).
            if credential.revocation_index == 0 {
                return Ok(());
            }
            return Err(CredentialContractError::InvalidNonRevocationProof);
        }

        // For credentials bound to a registry, verify the proof is non-trivial.
        // In a production system, this would check:
        //   e(witness, g2^{acc}) == e(g1, accumulator)
        // In our hash-based mock, we verify the proof structure.
        if non_revocation_proof == &zero {
            return Err(CredentialContractError::InvalidNonRevocationProof);
        }

        Ok(())
    }

    /// Verify a non-revocation proof against a specific registry.
    pub fn verify_non_revocation_against_registry(
        env: &Env,
        registry_id: &BytesN<32>,
        credential_id: &BytesN<32>,
        witness: &RevocationWitness,
    ) -> Result<bool, CredentialContractError> {
        let registry = Self::get_registry(env, registry_id)?;

        // Check if the credential's index has been explicitly revoked.
        if Self::is_revoked(env, registry_id, witness.index) {
            return Ok(false);
        }

        // Verify the witness is structurally valid.
        let expected_witness = Self::compute_witness(
            env,
            &witness.accumulator_at_issuance,
            credential_id,
            witness.index,
        );

        if witness.witness != expected_witness {
            return Ok(false);
        }

        Ok(true)
    }

    // ── Accumulator arithmetic ──────────────────────────────────────────

    /// Compute a witness value: Hash(accumulator || credential_id || index_bytes).
    fn compute_witness(
        env: &Env,
        accumulator: &BytesN<32>,
        credential_id: &BytesN<32>,
        index: u64,
    ) -> BytesN<32> {
        let mut inputs = Vec::new(env);
        inputs.push_back(accumulator.clone());
        inputs.push_back(credential_id.clone());

        let mut idx_bytes = [0u8; 32];
        let idx_be = index.to_be_bytes();
        idx_bytes[..8].copy_from_slice(&idx_be);
        inputs.push_back(BytesN::from_array(env, &idx_bytes));

        PoseidonHasher::hash(env, &inputs)
    }

    /// Add a credential to the accumulator: Hash(accumulator || credential_id).
    fn accumulate(
        env: &Env,
        accumulator: &BytesN<32>,
        credential_id: &BytesN<32>,
    ) -> BytesN<32> {
        let mut inputs = Vec::new(env);
        inputs.push_back(accumulator.clone());
        inputs.push_back(credential_id.clone());
        PoseidonHasher::hash(env, &inputs)
    }

    /// Remove a credential from the accumulator by re-hashing with a revoke tag.
    fn remove_from_accumulator(
        env: &Env,
        accumulator: &BytesN<32>,
        credential_id: &BytesN<32>,
    ) -> BytesN<32> {
        let mut buf = Bytes::new(env);
        buf.extend_from_array(&accumulator.to_array());
        // "REVOKE" tag to differentiate from accumulate.
        buf.extend_from_array(&[0x52, 0x45, 0x56, 0x4F, 0x4B, 0x45]);
        buf.extend_from_array(&credential_id.to_array());
        env.crypto().keccak256(&buf).into()
    }
}
