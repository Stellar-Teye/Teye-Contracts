#![no_std]

pub mod recovery;

use recovery::{RecoveryError, RecoveryRequest};
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol, Vec};

// ── Storage keys ─────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");

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
    pub fn add_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
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

    // ── Internal helpers ─────────────────────────────────────────────────────

    fn require_active_owner(env: &Env, caller: &Address) -> Result<(), RecoveryError> {
        if !recovery::is_owner_active(env, caller) {
            return Err(RecoveryError::Unauthorized);
        }
        Ok(())
    }
}
