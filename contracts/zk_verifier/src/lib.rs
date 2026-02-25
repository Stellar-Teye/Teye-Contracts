#![allow(dead_code, clippy::manual_inspect, clippy::arithmetic_side_effects)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! # ZK Verifier Module
//!
//! This module provides a Zero-Knowledge (ZK) proof verification system for the Soroban ecosystem.
//! It specifically implements support for Groth16 proofs over the BN254 (Alt-BN128) curve.
//!
//! The ZK subsystem is designed to provide privacy-preserving access control by allowing users
//! to prove they possess certain credentials or meet specific criteria without revealing
//! the underlying sensitive data.
//!
//! ## Key Components
//! - `ZkVerifierContract`: The main contract implementation handling access requests and auditing.
//! - `Bn254Verifier`: The core library for verifying Groth16 proofs.
//! - `AuditTrail`: A persistence layer for logging successful verifications.
//! - `ZkAccessHelper`: A utility for formatting binary proof data into interoperable requests.

mod audit;
pub mod credentials;
pub mod events;
mod helpers;
pub mod revocation;
pub mod selective_disclosure;
pub mod verifier;
pub mod vk;

pub use crate::audit::{AuditRecord, AuditTrail};
pub use crate::credentials::CredentialManager;
pub use crate::events::AccessRejectedEvent;
pub use crate::helpers::ZkAccessHelper;
pub use crate::verifier::{Bn254Verifier, PoseidonHasher, Proof, ProofValidationError, VerificationKey};
pub use crate::verifier::{Bn254Verifier, PoseidonHasher, Proof, ProofValidationError};
pub use crate::vk::VerificationKey;

use common::{nonce, whitelist};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    String, Symbol, Vec,
};
// use verifier::ProofValidationError;

const ADMIN: Symbol = symbol_short!("ADMIN");
const PENDING_ADMIN: Symbol = symbol_short!("PEND_ADM");
const RATE_CFG: Symbol = symbol_short!("RATECFG");
const RATE_TRACK: Symbol = symbol_short!("RLTRK");


/// Maximum number of public inputs accepted per proof verification.
const MAX_PUBLIC_INPUTS: u32 = 16;

/// Request structure for ZK access verification.
// TODO: post-quantum migration - This struct currently hardcodes a Groth16 `Proof`.
// Future PQ systems (like STARKs) will require an `enum ProofType` or dynamically sized bytes
// to encapsulate changing proof shapes and public inputs matrices.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRequest {
    /// The address of the user requesting access.
    pub user: Address,
    /// Unique identifier for the resource being accessed.
    pub resource_id: BytesN<32>,
    /// The Groth16 proof (points A, B, and C).
    pub proof: Proof,
    /// Public inputs associated with the proof.
    pub public_inputs: Vec<BytesN<32>>,
    /// Timestamp after which this proof is no longer valid.
    pub expires_at: u64,
    pub timestamp: u64,
}

/// Storage keys (all ≤9 chars for symbol_short!)
const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");
const PROOF_CTR: Symbol = symbol_short!("PROOF_CTR");
const VFY_RES: Symbol = symbol_short!("VFY_RES");

/// Contract error codes
#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    Unauthorized = 1,
    RateLimited = 2,
    InvalidConfig = 3,
    EmptyPublicInputs = 4,
    TooManyPublicInputs = 5,
    DegenerateProof = 6,
    /// A proof component is saturated (all 0xFF) — invalid curve encoding.
    OversizedProofComponent = 7,
    /// A G1 point has a malformed internal structure (e.g. one coordinate is zero).
    MalformedG1Point = 8,
    /// The G2 point has a malformed internal structure (e.g. a limb is zero).
    MalformedG2Point = 9,
    /// A public-input element is all zeros.
    ZeroedPublicInput = 10,
    /// Cross-contract proof deserialization produced structurally invalid data.
    MalformedProofData = 11,
    /// The proof has expired (current timestamp exceeds `expires_at`).
    ExpiredProof = 12,
    /// The provided nonce does not match the expected value (replay or out-of-order).
    InvalidNonce = 12,
    /// The contract is paused and cannot process verification requests.
    Paused = 12,
    /// Invalid authentication level supplied to the verifier.
    InvalidAuthLevel = 13,
    /// Public inputs are insufficient for the required authentication level.
    ProofRequiredForAuthLevel = 14,
}

