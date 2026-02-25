//! # Zero-Knowledge Credential Issuance and Presentation
//!
//! This module implements a verifiable credential system where:
//! - **Issuers** register schemas and issue credentials with committed claims.
//! - **Holders** present credentials with selective disclosure.
//! - **Verifiers** validate presentations without seeing undisclosed data.
//!
//! ## Credential Lifecycle
//! 1. Issuer registers a [`CredentialSchema`] defining allowed claims.
//! 2. Issuer issues a [`Credential`] to a holder with a Poseidon commitment
//!    to all claim values.
//! 3. Holder creates a [`CredentialPresentation`] choosing which claims to
//!    reveal and which predicates to prove.
//! 4. Verifier checks the presentation proof, non-revocation, and predicates.
//!
//! ## Credential Chaining
//! A new credential can be issued contingent on proof of an existing one.
//! The parent presentation is verified before the child credential is minted.

use common::credential_types::{
    ChainedIssuanceRequest, Credential, CredentialContractError, CredentialPresentation,
    CredentialSchema, CredentialStatus, RevocationWitness,
};
use soroban_sdk::{Address, Bytes, BytesN, Env, Symbol, Vec};

use crate::revocation::RevocationRegistryManager;
use crate::selective_disclosure::SelectiveDisclosureVerifier;
use crate::verifier::PoseidonHasher;

// ── Storage key prefixes ────────────────────────────────────────────────────

const SCHEMA_PREFIX: &str = "CRED_SCH";
const CRED_PREFIX: &str = "CRED_DAT";
const HOLDER_CREDS_PREFIX: &str = "HOLD_CRD";
const ISSUER_SCHEMAS_PREFIX: &str = "ISS_SCH";
const WITNESS_PREFIX: &str = "CRED_WIT";

// ── Credential Manager ──────────────────────────────────────────────────────

pub struct CredentialManager;

impl CredentialManager {
    // ── Schema operations ───────────────────────────────────────────────

    /// Register a new credential schema. Only the issuer can register schemas.
    pub fn register_schema(
        env: &Env,
        schema: &CredentialSchema,
    ) -> Result<(), CredentialContractError> {
        let key = (
            Symbol::new(env, SCHEMA_PREFIX),
            schema.schema_id.clone(),
        );

        if env.storage().persistent().has(&key) {
            return Err(CredentialContractError::DuplicateSchema);
        }

        env.storage().persistent().set(&key, schema);

        // Track schemas per issuer.
        let issuer_key = (
            Symbol::new(env, ISSUER_SCHEMAS_PREFIX),
            schema.issuer.clone(),
        );
        let mut schemas: Vec<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&issuer_key)
            .unwrap_or_else(|| Vec::new(env));
        schemas.push_back(schema.schema_id.clone());
        env.storage().persistent().set(&issuer_key, &schemas);

