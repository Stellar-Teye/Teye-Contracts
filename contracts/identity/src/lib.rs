#![no_std]

pub mod credential;
pub mod events;
pub mod recovery;

use credential::CredentialError;
use recovery::{RecoveryError, RecoveryRequest};
// FIX: Removed unused String import to clear warning
use soroban_sdk::{BytesN, contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec};

/// Preparation data for guardian addition
#[contracttype]
#[derive(Clone, Debug)]
pub struct PrepareGuardianAddition {
    pub caller: Address,
    pub guardian: Address,
    pub timestamp: u64,
}

/// Preparation data for guardian removal
#[contracttype]
#[derive(Clone, Debug)]
pub struct PrepareGuardianRemoval {
    pub caller: Address,
    pub guardian: Address,
    pub timestamp: u64,
}

/// Preparation data for recovery threshold change
#[contracttype]
#[derive(Clone, Debug)]
pub struct PrepareThresholdChange {
    pub caller: Address,
    pub threshold: u32,
    pub timestamp: u64,
}

// ── Storage keys ─────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");
const HOLDER_BIND_PREFIX: &str = "HLD_BIND";

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
        let result = recovery::add_guardian(&env, &caller, guardian.clone());
        if result.is_ok() {
            events::emit_guardian_changed(&env, caller, guardian, true);
        }
        result
    }

    /// Remove a guardian address.
    pub fn remove_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;
        let result = recovery::remove_guardian(&env, &caller, &guardian);
        if result.is_ok() {
            events::emit_guardian_changed(&env, caller, guardian, false);
        }
        result
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

    /// FIX: Renamed to id_initiate_recovery to prevent WASM symbol collision with key_manager
    pub fn id_initiate_recovery(
        env: Env,
        guardian: Address,
        owner: Address,
        new_address: Address,
    ) -> Result<(), RecoveryError> {
        guardian.require_auth();
        let result = recovery::initiate_recovery(&env, &guardian, &owner, new_address.clone());
        if result.is_ok() {
            events::emit_recovery_initiated(&env, owner, new_address, guardian);
        }
        result
    }

    /// FIX: Renamed to id_approve_recovery to prevent WASM symbol collision with key_manager
    pub fn id_approve_recovery(
        env: Env,
        guardian: Address,
        owner: Address,
    ) -> Result<(), RecoveryError> {
        guardian.require_auth();
        recovery::approve_recovery(&env, &guardian, &owner)
    }

    /// FIX: Renamed to id_execute_recovery to prevent WASM symbol collision with key_manager
    pub fn id_execute_recovery(
        env: Env,
        caller: Address,
        owner: Address,
    ) -> Result<Address, RecoveryError> {
        caller.require_auth();
        let result = recovery::execute_recovery(&env, &owner);
        if let Ok(ref new_addr) = result {
            events::emit_recovery_executed(&env, owner, new_addr.clone());
        }
        result
    }

    /// Owner cancels an active recovery request.
    pub fn cancel_recovery(env: Env, caller: Address) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;
        let result = recovery::cancel_recovery(&env, &caller);
        if result.is_ok() {
            events::emit_recovery_cancelled(&env, caller);
        }
        result
    }

    /// Check if an address is an active identity owner.
    pub fn is_owner_active(env: Env, owner: Address) -> bool {
        recovery::is_owner_active(&env, &owner)
    }

    /// Get the list of guardians for an owner.
    pub fn get_guardians(env: Env, owner: Address) -> Vec<Address> {
        recovery::get_guardians(&env, &owner)
    }

    /// Check if a guardian is registered for an owner.
    pub fn is_guardian(env: Env, owner: Address, guardian: Address) -> bool {
        recovery::get_guardians(&env, &owner).contains(&guardian)
    }

    /// Get the recovery threshold for an owner.
    pub fn get_recovery_threshold(env: Env, owner: Address) -> u32 {
        recovery::get_threshold(&env, &owner)
    }

    /// Get the active recovery request for an owner, if any.
    pub fn get_recovery_request(env: Env, owner: Address) -> Option<RecoveryRequest> {
        recovery::get_recovery_request(&env, &owner)
    }

    // ===== Two-Phase Commit Hooks =====

    /// Prepare phase for add_guardian operation
    pub fn prepare_add_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        Self::require_active_owner(&env, &caller)?;
        
        let guardians = recovery::get_guardians(&env, &caller);
        if guardians.contains(&guardian) {
            return Err(RecoveryError::DuplicateGuardian);
        }

        if guardians.len() >= 5 {
            return Err(RecoveryError::MaxGuardiansReached);
        }

        let prep_key = (Symbol::new(&env, "PREP_ADD_GUARD"), caller.clone(), guardian.clone());
        let prep_data = PrepareGuardianAddition {
            caller: caller.clone(),
            guardian: guardian.clone(),
            timestamp: env.ledger().timestamp(),
        };
        env.storage().temporary().set(&prep_key, &prep_data);

        Ok(())
    }

    pub fn commit_add_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        let prep_key = (Symbol::new(&env, "PREP_ADD_GUARD"), caller.clone(), guardian.clone());
        // FIXED: Added '?' to extract the struct from the Result
        let prep_data: PrepareGuardianAddition = env.storage().temporary().get(&prep_key)
            .ok_or(RecoveryError::Unauthorized)?;

        if prep_data.caller != caller || prep_data.guardian != guardian {
            return Err(RecoveryError::Unauthorized);
        }

        recovery::add_guardian(&env, &caller, guardian.clone())?;
        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    pub fn rollback_add_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        let prep_key = (Symbol::new(&env, "PREP_ADD_GUARD"), caller, guardian);
        env.storage().temporary().remove(&prep_key);
        Ok(())
    }

    pub fn prepare_remove_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        Self::require_active_owner(&env, &caller)?;
        
        let guardians = recovery::get_guardians(&env, &caller);
        if !guardians.contains(&guardian) {
            return Err(RecoveryError::GuardianNotFound);
        }

        let prep_key = (Symbol::new(&env, "PREP_REM_GUARD"), caller.clone(), guardian.clone());
        let prep_data = PrepareGuardianRemoval {
            caller: caller.clone(),
            guardian: guardian.clone(),
            timestamp: env.ledger().timestamp(),
        };
        env.storage().temporary().set(&prep_key, &prep_data);

        Ok(())
    }

    pub fn commit_remove_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        let prep_key = (Symbol::new(&env, "PREP_REM_GUARD"), caller.clone(), guardian.clone());
        // FIXED: Added '?' to extract the struct from the Result
        let prep_data: PrepareGuardianRemoval = env.storage().temporary().get(&prep_key)
            .ok_or(RecoveryError::Unauthorized)?;

        if prep_data.caller != caller || prep_data.guardian != guardian {
            return Err(RecoveryError::Unauthorized);
        }

        recovery::remove_guardian(&env, &caller, &guardian)?;
        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    pub fn rollback_remove_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        let prep_key = (Symbol::new(&env, "PREP_REM_GUARD"), caller, guardian);
        env.storage().temporary().remove(&prep_key);
        Ok(())
    }

    pub fn prepare_set_recovery_threshold(
        env: Env,
        caller: Address,
        threshold: u32,
    ) -> Result<(), RecoveryError> {
        Self::require_active_owner(&env, &caller)?;
        
        if threshold == 0 || threshold > 5 {
            return Err(RecoveryError::InvalidThreshold);
        }

        let guardians = recovery::get_guardians(&env, &caller);
        if threshold > guardians.len() as u32 {
            return Err(RecoveryError::InvalidThreshold);
        }

        let prep_key = (Symbol::new(&env, "PREP_SET_THRESH"), caller.clone());
        let prep_data = PrepareThresholdChange {
            caller: caller.clone(),
            threshold,
            timestamp: env.ledger().timestamp(),
        };
        env.storage().temporary().set(&prep_key, &prep_data);

        Ok(())
    }

    pub fn commit_set_recovery_threshold(
        env: Env,
        caller: Address,
        threshold: u32,
    ) -> Result<(), RecoveryError> {
        let prep_key = (Symbol::new(&env, "PREP_SET_THRESH"), caller.clone());
        // FIXED: Added '?' to extract the struct from the Result
        let prep_data: PrepareThresholdChange = env.storage().temporary().get(&prep_key)
            .ok_or(RecoveryError::Unauthorized)?;

        if prep_data.caller != caller || prep_data.threshold != threshold {
            return Err(RecoveryError::Unauthorized);
        }

        recovery::set_threshold(&env, &caller, threshold)?;
        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    pub fn rollback_set_recovery_threshold(
        env: Env,
        caller: Address,
        _threshold: u32,
    ) -> Result<(), RecoveryError> {
        let prep_key = (Symbol::new(&env, "PREP_SET_THRESH"), caller);
        env.storage().temporary().remove(&prep_key);
        Ok(())
    }

    // ── ZK credential verification ────────────────────────────────────────────

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

    pub fn get_zk_verifier(env: Env) -> Option<Address> {
        credential::get_zk_verifier(&env)
    }

    pub fn verify_zk_credential(
        env: Env,
        user: Address,
        resource_id: BytesN<32>,
        proof_a: soroban_sdk::Bytes,
        proof_b: soroban_sdk::Bytes,
        proof_c: soroban_sdk::Bytes,
        public_inputs: Vec<BytesN<32>>,
        expires_at: u64,
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
            expires_at,
            0,
        )
    }

    // ── Credential holder binding ────────────────────────────────────────────

    pub fn bind_credential(
        env: Env,
        caller: Address,
        credential_id: BytesN<32>,
    ) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;

        let key = (Symbol::new(&env, HOLDER_BIND_PREFIX), caller.clone());
        let mut creds: Vec<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));

        if !creds.contains(&credential_id) {
            creds.push_back(credential_id.clone());
            env.storage().persistent().set(&key, &creds);
        }

        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("CRD_BIND"), caller),
            credential_id,
        );

        Ok(())
    }

    pub fn unbind_credential(
        env: Env,
        caller: Address,
        credential_id: BytesN<32>,
    ) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;

        let key = (Symbol::new(&env, HOLDER_BIND_PREFIX), caller.clone());
        let creds: Vec<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));

        let mut new_creds = Vec::new(&env);
        for c in creds.iter() {
            if c != credential_id {
                new_creds.push_back(c);
            }
        }
        env.storage().persistent().set(&key, &new_creds);

        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("CRD_UBND"), caller),
            credential_id,
        );

        Ok(())
    }

    pub fn get_bound_credentials(env: Env, holder: Address) -> Vec<BytesN<32>> {
        let key = (Symbol::new(&env, HOLDER_BIND_PREFIX), holder);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env))
    }

    pub fn is_credential_bound(
        env: Env,
        holder: Address,
        credential_id: BytesN<32>,
    ) -> bool {
        let key = (Symbol::new(&env, HOLDER_BIND_PREFIX), holder);
        let creds: Vec<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));
        creds.contains(&credential_id)
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    fn require_active_owner(env: &Env, caller: &Address) -> Result<(), RecoveryError> {
        if !recovery::is_owner_active(env, caller) {
            return Err(RecoveryError::Unauthorized);
        }
        Ok(())
    }
}