/// Map low-level proof validation errors into contract-level errors.
fn map_proof_validation_error(e: ProofValidationError) -> ContractError {
    match e {
        ProofValidationError::ZeroedComponent => ContractError::DegenerateProof,
        ProofValidationError::OversizedComponent => ContractError::OversizedProofComponent,
        ProofValidationError::MalformedG1PointA | ProofValidationError::MalformedG1PointC => {
            ContractError::MalformedG1Point
        }
        ProofValidationError::MalformedG2Point => ContractError::MalformedG2Point,
        ProofValidationError::EmptyPublicInputs => ContractError::EmptyPublicInputs,
        ProofValidationError::ZeroedPublicInput => ContractError::ZeroedPublicInput,
    }
}

#[contract]
pub struct ZkVerifierContract;

/// Return `true` if every byte in `data` is zero.
fn is_all_zeros(data: &BytesN<32>) -> bool {
    let arr = data.to_array();
    let mut all_zero = true;
    let mut i = 0;
    while i < 32 {
        if arr[i] != 0 {
            all_zero = false;
            break;
        }
        i += 1;
    }
    all_zero
}

/// Validate request shape before running proof verification.
///
/// This performs lightweight structural checks on the `AccessRequest` envelope.
/// Deeper proof-component validation (zeroed, oversized, malformed coordinates)
/// is delegated to [`Bn254Verifier::validate_proof_components`] which runs
/// inside `verify_proof` and returns granular [`ProofValidationError`] variants
/// that are mapped to [`ContractError`] via the `From` impl.
fn validate_request(request: &AccessRequest) -> Result<(), ContractError> {
    if request.public_inputs.is_empty() {
        return Err(ContractError::EmptyPublicInputs);
    }

    if request.public_inputs.len() > MAX_PUBLIC_INPUTS {
        return Err(ContractError::TooManyPublicInputs);
    }

    if (is_all_zeros(&request.proof.a.x) && is_all_zeros(&request.proof.a.y))
        || (is_all_zeros(&request.proof.b.x.0)
            && is_all_zeros(&request.proof.b.x.1)
            && is_all_zeros(&request.proof.b.y.0)
            && is_all_zeros(&request.proof.b.y.1))
        || (is_all_zeros(&request.proof.c.x) && is_all_zeros(&request.proof.c.y))
    {
        return Err(ContractError::DegenerateProof);
    }

    for pi in request.public_inputs.iter() {
        if is_all_zeros(&pi) {
            return Err(ContractError::ZeroedPublicInput);
        }
    }

    Ok(())
}

fn validate_auth_level(level: u32) -> Result<(), ContractError> {
    if !(1..=4).contains(&level) {
        return Err(ContractError::InvalidAuthLevel);
    }
    Ok(())
}

fn validate_level4_attributes(request: &AccessRequest) -> Result<(), ContractError> {
    // Require at least two public inputs at level 4:
    // - primary operation binding
    // - privacy-preserving attribute commitment
    if request.public_inputs.len() < 2 {
        return Err(ContractError::ProofRequiredForAuthLevel);
    }
    Ok(())
}

