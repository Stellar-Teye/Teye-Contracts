#![no_std]

pub mod credential;
pub mod recovery;

use credential::CredentialError;
use recovery::{RecoveryError, RecoveryRequest};
use soroban_sdk::{contract, contractimpl, symbol_short, Address, BytesN, Env, Symbol, Vec};
use zk_verifier::vk::{G1Point as VkG1Point, G2Point as VkG2Point};

// ── Storage keys ─────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");

/// Re-export credential error for downstream consumers.
pub use credential::CredentialError as CredentialVerificationError;

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct IdentityContract;

#[contractimpl]
impl IdentityContract {
    /// Initialize the identity contract with an owner address.
    pub fn initialize(env: Env, owner: Address) -> Result<(), RecoveryError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(RecoveryError::AlreadyInitialized);
        }

        env.storage().instance().set(&ADMIN, &owner);
        env.storage().instance().set(&INITIALIZED, &true);
        recovery::set_owner_active(&env, &owner);

        Ok(())
    }

    /// Add a guardian address for social recovery (max 5).
    pub fn add_guardian(env: Env, caller: Address, guardian: Address) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;
        recovery::add_guardian(&env, &caller, guardian)
    }

    /// Remove a guardian address.
    pub fn remove_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;
        recovery::remove_guardian(&env, &caller, &guardian)
    }

    /// Set the M-of-N approval threshold for recovery.
    pub fn set_recovery_threshold(
        env: Env,
        caller: Address,
        threshold: u32,
    ) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;
        recovery::set_threshold(&env, &caller, threshold)
    }

    /// A guardian initiates recovery, proposing a new address.
    /// The initiating guardian counts as the first approval.
    pub fn initiate_recovery(
        env: Env,
        guardian: Address,
        owner: Address,
        new_address: Address,
    ) -> Result<(), RecoveryError> {
        guardian.require_auth();
        recovery::initiate_recovery(&env, &guardian, &owner, new_address)
    }

    /// A guardian approves an active recovery request.
    pub fn approve_recovery(
        env: Env,
        guardian: Address,
        owner: Address,
    ) -> Result<(), RecoveryError> {
        guardian.require_auth();
        recovery::approve_recovery(&env, &guardian, &owner)
    }

    /// Execute recovery after cooldown and sufficient approvals.
    /// Transfers identity ownership and deactivates the old address.
    pub fn execute_recovery(
        env: Env,
        caller: Address,
        owner: Address,
    ) -> Result<Address, RecoveryError> {
        caller.require_auth();
        recovery::execute_recovery(&env, &owner)
    }

    /// Owner cancels an active recovery request.
    pub fn cancel_recovery(env: Env, caller: Address) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;
        recovery::cancel_recovery(&env, &caller)
    }

    /// Check if an address is an active identity owner.
    pub fn is_owner_active(env: Env, owner: Address) -> bool {
        recovery::is_owner_active(&env, &owner)
    }

    /// Get the list of guardians for an owner.
    pub fn get_guardians(env: Env, owner: Address) -> Vec<Address> {
        recovery::get_guardians(&env, &owner)
    }

    /// Get the recovery threshold for an owner.
    pub fn get_recovery_threshold(env: Env, owner: Address) -> u32 {
        recovery::get_threshold(&env, &owner)
    }

    /// Get the active recovery request for an owner, if any.
    pub fn get_recovery_request(env: Env, owner: Address) -> Option<RecoveryRequest> {
        recovery::get_recovery_request(&env, &owner)
    }

    // ── ZK credential verification ────────────────────────────────────────────

    /// Set the address of the deployed `zk_verifier` contract.
    /// Only an active owner can call this.
    pub fn set_zk_verifier(
        env: Env,
        caller: Address,
        verifier_id: Address,
    ) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;
        credential::set_zk_verifier(&env, &verifier_id);
        Ok(())
    }

    /// Get the stored `zk_verifier` contract address.
    pub fn get_zk_verifier(env: Env) -> Option<Address> {
        credential::get_zk_verifier(&env)
    }

    /// Verify a ZK credential proof without revealing the credential on-chain.
    ///
    /// Delegates verification to the configured `zk_verifier` contract via a
    /// cross-contract call. Only the verification result and a privacy-preserving
    /// event (user + resource hash) are recorded.
    pub fn verify_zk_credential(
        env: Env,
        user: Address,
        resource_id: BytesN<32>,
        proof_a: VkG1Point,
        proof_b: VkG2Point,
        proof_c: VkG1Point,
        public_inputs: Vec<BytesN<32>>,
    ) -> Result<bool, CredentialError> {
        user.require_auth();
        credential::verify_zk_credential(
            &env,
            &user,
            resource_id,
            proof_a,
            proof_b,
            proof_c,
            public_inputs,
        )
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    fn require_active_owner(env: &Env, caller: &Address) -> Result<(), RecoveryError> {
        if !recovery::is_owner_active(env, caller) {
            return Err(RecoveryError::Unauthorized);
        }
        Ok(())
    }
}