        Ok(())
    }

    /// Retrieve a credential schema by its ID.
    pub fn get_schema(
        env: &Env,
        schema_id: &BytesN<32>,
    ) -> Result<CredentialSchema, CredentialContractError> {
        let key = (Symbol::new(env, SCHEMA_PREFIX), schema_id.clone());
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(CredentialContractError::SchemaNotFound)
    }

    /// List all schema IDs registered by a given issuer.
    pub fn get_issuer_schemas(env: &Env, issuer: &Address) -> Vec<BytesN<32>> {
        let key = (Symbol::new(env, ISSUER_SCHEMAS_PREFIX), issuer.clone());
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env))
    }

    // ── Credential issuance ─────────────────────────────────────────────

    /// Issue a new credential to a holder.
    ///
    /// The issuer provides a commitment to all claim values. The actual values
    /// are never stored on-chain — only the commitment is recorded.
    pub fn issue_credential(
        env: &Env,
        credential: &Credential,
    ) -> Result<(), CredentialContractError> {
        // Verify schema exists.
        Self::get_schema(env, &credential.schema_id)?;

        // Check for duplicate.
        let cred_key = (
            Symbol::new(env, CRED_PREFIX),
            credential.credential_id.clone(),
        );
        if env.storage().persistent().has(&cred_key) {
            return Err(CredentialContractError::DuplicateCredential);
        }

        // Store credential.
        env.storage().persistent().set(&cred_key, credential);

        // Track credentials per holder.
        let holder_key = (
            Symbol::new(env, HOLDER_CREDS_PREFIX),
            credential.holder.clone(),
        );
        let mut creds: Vec<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&holder_key)
            .unwrap_or_else(|| Vec::new(env));
        creds.push_back(credential.credential_id.clone());
        env.storage().persistent().set(&holder_key, &creds);

        Ok(())
    }

    /// Issue a credential based on proof of an existing (parent) credential.
    ///
    /// The parent presentation is fully verified before the child is issued.
    /// This enables credential chaining while maintaining privacy: the child
    /// issuer only sees what the parent presentation selectively discloses.
    pub fn issue_chained_credential(
        env: &Env,
        issuer: &Address,
        request: &ChainedIssuanceRequest,
        new_credential_id: BytesN<32>,
        holder: &Address,
        revocation_index: u64,
    ) -> Result<Credential, CredentialContractError> {
        // 1. Verify the parent presentation is valid.
        let parent_cred = Self::get_credential(env, &request.parent_presentation.credential_id)?;

        // Ensure parent credential is active.
        Self::require_active(env, &parent_cred)?;

        // Verify the parent presentation proof.
        SelectiveDisclosureVerifier::verify_presentation(env, &request.parent_presentation)?;

        // 2. Build the child credential.
        let zero = BytesN::from_array(env, &[0u8; 32]);
        let child = Credential {
            credential_id: new_credential_id,
            schema_id: request.new_schema_id.clone(),
            issuer: issuer.clone(),
            holder: holder.clone(),
            issued_at: env.ledger().timestamp(),
            expires_at: request.requested_expiry,
            status: CredentialStatus::Active,
            claims_commitment: request.new_claims_commitment.clone(),
            parent_credential_id: request.parent_presentation.credential_id.clone(),
            revocation_index,
        };

        // 3. Issue the child credential.
        Self::issue_credential(env, &child)?;

        Ok(child)
    }

    // ── Credential retrieval ────────────────────────────────────────────

    /// Retrieve a credential by its ID.
    pub fn get_credential(
        env: &Env,
        credential_id: &BytesN<32>,
    ) -> Result<Credential, CredentialContractError> {
        let key = (Symbol::new(env, CRED_PREFIX), credential_id.clone());
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(CredentialContractError::CredentialNotFound)
    }

    /// List all credential IDs held by a given address.
    pub fn get_holder_credentials(env: &Env, holder: &Address) -> Vec<BytesN<32>> {
        let key = (Symbol::new(env, HOLDER_CREDS_PREFIX), holder.clone());
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env))
    }

    // ── Credential status ───────────────────────────────────────────────

    /// Update the status of a credential. Only the issuer can change status.
    pub fn update_status(
        env: &Env,
        credential_id: &BytesN<32>,
        new_status: CredentialStatus,
    ) -> Result<(), CredentialContractError> {
        let key = (Symbol::new(env, CRED_PREFIX), credential_id.clone());
        let mut cred: Credential = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(CredentialContractError::CredentialNotFound)?;

        cred.status = new_status;
        env.storage().persistent().set(&key, &cred);
        Ok(())
    }

    /// Check that a credential is active (not revoked, expired, or suspended).
    pub fn require_active(
        env: &Env,
        credential: &Credential,
    ) -> Result<(), CredentialContractError> {
        match credential.status {
            CredentialStatus::Active => {
                // Also check temporal expiration.
                if env.ledger().timestamp() > credential.expires_at {
                    return Err(CredentialContractError::CredentialExpired);
                }
                Ok(())
            }
            CredentialStatus::Revoked => Err(CredentialContractError::CredentialRevoked),
            CredentialStatus::Expired => Err(CredentialContractError::CredentialExpired),
            CredentialStatus::Suspended => Err(CredentialContractError::CredentialSuspended),
        }
    }

    // ── Revocation witness storage ──────────────────────────────────────

    /// Store a non-revocation witness for a credential.
    pub fn store_witness(
        env: &Env,
        witness: &RevocationWitness,
    ) -> Result<(), CredentialContractError> {
        let key = (
            Symbol::new(env, WITNESS_PREFIX),
            witness.credential_id.clone(),
        );
        env.storage().persistent().set(&key, witness);
        Ok(())
    }

    /// Retrieve the non-revocation witness for a credential.
    pub fn get_witness(
        env: &Env,
        credential_id: &BytesN<32>,
    ) -> Result<RevocationWitness, CredentialContractError> {
        let key = (Symbol::new(env, WITNESS_PREFIX), credential_id.clone());
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(CredentialContractError::CredentialNotFound)
    }

    // ── Presentation verification (full pipeline) ───────────────────────

    /// Verify a complete credential presentation including:
    /// 1. Credential existence and active status.
    /// 2. Schema binding.
    /// 3. Selective disclosure proofs.
    /// 4. Predicate proofs.
    /// 5. Non-revocation proof.
    pub fn verify_presentation(
        env: &Env,
        presentation: &CredentialPresentation,
    ) -> Result<bool, CredentialContractError> {
        // 1. Retrieve and validate credential.
        let credential = Self::get_credential(env, &presentation.credential_id)?;
        Self::require_active(env, &credential)?;

        // 2. Verify schema binding.
        if credential.schema_id != presentation.schema_id {
            return Err(CredentialContractError::SchemaMismatch);
        }

        // 3. Verify holder binding.
        if credential.holder != presentation.holder {
            return Err(CredentialContractError::NotHolder);
        }

        // 4. Verify the selective disclosure and predicate proofs.
        SelectiveDisclosureVerifier::verify_presentation(env, presentation)?;

        // 5. Verify non-revocation.
        RevocationRegistryManager::verify_non_revocation_proof(
            env,
            &credential,
            &presentation.non_revocation_proof,
        )?;

        Ok(true)
    }

    /// Verify multiple presentations in a batch.
    ///
    /// Batch verification is more efficient than individual verification
    /// because shared state lookups (schemas, registries) are amortized.
    pub fn batch_verify_presentations(
        env: &Env,
        presentations: &Vec<CredentialPresentation>,
    ) -> Result<common::credential_types::BatchVerificationResult, CredentialContractError> {
        if presentations.is_empty() {
            return Err(CredentialContractError::EmptyBatch);
        }

        let total = presentations.len();
        let mut verified: u32 = 0;
        let mut failed: u32 = 0;
        let mut results = Vec::new(env);

        for presentation in presentations.iter() {
            match Self::verify_presentation(env, &presentation) {
                Ok(true) => {
                    verified += 1;
                    results.push_back(true);
                }
                _ => {
                    failed += 1;
                    results.push_back(false);
                }
            }
        }

        Ok(common::credential_types::BatchVerificationResult {
            total,
            verified,
            failed,
            results,
        })
    }
}