#[contractimpl]
impl ZkVerifierContract {
    /// Initialize the zk verifier contract
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(ContractError::AlreadyInitialized);
        }

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);
        env.storage().instance().set(&PROOF_CTR, &0u64);

    /// Store the Groth16 verification key used by `verify_access`.
    ///
    /// Only the admin may call this.  The key can be updated at any time
    /// (e.g. after a trusted-setup ceremony rotation).
    pub fn set_verification_key(
        env: Env,
        caller: Address,
        vk: VerificationKey,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller)?;
        env.storage().instance().set(&VK, &vk);
        Ok(())
    }

    /// Retrieve the current verification key, if one has been set.
    pub fn get_verification_key(env: Env) -> Option<VerificationKey> {
        env.storage().instance().get(&VK)
    }

    fn require_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
        caller.require_auth();

        let admin: Address = match env.storage().instance().get(&ADMIN) {
            Some(admin) => admin,
            None => return Self::unauthorized(env, caller, action, "initialized_admin"),
        };

        if caller != &admin {
            return Self::unauthorized(env, caller, action, "current_admin");
        }

        Ok(())
    }

    /// Propose a new admin address. Only the current admin can call this.
    /// The new admin must call `accept_admin` to complete the transfer.
    pub fn propose_admin(
        env: Env,
        current_admin: Address,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &current_admin, "propose_admin")?;

        env.storage().instance().set(&PENDING_ADMIN, &new_admin);

        events::publish_admin_transfer_proposed(&env, current_admin, new_admin);

        Ok(())
    }

    /// Accept the pending admin transfer. Only the proposed new admin can call this.
    /// Completes the two-step admin transfer process.
    pub fn accept_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        new_admin.require_auth();

        let pending: Address = env
            .storage()
            .instance()
            .get(&PENDING_ADMIN)
            .ok_or(ContractError::InvalidConfig)?;

        if new_admin != pending {
            return Self::unauthorized(&env, &new_admin, "accept_admin", "pending_admin");
        }

        let old_admin: Address = match env.storage().instance().get(&ADMIN) {
            Some(admin) => admin,
            None => {
                return Self::unauthorized(&env, &new_admin, "accept_admin", "initialized_admin")
            }
        };

        env.storage().instance().set(&ADMIN, &new_admin);
        env.storage().instance().remove(&PENDING_ADMIN);

        events::publish_admin_transfer_accepted(&env, old_admin, new_admin);

        Ok(())
    }

    /// Cancel a pending admin transfer. Only the current admin can call this.
    pub fn cancel_admin_transfer(env: Env, current_admin: Address) -> Result<(), ContractError> {
        Self::require_admin(&env, &current_admin, "cancel_admin_transfer")?;

        let pending: Address = env
            .storage()
            .instance()
            .get(&PENDING_ADMIN)
            .ok_or(ContractError::InvalidConfig)?;

        env.storage().instance().remove(&PENDING_ADMIN);

        events::publish_admin_transfer_cancelled(&env, current_admin, pending);

        Ok(())
    }

    /// Get the pending admin address, if any.
    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&PENDING_ADMIN)
    }


    /// Configure per-address rate limiting for this contract.
    pub fn set_rate_limit_config(
        env: Env,
        caller: Address,
        max_requests_per_window: u64,
        window_duration_seconds: u64,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller, "set_rate_limit_config")?;

        if max_requests_per_window == 0 || window_duration_seconds == 0 {
            return Err(ContractError::InvalidConfig);
        }

        let mut proof_ids = Vec::new(&env);

        for i in 0..proofs.len() {
            let proof = proofs.get(i).unwrap().clone();
            let public_inputs = public_inputs_batch.get(i).unwrap().clone();

            let proof_id = Self::verify_proof(env.clone(), submitter.clone(), proof, public_inputs)?;
            proof_ids.push_back(proof_id);
        }

        Ok(proof_ids)
    }

    /// Sets the ZK Verification Key for Groth16.
    pub fn set_verification_key(
        env: Env,
        caller: Address,
        vk: VerificationKey,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller, "set_verification_key")?;
        env.storage().instance().set(&symbol_short!("VK"), &vk);
        Ok(())
    }

    /// Gets the configured Verification Key.
    pub fn get_verification_key(env: Env) -> Option<VerificationKey> {
        env.storage().instance().get(&symbol_short!("VK"))
    }
    /// Return the current rate limiting configuration, if any.
    pub fn get_rate_limit_config(env: Env) -> Option<(u64, u64)> {
        env.storage().instance().get(&RATE_CFG)
    }

    /// Enables or disables whitelist enforcement.
    pub fn set_whitelist_enabled(
        env: Env,
        caller: Address,
        enabled: bool,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller, "set_whitelist_enabled")?;
        whitelist::set_whitelist_enabled(&env, enabled);
        Ok(())
    }

    /// Adds an address to the whitelist.
    pub fn add_to_whitelist(env: Env, caller: Address, user: Address) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller, "add_to_whitelist")?;
        whitelist::add_to_whitelist(&env, &user);
        Ok(())
    }

    /// Removes an address from the whitelist.
    pub fn remove_from_whitelist(
        env: Env,
        caller: Address,
        user: Address,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller, "remove_from_whitelist")?;
        whitelist::remove_from_whitelist(&env, &user);
        Ok(())
    }

    /// Check if contract is initialized
    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&INITIALIZED)
    }

    // ======================== Two-Phase Commit Hooks ========================

    /// Prepare phase for proof verification
    pub fn prepare_verify_proof(
        env: Env,
        submitter: Address,
        proof: Proof,
        public_inputs: Vec<BytesN<32>>,
    ) -> Result<u64, ContractError> {
        Self::require_initialized(&env)?;

        if public_inputs.is_empty() {
            return Err(ContractError::InvalidInput);
        }

        let proof_id: u64 = env
            .storage()
            .instance()
            .get(&PROOF_CTR)
            .unwrap_or(0u64)
            .saturating_add(1u64);

        let prep_key = (symbol_short!("PREP_VFY"), proof_id);
        let prep_data = PrepareVerification {
            proof_id,
            submitter: submitter.clone(),
            proof: proof.clone(),
            public_inputs: public_inputs.clone(),
            timestamp: env.ledger().timestamp(),
        };
        env.storage().temporary().set(&prep_key, &prep_data);

        Ok(proof_id)
    }

    pub fn get_nonce(env: Env, user: Address) -> u64 {
        let key = (symbol_short!("NONCE"), user);
        env.storage().persistent().get(&key).unwrap_or(0u64)
    }

    fn check_and_update_rate_limit(env: &Env, user: &Address) -> Result<(), ContractError> {
        let cfg: Option<(u64, u64)> = env.storage().instance().get(&RATE_CFG);
        let (max_requests_per_window, window_duration_seconds) = match cfg {
            Some(c) => c,
            None => return Ok(()),
        };

        let key = (VFY_RES, proof_id);
        env.storage().persistent().set(&key, &result);

        audit::AuditTrail::log_verification(&env, &prep_data.submitter, proof_id, verified);

        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    /// Rollback phase for proof verification
    pub fn rollback_verify_proof(env: Env, proof_id: u64) -> Result<(), ContractError> {
        let prep_key = (symbol_short!("PREP_VFY"), proof_id);
        env.storage().temporary().remove(&prep_key);
        Ok(())
    }

    /// Prepare phase for batch verification
    pub fn prepare_batch_verify_proofs(
        env: Env,
        submitter: Address,
        proofs: Vec<Proof>,
        public_inputs_batch: Vec<Vec<BytesN<32>>>,
    ) -> Result<Vec<u64>, ContractError> {
        Self::require_initialized(&env)?;

        if proofs.len() != public_inputs_batch.len() {
            return Err(ContractError::InvalidInput);
        }

        let mut proof_ids = Vec::new(&env);
        let mut start_proof_id: u64 = env
            .storage()
            .instance()
            .get(&PROOF_CTR)
            .unwrap_or(0u64);

        for i in 0..proofs.len() {
            let public_inputs = public_inputs_batch.get(i).unwrap().clone();

            if public_inputs.is_empty() {
                return Err(ContractError::InvalidInput);
            }

            start_proof_id = start_proof_id.saturating_add(1);
            proof_ids.push_back(start_proof_id);

            let prep_key = (symbol_short!("PREP_BVF"), start_proof_id);
            let prep_data = PrepareVerification {
                proof_id: start_proof_id,
                submitter: submitter.clone(),
                proof: proofs.get(i).unwrap().clone(),
                public_inputs,
                timestamp: env.ledger().timestamp(),
            };
            env.storage().temporary().set(&prep_key, &prep_data);
        }

        Ok(proof_ids)
    }

    fn validate_and_increment_nonce(
        env: &Env,
        user: &Address,
        provided_nonce: u64,
    ) -> Result<(), ContractError> {
        let key = (symbol_short!("NONCE"), user.clone());
        let current = env.storage().persistent().get::<_, u64>(&key).unwrap_or(0);

        if provided_nonce != current {
            return Err(ContractError::InvalidNonce);
        }

        // Increment nonce and persist
        env.storage().persistent().set(&key, &(current + 1));
        Ok(())
    }

    /// Verifies a ZK proof for resource access.
    ///
    /// This is the primary entry point for users to gain access to protected resources.
    /// It performs the following steps:
    /// 1. Authorizes the user.
    /// 2. Validates the request shape.
    /// 3. Checks whitelist and rate limits.
    /// 4. Verifies the Groth16 proof via `Bn254Verifier`.
    /// 5. Logs the access in the `AuditTrail` if successful.
    ///
    /// Returns `true` if the proof is valid and all checks pass, otherwise returns an error or `false`.
    pub fn verify_access(env: Env, request: AccessRequest) -> Result<bool, ContractError> {
        common::pausable::require_not_paused(&env).map_err(|_| ContractError::Paused)?;
        request.user.require_auth();

        validate_request(&request).map_err(|err| {
            events::publish_access_rejected(
                &env,
                request.user.clone(),
                request.resource_id.clone(),
                err,
            );
            err
        })?;

        if env.ledger().timestamp() > request.expires_at {
            let err = ContractError::ExpiredProof;
        Self::validate_and_increment_nonce(&env, &request.user, request.nonce).map_err(|_| {
            events::publish_access_rejected(
                &env,
                request.user.clone(),
                request.resource_id.clone(),
                err,
            );
            return Err(err);
        }
                ContractError::InvalidNonce,
            );
            ContractError::InvalidNonce
        })?;

        if !whitelist::check_whitelist_access(&env, &request.user) {
            events::publish_access_rejected(
                &env,
                request.user.clone(),
                request.resource_id.clone(),
                ContractError::Unauthorized,
            );
            return Self::unauthorized(&env, &request.user, "verify_access", "whitelisted_user");
        }

        Self::check_and_update_rate_limit(&env, &request.user).map_err(|err| {
            events::publish_access_rejected(
                &env,
                request.user.clone(),
                request.resource_id.clone(),
                err,
            );
            err
        })?;

        Bn254Verifier::validate_proof_components(&request.proof, &request.public_inputs)
            .map_err(map_proof_validation_error)?;

        // TODO: post-quantum migration - The verification branch below is hardcoded for BN254 Groth16.
        // During migration, checking `request.proof_type` should branch to `PostQuantumVerifier::verify_proof`
        // or a native host-function call if STARK verification limits CPU budgets.
        let vk = Self::get_verification_key(env.clone()).ok_or(ContractError::InvalidConfig)?;
        let is_valid =
            Bn254Verifier::verify_proof(&env, &vk, &request.proof, &request.public_inputs);
        if is_valid {
            let proof_hash = PoseidonHasher::hash(&env, &request.public_inputs);
            AuditTrail::log_access(&env, request.user, request.resource_id, proof_hash, request.expires_at);
        } else {
            Self::emit_access_violation(
                &env,
                &request.user,
                "verify_access",
                "valid_groth16_proof",
            );
        }
        Ok(())
    }

    /// Verifies access with auth-level-aware ZK requirements.
    ///
    /// Level mapping:
    /// - 1/2/3: standard proof verification path
    /// - 4: requires additional attribute proof material in public inputs
    pub fn verify_auth_level_access(
        env: Env,
        request: AccessRequest,
        required_auth_level: u32,
    ) -> Result<bool, ContractError> {
        validate_auth_level(required_auth_level)?;

        if required_auth_level >= 4 {
            validate_level4_attributes(&request)?;
        }

        Self::verify_access(env, request)
    }

    /// Retrieves an audit record for a specific user and resource.
    ///
    /// Returns the most recent `AuditRecord` if it exists, otherwise `None`.
    pub fn get_audit_record(
        env: Env,
        user: Address,
        resource_id: BytesN<32>,
    ) -> Option<AuditRecord> {
        AuditTrail::get_record(&env, user, resource_id)
    }

    /// Verifies access for a delegated computation.
    /// This allows off-chain executors to verify proofs on behalf of users.
    pub fn verify_delegated_access(
        env: Env,
        executor: Address,
        request: AccessRequest,
    ) -> Result<bool, ContractError> {
        executor.require_auth();
        // Additional checks for authorized executors can be added here
        Self::verify_access(env, request)
    /// Verifies the integrity of the audit chain for a given user and resource.
    ///
    /// Returns `true` if all hash links are valid, or if the chain is empty.
    pub fn verify_audit_chain(env: Env, user: Address, resource_id: BytesN<32>) -> bool {
        AuditTrail::verify_chain(&env, user, resource_id)
    }

    // ── Credential schema management ─────────────────────────────────────────

    /// Register a new credential schema. Only admin can register schemas.
    pub fn register_schema(
        env: Env,
        caller: Address,
        schema: CredentialSchema,
    ) -> Result<(), CredentialContractError> {
        Self::require_admin(&env, &caller, "register_schema")
            .map_err(|_| CredentialContractError::NotIssuer)?;

        CredentialManager::register_schema(&env, &schema)?;

        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("SCH_REG"), schema.issuer.clone()),
            schema.schema_id.clone(),
        );

        Ok(())
    }

    /// Retrieve a credential schema by ID.
    pub fn get_schema(
        env: Env,
        schema_id: BytesN<32>,
    ) -> Result<CredentialSchema, CredentialContractError> {
        CredentialManager::get_schema(&env, &schema_id)
    }

    /// List all schema IDs registered by a given issuer.
    pub fn get_issuer_schemas(env: Env, issuer: Address) -> Vec<BytesN<32>> {
        CredentialManager::get_issuer_schemas(&env, &issuer)
    }

    // ── Credential issuance ──────────────────────────────────────────────────

    /// Issue a new verifiable credential to a holder.
    ///
    /// The issuer provides a ZK commitment to all claim values. Actual values
    /// are never stored on-chain.
    pub fn issue_credential(
        env: Env,
        caller: Address,
        credential: Credential,
    ) -> Result<(), CredentialContractError> {
        caller.require_auth();

        // Only the issuer listed in the credential can issue it.
        if caller != credential.issuer {
            return Err(CredentialContractError::NotIssuer);
        }

        CredentialManager::issue_credential(&env, &credential)?;

        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("CRD_ISS"), credential.holder.clone()),
            credential.credential_id.clone(),
        );

        Ok(())
    }

    /// Issue a credential based on proof of an existing credential (chaining).
    ///
    /// The parent presentation is verified before the child is issued.
    pub fn issue_chained_credential(
        env: Env,
        caller: Address,
        request: ChainedIssuanceRequest,
        new_credential_id: BytesN<32>,
        holder: Address,
        revocation_index: u64,
    ) -> Result<Credential, CredentialContractError> {
        caller.require_auth();

        let child = CredentialManager::issue_chained_credential(
            &env,
            &caller,
            &request,
            new_credential_id,
            &holder,
            revocation_index,
        )?;

        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("CRD_CHN"), holder),
            child.credential_id.clone(),
        );

        Ok(child)
    }

    // ── Credential queries ───────────────────────────────────────────────────

    /// Retrieve a credential by its ID.
    pub fn get_credential(
        env: Env,
        credential_id: BytesN<32>,
    ) -> Result<Credential, CredentialContractError> {
        CredentialManager::get_credential(&env, &credential_id)
    }

    /// List all credential IDs held by a given address.
    pub fn get_holder_credentials(env: Env, holder: Address) -> Vec<BytesN<32>> {
        CredentialManager::get_holder_credentials(&env, &holder)
    }

    // ── Credential presentation & verification ───────────────────────────────

    /// Verify a credential presentation with selective disclosure.
    ///
    /// This is the primary entry point for verifiers. It checks:
    /// 1. Credential existence and active status
    /// 2. Schema binding
    /// 3. Holder binding
    /// 4. Selective disclosure proofs
    /// 5. Predicate proofs
    /// 6. Non-revocation proof
    pub fn verify_presentation(
        env: Env,
        presentation: CredentialPresentation,
    ) -> Result<bool, CredentialContractError> {
        let result = CredentialManager::verify_presentation(&env, &presentation)?;

        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("CRD_VER"), presentation.holder.clone()),
            presentation.credential_id.clone(),
        );

        Ok(result)
    }

    /// Verify multiple credential presentations in a batch.
    ///
    /// More efficient than individual verification because shared state
    /// lookups (schemas, registries) are amortized across the batch.
    pub fn batch_verify_presentations(
        env: Env,
        presentations: Vec<CredentialPresentation>,
    ) -> Result<BatchVerificationResult, CredentialContractError> {
        let result = CredentialManager::batch_verify_presentations(&env, &presentations)?;

        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("CRD_BAT"),),
            (result.total, result.verified, result.failed),
        );

        Ok(result)
    }

    // ── Revocation registry ──────────────────────────────────────────────────

    /// Create a new revocation registry. Only admin can create registries.
    pub fn create_revocation_registry(
        env: Env,
        caller: Address,
        registry_id: BytesN<32>,
    ) -> Result<RevocationRegistry, CredentialContractError> {
        Self::require_admin(&env, &caller, "create_revocation_registry")
            .map_err(|_| CredentialContractError::NotIssuer)?;

        let registry =
            RevocationRegistryManager::create_registry(&env, registry_id.clone(), &caller)?;

        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("REG_NEW"), caller),
            registry_id,
        );

        Ok(registry)
    }

    /// Get a revocation registry by ID.
    pub fn get_revocation_registry(
        env: Env,
        registry_id: BytesN<32>,
    ) -> Result<RevocationRegistry, CredentialContractError> {
        RevocationRegistryManager::get_registry(&env, &registry_id)
    }

    /// Generate a non-revocation witness for a credential.
    pub fn generate_revocation_witness(
        env: Env,
        caller: Address,
        registry_id: BytesN<32>,
        credential_id: BytesN<32>,
        index: u64,
    ) -> Result<RevocationWitness, CredentialContractError> {
        Self::require_admin(&env, &caller, "generate_revocation_witness")
            .map_err(|_| CredentialContractError::NotIssuer)?;

        RevocationRegistryManager::generate_witness(&env, &registry_id, &credential_id, index)
    }

    /// Revoke a credential by updating the revocation registry accumulator.
    ///
    /// After revocation, the credential's non-revocation witness will no longer
    /// verify against the updated accumulator.
    pub fn revoke_credential(
        env: Env,
        caller: Address,
        registry_id: BytesN<32>,
        credential_id: BytesN<32>,
        index: u64,
    ) -> Result<(), CredentialContractError> {
        Self::require_admin(&env, &caller, "revoke_credential")
            .map_err(|_| CredentialContractError::NotIssuer)?;

        // Update credential status.
        CredentialManager::update_status(
            &env,
            &credential_id,
            CredentialStatus::Revoked,
        )?;

        // Update revocation registry.
        RevocationRegistryManager::revoke_credential(
            &env,
            &registry_id,
            &credential_id,
            index,
        )?;

        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("CRD_REV"), credential_id.clone()),
            index,
        );

        Ok(())
    }

    /// Check if a credential is revoked in a registry.
    pub fn is_credential_revoked(
        env: Env,
        registry_id: BytesN<32>,
        index: u64,
    ) -> bool {
        RevocationRegistryManager::is_revoked(&env, &registry_id, index)
    }
}
