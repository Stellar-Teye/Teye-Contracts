#![no_std]
#![allow(clippy::too_many_arguments)]
pub mod appointment;
pub mod audit;
pub mod circuit_breaker;
pub mod emergency;
pub mod errors;
pub mod events;
pub mod examination;
pub mod patient_profile;
pub mod prescription;
pub mod provider;
pub mod rate_limit;
pub mod rbac;
pub mod validation;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

use teye_common::whitelist;

/// Re-export the contract-specific error type at the crate root.
pub use errors::ContractError;

/// Re-export provider types needed by other modules (e.g. events).
pub use provider::VerificationStatus;

/// Re-export error helpers used throughout the contract.
pub use errors::{create_error_context, log_error};

/// Re-export types from submodules used directly in the contract impl.
pub use audit::{AccessAction, AccessResult};
pub use examination::{
    EyeExamination, IntraocularPressure, OptFundusPhotography, OptRetinalImaging, OptVisualField,
    SlitLampFindings, VisualAcuity,
};
pub use patient_profile::{
    EmergencyContact, InsuranceInfo, OptionalEmergencyContact, OptionalInsuranceInfo,
    PatientProfile,
};
pub use prescription::{LensType, OptionalContactLensData, Prescription, PrescriptionData};

/// Storage keys for the contract
const ADMIN: Symbol = symbol_short!("ADMIN");
const PENDING_ADMIN: Symbol = symbol_short!("PEND_ADM");
const INITIALIZED: Symbol = symbol_short!("INIT");
const RATE_CFG: Symbol = symbol_short!("RL_CFG");
const RATE_TRACK: Symbol = symbol_short!("RL_TRK");

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

const ENC_CUR: Symbol = symbol_short!("ENC_CUR");
const ENC_KEY: Symbol = symbol_short!("ENC_KEY");

/// Extends the time-to-live (TTL) for a storage key containing an Address.
/// This ensures the data remains accessible for the extended period.
fn extend_ttl_address_key(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Extends the time-to-live (TTL) for a storage key containing a u64 value.
/// This ensures the data remains accessible for the extended period.
fn extend_ttl_u64_key(env: &Env, key: &(Symbol, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Extends the time-to-live (TTL) for an access grant storage key.
/// This ensures access grant data remains accessible for the extended period.
fn extend_ttl_access_key(env: &Env, key: &(Symbol, Address, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_ttl_record_access_key(env: &Env, key: &(Symbol, u64, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn consent_key(patient: &Address, grantee: &Address) -> (Symbol, Address, Address) {
    (symbol_short!("CONSENT"), patient.clone(), grantee.clone())
}

fn has_active_consent(env: &Env, patient: &Address, grantee: &Address) -> bool {
    let key = consent_key(patient, grantee);
    if let Some(consent) = env.storage().persistent().get::<_, ConsentGrant>(&key) {
        !consent.revoked && consent.expires_at > env.ledger().timestamp()
    } else {
        false
    }
}

pub use rbac::{Permission, Role, AccessPolicy, PolicyContext, evaluate_access_policies, set_user_credential, set_record_sensitivity, create_access_policy, CredentialType, SensitivityLevel, TimeRestriction};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsentType {
    Treatment,
    Research,
    Sharing,
}

/// Access levels for record sharing
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessLevel {
    /// No access to the record
    None,
    /// Read-only access to the record
    Read,
    /// Write access to the record
    Write,
    /// Full access including read, write, and delete
    Full,
}

/// Vision record types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordType {
    /// Eye examination record
    Examination,
    /// Prescription record
    Prescription,
    /// Diagnosis record
    Diagnosis,
    /// Treatment record
    Treatment,
    /// Surgery record
    Surgery,
    /// Laboratory result record
    LabResult,
}

/// Status for emergency access grants
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmergencyStatus {
    Active,
    Revoked,
    Expired,
}

/// User information structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct User {
    pub address: Address,
    pub role: Role,
    pub name: String,
    pub registered_at: u64,
    pub is_active: bool,
}

/// Vision record structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct VisionRecord {
    pub id: u64,
    pub patient: Address,
    pub provider: Address,
    pub record_type: RecordType,
    pub data_hash: String,
    pub key_version: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Access grant structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct AccessGrant {
    pub patient: Address,
    pub grantee: Address,
    pub level: AccessLevel,
    pub granted_at: u64,
    pub expires_at: u64,
}

/// Consent grant structure for patient-to-provider consent tracking
#[contracttype]
#[derive(Clone, Debug)]
pub struct ConsentGrant {
    pub patient: Address,
    pub grantee: Address,
    pub consent_type: ConsentType,
    pub granted_at: u64,
    pub expires_at: u64,
    pub revoked: bool,
}

/// Input for batch record creation
#[contracttype]
#[derive(Clone, Debug)]
pub struct BatchRecordInput {
    pub patient: Address,
    pub record_type: RecordType,
    pub data_hash: String,
}

/// Input for batch access granting
#[contracttype]
#[derive(Clone, Debug)]
pub struct BatchGrantInput {
    pub grantee: Address,
    pub level: AccessLevel,
    pub duration_seconds: u64,
}

#[contract]
#[allow(clippy::too_many_arguments)]
pub struct VisionRecordsContract;

#[contractimpl]
#[allow(clippy::too_many_arguments)]
impl VisionRecordsContract {
    fn enforce_rate_limit(env: &Env, caller: &Address) -> Result<(), ContractError> {
        let cfg: Option<(u64, u64)> = env.storage().instance().get(&RATE_CFG);
        let (max_requests_per_window, window_duration_seconds) = match cfg {
            Some(c) => c,
            None => return Ok(()), // No config set -> unlimited
        };

        if max_requests_per_window == 0 || window_duration_seconds == 0 {
            // Explicitly disabled
            return Ok(());
        }

        let now = env.ledger().timestamp();
        let key = (RATE_TRACK, caller.clone());

        let mut state: (u64, u64) = env.storage().persistent().get(&key).unwrap_or((0, now));

        let window_end = state.1.saturating_add(window_duration_seconds);
        if now >= window_end {
            state.0 = 0;
            state.1 = now;
        }

        let next = state.0.saturating_add(1);
        if next > max_requests_per_window {
            return Err(ContractError::RateLimitExceeded);
        }

        state.0 = next;
        env.storage().persistent().set(&key, &state);

        Ok(())
    }

    /// Initialize the contract with an admin address
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&INITIALIZED) {
            let context = create_error_context(
                &env,
                ContractError::AlreadyInitialized,
                Some(admin.clone()),
                Some(String::from_str(&env, "initialize")),
            );
            log_error(
                &env,
                ContractError::AlreadyInitialized,
                Some(admin),
                None,
                None,
            );
            events::publish_error(&env, ContractError::AlreadyInitialized as u32, context);
            return Err(ContractError::AlreadyInitialized);
        }

        // admin.require_auth();

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);
        rbac::assign_role(&env, admin.clone(), Role::Admin, 0);

        // Bootstrap the admin with the Admin role so they can register other users
        rbac::assign_role(&env, admin.clone(), Role::Admin, 0);

        // Assign the Admin RBAC role so the admin has permissions
        rbac::assign_role(&env, admin.clone(), Role::Admin, 0);

        // Bootstrap the initializing admin as SuperAdmin in the tier system
        admin_tiers::set_super_admin(&env, &admin);
        admin_tiers::track_admin(&env, &admin);

        events::publish_initialized(&env, admin);

        Ok(())
    }

    /// Get the admin address
    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        match env.storage().instance().get(&ADMIN) {
            Some(admin) => Ok(admin),
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::NotInitialized,
                    None,
                    Some(String::from_str(&env, "get_admin")),
                );
                log_error(&env, ContractError::NotInitialized, None, None, None);
                events::publish_error(&env, ContractError::NotInitialized as u32, context);
                Err(ContractError::NotInitialized)
            }
        }
    }

    /// Check if the contract is initialized
    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&INITIALIZED)
    }

    /// Propose a new admin address. Only the current admin can call this.
    /// The new admin must call `accept_admin` to complete the transfer.
    pub fn propose_admin(
        env: Env,
        current_admin: Address,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        current_admin.require_auth();

        let admin = Self::get_admin(env.clone())?;
        if current_admin != admin {
            return Err(ContractError::Unauthorized);
        }

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
            .ok_or(ContractError::InvalidInput)?;

        if new_admin != pending {
            return Err(ContractError::Unauthorized);
        }

        let old_admin = Self::get_admin(env.clone())?;

        env.storage().instance().set(&ADMIN, &new_admin);
        env.storage().instance().remove(&PENDING_ADMIN);

        events::publish_admin_transfer_accepted(&env, old_admin, new_admin);

        Ok(())
    }

    /// Cancel a pending admin transfer. Only the current admin can call this.
    pub fn cancel_admin_transfer(
        env: Env,
        current_admin: Address,
    ) -> Result<(), ContractError> {
        current_admin.require_auth();

        let admin = Self::get_admin(env.clone())?;
        if current_admin != admin {
            return Err(ContractError::Unauthorized);
        }

        let pending: Address = env
            .storage()
            .instance()
            .get(&PENDING_ADMIN)
            .ok_or(ContractError::InvalidInput)?;

        env.storage().instance().remove(&PENDING_ADMIN);

        events::publish_admin_transfer_cancelled(&env, current_admin, pending);

        Ok(())
    }

    /// Get the pending admin address, if any.
    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&PENDING_ADMIN)
    }

    /// Configure per-address rate limiting for this contract.
    ///
    /// Requires at least `ContractAdmin` tier, or legacy admin/SystemAdmin.
    pub fn set_rate_limit_config(
        env: Env,
        caller: Address,
        max_requests_per_window: u64,
        window_duration_seconds: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        if max_requests_per_window == 0 || window_duration_seconds == 0 {
            return Err(ContractError::InvalidInput);
        }

        if !Self::has_admin_access(&env, &caller, &AdminTier::ContractAdmin) {
            return Err(ContractError::Unauthorized);
        }

        env.storage().instance().set(
            &RATE_CFG,
            &(max_requests_per_window, window_duration_seconds),
        );

        Ok(())
    }

    /// Set or rotate an encryption master key under a given `version`.
    /// Stores the key bytes persistently under (ENC_KEY, version) and updates current.
    pub fn set_encryption_key(
        env: Env,
        caller: Address,
        version: String,
        key: String,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let admin = Self::get_admin(env.clone())?;
        let has_system_admin = rbac::has_permission(&env, &caller, &Permission::SystemAdmin);
        if caller != admin && !has_system_admin {
            return Err(ContractError::Unauthorized);
        }

        // Persist the key hex string under (ENC_KEY, version)
        env.storage()
            .persistent()
            .set(&(ENC_KEY, version.clone()), &key);
        // Update current active version
        env.storage().instance().set(&ENC_CUR, &version);

        Ok(())
    }

    /// Return the current rate limiting configuration, if any.
    pub fn get_rate_limit_config(env: Env) -> Option<(u64, u64)> {
        env.storage().instance().get(&RATE_CFG)
    }

    /// Enables or disables whitelist enforcement globally.
    ///
    /// Requires at least `ContractAdmin` tier, or legacy admin/SystemAdmin.
    pub fn set_whitelist_enabled(
        env: Env,
        caller: Address,
        enabled: bool,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if !Self::has_admin_access(&env, &caller, &AdminTier::ContractAdmin) {
            return Err(ContractError::Unauthorized);
        }
        whitelist::set_whitelist_enabled(&env, enabled);
        Ok(())
    }

    /// Adds an address to the whitelist.
    ///
    /// Requires at least `ContractAdmin` tier, or legacy admin/SystemAdmin.
    pub fn add_to_whitelist(env: Env, caller: Address, user: Address) -> Result<(), ContractError> {
        caller.require_auth();
        if !Self::has_admin_access(&env, &caller, &AdminTier::ContractAdmin) {
            return Err(ContractError::Unauthorized);
        }
        whitelist::add_to_whitelist(&env, &user);
        Ok(())
    }

    /// Removes an address from the whitelist.
    ///
    /// Requires at least `ContractAdmin` tier, or legacy admin/SystemAdmin.
    pub fn remove_from_whitelist(
        env: Env,
        caller: Address,
        user: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if !Self::has_admin_access(&env, &caller, &AdminTier::ContractAdmin) {
            return Err(ContractError::Unauthorized);
        }
        whitelist::remove_from_whitelist(&env, &user);
        Ok(())
    }

    pub fn is_whitelist_enabled(env: Env) -> bool {
        whitelist::is_whitelist_enabled(&env)
    }

    pub fn is_whitelisted(env: Env, user: Address) -> bool {
        whitelist::is_whitelisted(&env, &user)
    }

    /// Register a new user
    pub fn register_user(
        env: Env,
        caller: Address,
        user: Address,
        role: Role,
        name: String,
    ) -> Result<(), ContractError> {
        circuit_breaker::require_not_paused(
            &env,
            &circuit_breaker::PauseScope::Function(symbol_short!("REG_USR")),
        )?;
        caller.require_auth();

        if !whitelist::check_whitelist_access(&env, &caller) {
            return Err(ContractError::Unauthorized);
        }

        // Unified check: covers direct role, custom grants, and delegated roles
        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            let resource_id = String::from_str(&env, "register_user");
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(resource_id.clone()),
            );
            log_error(
                &env,
                ContractError::Unauthorized,
                Some(caller),
                Some(resource_id),
                None,
            );
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        validation::validate_name(&name)?;

        let user_data = User {
            address: user.clone(),
            role: role.clone(),
            name: name.clone(),
            registered_at: env.ledger().timestamp(),
            is_active: true,
        };

        let key = (symbol_short!("USER"), user.clone());
        env.storage().persistent().set(&key, &user_data);
        extend_ttl_address_key(&env, &key);
        rbac::assign_role(&env, user.clone(), role.clone(), 0);

        rbac::assign_role(&env, user.clone(), role.clone(), 0);

        // Assign the role in the RBAC system
        rbac::assign_role(&env, user.clone(), role.clone(), 0);

        // Create the RBAC role assignment so has_permission works
        rbac::assign_role(&env, user.clone(), role.clone(), 0);

        events::publish_user_registered(&env, user, role, name);

        Ok(())
    }

    /// Get user information
    pub fn get_user(env: Env, user: Address) -> Result<User, ContractError> {
        let key = (symbol_short!("USER"), user.clone());
        match env.storage().persistent().get(&key) {
            Some(user_data) => Ok(user_data),
            None => {
                let resource_id = String::from_str(&env, "get_user");
                let context = create_error_context(
                    &env,
                    ContractError::UserNotFound,
                    Some(user.clone()),
                    Some(resource_id.clone()),
                );
                log_error(
                    &env,
                    ContractError::UserNotFound,
                    Some(user),
                    Some(resource_id),
                    None,
                );
                events::publish_error(&env, ContractError::UserNotFound as u32, context);
                Err(ContractError::UserNotFound)
            }
        }
    }

    /// Add a vision record
    #[allow(clippy::arithmetic_side_effects)]
    pub fn add_record(
        env: Env,
        caller: Address,
        patient: Address,
        provider: Address,
        record_type: RecordType,
        data_hash: String,
    ) -> Result<u64, ContractError> {
        circuit_breaker::require_not_paused(
            &env,
            &circuit_breaker::PauseScope::Function(symbol_short!("ADD_REC")),
        )?;
        caller.require_auth();

        if !whitelist::check_whitelist_access(&env, &caller) {
            return Err(ContractError::Unauthorized);
        }

        Self::enforce_rate_limit(&env, &caller)?;

        validation::validate_data_hash(&data_hash)?;

        // If caller is the provider, unified check covers direct + delegated WriteRecord.
        // Otherwise, check if this specific provider delegated to the caller.
        let has_perm = if caller == provider {
            rbac::has_permission(&env, &caller, &Permission::WriteRecord)
        } else {
            rbac::has_delegated_permission(&env, &provider, &caller, &Permission::WriteRecord)
        };

        // Fall back to SystemAdmin (unified: direct role + any delegation)
        if !has_perm && !rbac::has_permission(&env, &caller, &Permission::SystemAdmin) {
            // Log failed write attempt
            let audit_entry = audit::create_audit_entry(
                &env,
                caller.clone(),
                patient.clone(),
                None,
                AccessAction::Write,
                AccessResult::Denied,
                Some(String::from_str(&env, "Insufficient permissions")),
            );
            audit::add_audit_entry(&env, &audit_entry);
            events::publish_audit_log_entry(&env, &audit_entry);

            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "add_record")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        // Generate record ID
        let counter_key = symbol_short!("REC_CTR");
        let record_id: u64 = env.storage().instance().get(&counter_key).unwrap_or(0) + 1;
        env.storage().instance().set(&counter_key, &record_id);

        // Determine current encryption key version (if any) and load master bytes
        let current_version: Option<String> = env.storage().instance().get(&ENC_CUR);
        let mut master_bytes: StdVec<u8> = StdVec::new();
        if let Some(ver) = current_version.clone() {
            if let Some(sv) = env
                .storage()
                .persistent()
                .get::<(Symbol, String), String>(&(ENC_KEY, ver.clone()))
            {
                let hex = sv.to_string();
                if let Some(bytes) = common::hex_to_bytes(&hex) {
                    master_bytes = bytes;
                }
            }
        }

        // Build KeyManager and encrypt the provided data_hash
        let km = KeyManager::new(master_bytes);
        let plaintext: StdString = data_hash.to_string();
        let ciphertext = km.encrypt(None, &plaintext);
        let stored_hash = String::from_str(&env, &ciphertext);

        let record = VisionRecord {
            id: record_id,
            patient: patient.clone(),
            provider: provider.clone(),
            record_type: record_type.clone(),
            data_hash: stored_hash,
            key_version: current_version.clone(),
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
        };

        let key = (symbol_short!("RECORD"), record_id);
        env.storage().persistent().set(&key, &record);
        extend_ttl_u64_key(&env, &key);

        // Add to patient's record list
        let patient_key = (symbol_short!("PAT_REC"), patient.clone());
        let mut patient_records: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));
        patient_records.push_back(record_id);
        env.storage()
            .persistent()
            .set(&patient_key, &patient_records);

        Ok(record_id)
    }

    /// Add multiple vision records in a single transaction.
    /// Validates provider permission once, then creates all records atomically.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn add_records(
        env: Env,
        provider: Address,
        records: Vec<BatchRecordInput>,
    ) -> Result<Vec<u64>, ContractError> {
        provider.require_auth();

        if records.is_empty() {
            return Err(ContractError::InvalidInput);
        }

        if !whitelist::check_whitelist_access(&env, &provider) {
            return Err(ContractError::Unauthorized);
        }

        // Check provider has WriteRecord permission once for the whole batch
        if !rbac::has_permission(&env, &provider, &Permission::WriteRecord)
            && !rbac::has_permission(&env, &provider, &Permission::SystemAdmin)
        {
            return Err(ContractError::Unauthorized);
        }

        let counter_key = symbol_short!("REC_CTR");
        let mut current_id: u64 = env.storage().instance().get(&counter_key).unwrap_or(0);
        let mut record_ids = Vec::new(&env);

        // Load current encryption key/version once for the batch
        let current_version: Option<String> = env.storage().instance().get(&ENC_CUR);
        let mut master_bytes_batch: StdVec<u8> = StdVec::new();
        if let Some(ver) = current_version.clone() {
            if let Some(sv) = env
                .storage()
                .persistent()
                .get::<(Symbol, String), String>(&(ENC_KEY, ver.clone()))
            {
                let hex = sv.to_string();
                if let Some(bytes) = common::hex_to_bytes(&hex) {
                    master_bytes_batch = bytes;
                }
            }
        }

        for input in records.iter() {
            current_id += 1;

            // Encrypt input.data_hash with batch master
            let km = KeyManager::new(master_bytes_batch.clone());
            let plaintext: StdString = input.data_hash.to_string();
            let ciphertext = km.encrypt(None, &plaintext);
            let stored_hash = String::from_str(&env, &ciphertext);

            let record = VisionRecord {
                id: current_id,
                patient: input.patient.clone(),
                provider: provider.clone(),
                record_type: input.record_type.clone(),
                data_hash: stored_hash,
                key_version: current_version.clone(),
                created_at: env.ledger().timestamp(),
                updated_at: env.ledger().timestamp(),
            };

            let key = (symbol_short!("RECORD"), current_id);
            env.storage().persistent().set(&key, &record);

            let patient_key = (symbol_short!("PAT_REC"), input.patient.clone());
            let mut patient_records: Vec<u64> = env
                .storage()
                .persistent()
                .get(&patient_key)
                .unwrap_or(Vec::new(&env));
            patient_records.push_back(current_id);
            env.storage()
                .persistent()
                .set(&patient_key, &patient_records);

            events::publish_record_added(
                &env,
                current_id,
                input.patient.clone(),
                provider.clone(),
                input.record_type.clone(),
            );

            record_ids.push_back(current_id);
        }

        env.storage().instance().set(&counter_key, &current_id);

        events::publish_batch_records_added(&env, provider, record_ids.len());

        Ok(record_ids)
    }

    /// Get a vision record by ID.
    pub fn get_record(
        env: Env,
        caller: Address,
        record_id: u64,
    ) -> Result<VisionRecord, ContractError> {
        caller.require_auth();
        let key = (symbol_short!("RECORD"), record_id);
        match env.storage().persistent().get::<_, VisionRecord>(&key) {
            Some(record) => {
                // Check access permissions
                let has_access = if caller == record.patient || caller == record.provider {
                    // Patient can always read their own records
                    // Provider can read records they created
                    true
                } else {
                    // Check if caller has broad read permissions, active consent, or explicit grant
                    rbac::has_permission(&env, &caller, &Permission::ReadAnyRecord)
                        || rbac::has_permission(&env, &caller, &Permission::SystemAdmin)
                        || has_active_consent(&env, &record.patient, &caller)
                        || {
                            let access_level = Self::check_access(
                                env.clone(),
                                record.patient.clone(),
                                caller.clone(),
                            );
                            access_level != AccessLevel::None
                        }
                        || Self::check_record_access(env.clone(), record_id, caller.clone())
                            != AccessLevel::None
                };

                if !has_access {
                    // Log failed access attempt
                    let audit_entry = audit::create_audit_entry(
                        &env,
                        caller.clone(),
                        record.patient.clone(),
                        Some(record_id),
                        AccessAction::Read,
                        AccessResult::Denied,
                        Some(String::from_str(&env, "Insufficient permissions")),
                    );
                    audit::add_audit_entry(&env, &audit_entry);
                    events::publish_audit_log_entry(&env, &audit_entry);

                    return Err(ContractError::Unauthorized);
                }

                // Log successful access
                let audit_entry = audit::create_audit_entry(
                    &env,
                    caller.clone(),
                    record.patient.clone(),
                    Some(record_id),
                    AccessAction::Read,
                    AccessResult::Success,
                    None,
                );
                audit::add_audit_entry(&env, &audit_entry);
                events::publish_audit_log_entry(&env, &audit_entry);

                // Decrypt data_hash for authorized caller before returning
                let mut out_record = record.clone();
                // Prefer record's key_version, fall back to current instance version
                let key_ver = out_record
                    .key_version
                    .clone()
                    .or_else(|| env.storage().instance().get(&ENC_CUR));
                let mut master_bytes: StdVec<u8> = StdVec::new();
                if let Some(ver) = key_ver {
                    if let Some(sv) = env
                        .storage()
                        .persistent()
                        .get::<(Symbol, String), String>(&(ENC_KEY, ver.clone()))
                    {
                        let hex = sv.to_string();
                        if let Some(bytes) = common::hex_to_bytes(&hex) {
                            master_bytes = bytes;
                        }
                    }
                }

                if !master_bytes.is_empty() || out_record.key_version.is_none() {
                    let km = KeyManager::new(master_bytes);
                    let ciphertext_std: StdString = out_record.data_hash.to_string();
                    if let Some(plain) = km.decrypt(None, &ciphertext_std) {
                        out_record.data_hash = String::from_str(&env, &plain);
                    }
                }

                Ok(out_record)
            }
            None => {
                // Log failed access attempt (record not found)
                // We don't know the patient, so we'll use caller as placeholder
                let audit_entry = audit::create_audit_entry(
                    &env,
                    caller.clone(),
                    caller.clone(), // Placeholder since we don't know patient
                    Some(record_id),
                    AccessAction::Read,
                    AccessResult::NotFound,
                    Some(String::from_str(&env, "Record not found")),
                );
                audit::add_audit_entry(&env, &audit_entry);
                events::publish_audit_log_entry(&env, &audit_entry);

                let resource_id = String::from_str(&env, "get_record");
                let context = create_error_context(
                    &env,
                    ContractError::RecordNotFound,
                    None,
                    Some(resource_id.clone()),
                );
                log_error(
                    &env,
                    ContractError::RecordNotFound,
                    None,
                    Some(resource_id),
                    None,
                );
                events::publish_error(&env, ContractError::RecordNotFound as u32, context);
                Err(ContractError::RecordNotFound)
            }
        }
    }

    /// Add eye examination details for an existing record
    #[allow(clippy::too_many_arguments)]
    pub fn add_eye_examination(
        env: Env,
        caller: Address,
        record_id: u64,
        visual_acuity: VisualAcuity,
        iop: IntraocularPressure,
        slit_lamp: SlitLampFindings,
        visual_field: OptVisualField,
        retina_imaging: OptRetinalImaging,
        fundus_photo: OptFundusPhotography,
        clinical_notes: String,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let record = Self::get_record(env.clone(), caller.clone(), record_id)?;

        let has_perm = if caller == record.provider {
            rbac::has_permission(&env, &caller, &Permission::WriteRecord)
        } else {
            rbac::has_delegated_permission(
                &env,
                &record.provider,
                &caller,
                &Permission::WriteRecord,
            )
        };

        if !has_perm && !rbac::has_permission(&env, &caller, &Permission::SystemAdmin) {
            return Err(ContractError::Unauthorized);
        }

        if record.record_type != RecordType::Examination {
            return Err(ContractError::InvalidRecordType);
        }

        let exam = EyeExamination {
            record_id,
            visual_acuity,
            iop,
            slit_lamp,
            visual_field,
            retina_imaging,
            fundus_photo,
            clinical_notes,
        };

        examination::set_examination(&env, &exam);
        events::publish_examination_added(&env, record_id);

        Ok(())
    }

    /// Retrieve eye examination details for a record
    pub fn get_eye_examination(
        env: Env,
        caller: Address,
        record_id: u64,
    ) -> Result<EyeExamination, ContractError> {
        caller.require_auth();
        let record = Self::get_record(env.clone(), caller.clone(), record_id)?;

        let has_perm = if caller == record.patient || caller == record.provider {
            true
        } else {
            let access = Self::check_access(env.clone(), record.patient.clone(), caller.clone());
            let record_access =
                Self::check_record_access(env.clone(), record_id, caller.clone());
            access == AccessLevel::Read
                || access == AccessLevel::Write
                || access == AccessLevel::Full
                || record_access != AccessLevel::None
                || rbac::has_permission(&env, &caller, &Permission::SystemAdmin)
        };

        if !has_perm {
            return Err(ContractError::AccessDenied);
        }

        examination::get_examination(&env, record_id).ok_or(ContractError::RecordNotFound)
    }

    /// Get all records for a patient
    pub fn get_patient_records(env: Env, patient: Address) -> Vec<u64> {
        let key = (symbol_short!("PAT_REC"), patient);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env))
    }

    /// Grant access to a user
    #[allow(clippy::arithmetic_side_effects)]
    pub fn grant_access(
        env: Env,
        caller: Address,
        patient: Address,
        grantee: Address,
        level: AccessLevel,
        duration_seconds: u64,
    ) -> Result<(), ContractError> {
        circuit_breaker::require_not_paused(
            &env,
            &circuit_breaker::PauseScope::Function(symbol_short!("GRT_ACC")),
        )?;
        caller.require_auth();

        Self::enforce_rate_limit(&env, &caller)?;

        validation::validate_duration(duration_seconds)?;

        let has_perm = if caller == patient {
            true // Patient manages own access
        } else {
            // Specific patient→caller delegation for ManageAccess
            rbac::has_delegated_permission(&env, &patient, &caller, &Permission::ManageAccess)
                // Or caller has SystemAdmin (unified: direct + any delegation)
                || rbac::has_permission(&env, &caller, &Permission::SystemAdmin)
        };

        if !has_perm {
            // Log failed access grant attempt
            let audit_entry = audit::create_audit_entry(
                &env,
                caller.clone(),
                patient.clone(),
                None,
                AccessAction::GrantAccess,
                AccessResult::Denied,
                Some(String::from_str(&env, "Insufficient permissions")),
            );
            audit::add_audit_entry(&env, &audit_entry);
            events::publish_audit_log_entry(&env, &audit_entry);
            return Err(ContractError::Unauthorized);
        }

        let expires_at = env.ledger().timestamp() + duration_seconds;
        let grant = AccessGrant {
            patient: patient.clone(),
            grantee: grantee.clone(),
            level: level.clone(),
            granted_at: env.ledger().timestamp(),
            expires_at,
        };

        let key = (symbol_short!("ACCESS"), patient.clone(), grantee.clone());
        env.storage().persistent().set(&key, &grant);
        extend_ttl_access_key(&env, &key);

        // Track the grantee address in the patient's grantee list for purge iteration.
        let list_key = (symbol_short!("ACC_LST"), patient.clone());
        let mut grantees: Vec<Address> = env
            .storage()
            .persistent()
            .get(&list_key)
            .unwrap_or(Vec::new(&env));
        // Avoid duplicates: only append if not already present.
        let mut found = false;
        for i in 0..grantees.len() {
            if grantees.get(i) == Some(grantee.clone()) {
                found = true;
                break;
            }
        }
        if !found {
            grantees.push_back(grantee.clone());
            env.storage().persistent().set(&list_key, &grantees);
        }

        events::publish_access_granted(&env, patient, grantee, level, duration_seconds, expires_at);

        Ok(())
    }

    /// Grant access to multiple users in a single transaction.
    /// Patient authorizes once for the entire batch.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn grant_access_batch(
        env: Env,
        patient: Address,
        grants: Vec<BatchGrantInput>,
    ) -> Result<(), ContractError> {
        patient.require_auth();

        if grants.is_empty() {
            return Err(ContractError::InvalidInput);
        }

        let now = env.ledger().timestamp();
        for grant in grants.iter() {
            let expires_at = now + grant.duration_seconds;
            let access_grant = AccessGrant {
                patient: patient.clone(),
                grantee: grant.grantee.clone(),
                level: grant.level.clone(),
                granted_at: now,
                expires_at,
            };
            let key = (
                symbol_short!("ACCESS"),
                patient.clone(),
                grant.grantee.clone(),
            );
            env.storage().persistent().set(&key, &access_grant);

            events::publish_access_granted(
                &env,
                patient.clone(),
                grant.grantee.clone(),
                grant.level.clone(),
                grant.duration_seconds,
                expires_at,
            );
        }

        events::publish_batch_access_granted(&env, patient, grants.len());

        Ok(())
    }

    /// Check access level with ABAC policy evaluation
    pub fn check_access(env: Env, patient: Address, grantee: Address) -> AccessLevel {
        // First check traditional consent-based access
        if !has_active_consent(&env, &patient, &grantee) {
            return AccessLevel::None;
        }

        let key = (symbol_short!("ACCESS"), patient.clone(), grantee.clone());

        if let Some(grant) = env.storage().persistent().get::<_, AccessGrant>(&key) {
            if grant.expires_at > env.ledger().timestamp() {
                // Check if ABAC policies also allow this access
                let abac_allowed = evaluate_access_policies(&env, &grantee, None, Some(patient.clone()));
                if abac_allowed {
                    return grant.level;
                }
            }
        }

        AccessLevel::None
    }

    /// Grant record-level access to a specific record.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn grant_record_access(
        env: Env,
        patient: Address,
        grantee: Address,
        record_id: u64,
        level: AccessLevel,
        duration_seconds: u64,
    ) -> Result<(), ContractError> {
        patient.require_auth();
        validation::validate_duration(duration_seconds)?;

        let record_key = (symbol_short!("RECORD"), record_id);
        let record: VisionRecord = env
            .storage()
            .persistent()
            .get(&record_key)
            .ok_or(ContractError::RecordNotFound)?;
        if record.patient != patient {
            return Err(ContractError::Unauthorized);
        }

        let now = env.ledger().timestamp();
        let expires_at = now + duration_seconds;
        let grant = AccessGrant {
            patient: patient.clone(),
            grantee: grantee.clone(),
            level: level.clone(),
            granted_at: now,
            expires_at,
        };

        let key = (symbol_short!("REC_ACC"), record_id, grantee.clone());
        env.storage().persistent().set(&key, &grant);
        extend_ttl_record_access_key(&env, &key);

        events::publish_record_access_granted(
            &env,
            patient,
            grantee,
            record_id,
            level,
            duration_seconds,
            expires_at,
        );
        Ok(())
    }

    /// Check record-level access for a specific grantee.
    pub fn check_record_access(env: Env, record_id: u64, grantee: Address) -> AccessLevel {
        let key = (symbol_short!("REC_ACC"), record_id, grantee);
        if let Some(grant) = env.storage().persistent().get::<_, AccessGrant>(&key) {
            if grant.expires_at > env.ledger().timestamp() {
                return grant.level;
            }
        }
        AccessLevel::None
    }

    /// Revoke record-level access for a specific record.
    pub fn revoke_record_access(
        env: Env,
        patient: Address,
        grantee: Address,
        record_id: u64,
    ) -> Result<(), ContractError> {
        patient.require_auth();
        let record_key = (symbol_short!("RECORD"), record_id);
        let record: VisionRecord = env
            .storage()
            .persistent()
            .get(&record_key)
            .ok_or(ContractError::RecordNotFound)?;
        if record.patient != patient {
            return Err(ContractError::Unauthorized);
        }

        let key = (symbol_short!("REC_ACC"), record_id, grantee);
        env.storage().persistent().remove(&key);
        Ok(())
    }

    /// Grant consent for a grantee.
    pub fn grant_consent(
        env: Env,
        patient: Address,
        grantee: Address,
        consent_type: ConsentType,
        duration_seconds: u64,
    ) -> Result<(), ContractError> {
        patient.require_auth();
        if duration_seconds == 0 {
            return Err(ContractError::InvalidInput);
        }
        let now = env.ledger().timestamp();
        let consent = ConsentGrant {
            patient: patient.clone(),
            grantee: grantee.clone(),
            consent_type: consent_type.clone(),
            granted_at: now,
            expires_at: now.saturating_add(duration_seconds),
            revoked: false,
        };
        let key = consent_key(&patient, &grantee);
        env.storage().persistent().set(&key, &consent);
        extend_ttl_access_key(&env, &key);
        events::publish_consent_granted(&env, patient, grantee, consent_type, consent.expires_at);
        Ok(())
    }

    /// Revoke previously granted consent.
    pub fn revoke_consent(
        env: Env,
        patient: Address,
        grantee: Address,
    ) -> Result<(), ContractError> {
        patient.require_auth();
        let key = consent_key(&patient, &grantee);
        if let Some(mut consent) = env.storage().persistent().get::<_, ConsentGrant>(&key) {
            consent.revoked = true;
            env.storage().persistent().set(&key, &consent);
        }
        events::publish_consent_revoked(&env, patient, grantee);
        Ok(())
    }

    /// Revoke access
    pub fn revoke_access(
        env: Env,
        patient: Address,
        grantee: Address,
    ) -> Result<(), ContractError> {
        patient.require_auth();

        let key = (symbol_short!("ACCESS"), patient.clone(), grantee.clone());
        env.storage().persistent().remove(&key);

        // Log successful access revoke
        let audit_entry = audit::create_audit_entry(
            &env,
            patient.clone(),
            patient.clone(),
            None,
            AccessAction::RevokeAccess,
            AccessResult::Success,
            None,
        );
        audit::add_audit_entry(&env, &audit_entry);
        events::publish_audit_log_entry(&env, &audit_entry);

        events::publish_access_revoked(&env, patient, grantee);

        Ok(())
    }

    /// Purge all expired access grants for a given patient.
    ///
    /// Only the patient themselves or a SystemAdmin may call this.
    /// Returns the number of grants removed.
    pub fn purge_expired_grants(
        env: Env,
        caller: Address,
        patient: Address,
    ) -> Result<u32, ContractError> {
        caller.require_auth();

        let is_patient = caller == patient;
        let is_admin = rbac::has_permission(&env, &caller, &Permission::SystemAdmin);
        if !is_patient && !is_admin {
            return Err(ContractError::Unauthorized);
        }

        let list_key = (symbol_short!("ACC_LST"), patient.clone());
        let grantees: Vec<Address> = env
            .storage()
            .persistent()
            .get(&list_key)
            .unwrap_or(Vec::new(&env));

        let now = env.ledger().timestamp();
        let mut remaining = Vec::new(&env);
        let mut purged: u32 = 0;

        for i in 0..grantees.len() {
            if let Some(grantee) = grantees.get(i) {
                let access_key = (symbol_short!("ACCESS"), patient.clone(), grantee.clone());

                match env
                    .storage()
                    .persistent()
                    .get::<_, AccessGrant>(&access_key)
                {
                    Some(grant) if grant.expires_at <= now => {
                        env.storage().persistent().remove(&access_key);
                        events::publish_access_expired(
                            &env,
                            patient.clone(),
                            grantee,
                            grant.expires_at,
                        );
                        purged += 1;
                    }
                    Some(_) => {
                        // Grant still active — keep in the list.
                        remaining.push_back(grantee);
                    }
                    None => {
                        // Already removed — nothing to purge, drop from list.
                    }
                }
            }
        }

        // Update the grantee list to only keep active entries.
        if remaining.is_empty() {
            env.storage().persistent().remove(&list_key);
        } else {
            env.storage().persistent().set(&list_key, &remaining);
        }

        Ok(purged)
    }

    /// Get the total number of records
    pub fn get_record_count(env: Env) -> u64 {
        let counter_key = symbol_short!("REC_CTR");
        env.storage().instance().get(&counter_key).unwrap_or(0)
    }

    /// Get multiple records by IDs.
    pub fn get_records(env: Env, record_ids: Vec<u64>) -> Result<Vec<VisionRecord>, ContractError> {
        if record_ids.is_empty() {
            return Err(ContractError::InvalidInput);
        }
        let mut records = Vec::new(&env);
        for record_id in record_ids.iter() {
            let key = (symbol_short!("RECORD"), record_id);
            let record = env
                .storage()
                .persistent()
                .get::<_, VisionRecord>(&key)
                .ok_or(ContractError::RecordNotFound)?;
            records.push_back(record);
        }
        Ok(records)
    }

    /// Add a new prescription
    #[allow(clippy::too_many_arguments)]
    pub fn add_prescription(
        env: Env,
        patient: Address,
        provider: Address,
        lens_type: LensType,
        left_eye: PrescriptionData,
        right_eye: PrescriptionData,
        contact_data: OptionalContactLensData,
        duration_seconds: u64,
        metadata_hash: String,
    ) -> Result<u64, ContractError> {
        provider.require_auth();

        // Check if provider is authorized (role check)
        let provider_data = VisionRecordsContract::get_user(env.clone(), provider.clone())?;
        if provider_data.role != Role::Optometrist && provider_data.role != Role::Ophthalmologist {
            return Err(ContractError::Unauthorized);
        }

        // Generate ID
        let counter_key = symbol_short!("RX_CTR");
        let rx_id: u64 = env.storage().instance().get(&counter_key).unwrap_or(0) + 1;
        env.storage().instance().set(&counter_key, &rx_id);

        let rx = Prescription {
            id: rx_id,
            patient,
            provider,
            lens_type,
            left_eye,
            right_eye,
            contact_data,
            issued_at: env.ledger().timestamp(),
            expires_at: env.ledger().timestamp() + duration_seconds,
            verified: false,
            metadata_hash,
        };

        prescription::save_prescription(&env, &rx);

        Ok(rx_id)
    }

    /// Get a prescription by ID
    pub fn get_prescription(env: Env, rx_id: u64) -> Result<Prescription, ContractError> {
        prescription::get_prescription(&env, rx_id).ok_or(ContractError::RecordNotFound)
    }

    /// Get all prescription IDs for a patient
    pub fn get_prescription_history(env: Env, patient: Address) -> Vec<u64> {
        prescription::get_patient_history(&env, patient)
    }

    /// Verify a prescription (e.g., by a pharmacy or another provider)
    pub fn verify_prescription(
        env: Env,
        rx_id: u64,
        verifier: Address,
    ) -> Result<bool, ContractError> {
        // Ensure verifier exists
        VisionRecordsContract::get_user(env.clone(), verifier.clone())?;

        Ok(prescription::verify_prescription(&env, rx_id, verifier))
    }

    /// Contract version
    pub fn version() -> u32 {
        2 // Updated for patient profile management
    }

    // ======================== Patient Profile Management ========================

    /// Create a new patient profile
    pub fn create_profile(
        env: Env,
        caller: Address,
        patient: Address,
        date_of_birth_hash: String,
        gender_hash: String,
        blood_type_hash: String,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Only patient or authorized user can create profile
        if caller != patient && !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Err(ContractError::Unauthorized);
        }

        // Check if profile already exists
        let profile_key = (symbol_short!("PAT_PROF"), patient.clone());
        if env.storage().persistent().has(&profile_key) {
            return Err(ContractError::InvalidInput); // Profile already exists
        }

        let profile = PatientProfile {
            patient: patient.clone(),
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
            is_active: true,
            date_of_birth_hash,
            gender_hash,
            blood_type_hash,
            emergency_contact: OptionalEmergencyContact::None,
            insurance_info: OptionalInsuranceInfo::None,
            medical_history_refs: Vec::new(&env),
        };

        env.storage().persistent().set(&profile_key, &profile);
        events::publish_profile_created(&env, patient);

        Ok(())
    }

    /// Update patient demographics
    pub fn update_demographics(
        env: Env,
        caller: Address,
        patient: Address,
        date_of_birth_hash: String,
        gender_hash: String,
        blood_type_hash: String,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Only profile owner can update
        if caller != patient {
            return Err(ContractError::Unauthorized);
        }

        let profile_key = (symbol_short!("PAT_PROF"), patient.clone());
        let mut profile: PatientProfile = env
            .storage()
            .persistent()
            .get(&profile_key)
            .ok_or(ContractError::UserNotFound)?;

        profile.date_of_birth_hash = date_of_birth_hash;
        profile.gender_hash = gender_hash;
        profile.blood_type_hash = blood_type_hash;
        profile.updated_at = env.ledger().timestamp();

        env.storage().persistent().set(&profile_key, &profile);
        events::publish_profile_updated(&env, patient);

        Ok(())
    }

    /// Update emergency contact information
    pub fn update_emergency_contact(
        env: Env,
        caller: Address,
        patient: Address,
        contact: Option<EmergencyContact>,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Only profile owner can update
        if caller != patient {
            return Err(ContractError::Unauthorized);
        }

        let profile_key = (symbol_short!("PAT_PROF"), patient.clone());
        let mut profile: PatientProfile = env
            .storage()
            .persistent()
            .get(&profile_key)
            .ok_or(ContractError::UserNotFound)?;

        profile.emergency_contact = match contact {
            Some(c) => OptionalEmergencyContact::Some(c),
            None => OptionalEmergencyContact::None,
        };
        profile.updated_at = env.ledger().timestamp();

        env.storage().persistent().set(&profile_key, &profile);
        events::publish_profile_updated(&env, patient);

        Ok(())
    }

    /// Update insurance information (hashed values only)
    pub fn update_insurance(
        env: Env,
        caller: Address,
        patient: Address,
        insurance_info: Option<InsuranceInfo>,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Only profile owner can update
        if caller != patient {
            return Err(ContractError::Unauthorized);
        }

        let profile_key = (symbol_short!("PAT_PROF"), patient.clone());
        let mut profile: PatientProfile = env
            .storage()
            .persistent()
            .get(&profile_key)
            .ok_or(ContractError::UserNotFound)?;

        profile.insurance_info = match insurance_info {
            Some(info) => OptionalInsuranceInfo::Some(info),
            None => OptionalInsuranceInfo::None,
        };
        profile.updated_at = env.ledger().timestamp();

        env.storage().persistent().set(&profile_key, &profile);
        events::publish_profile_updated(&env, patient);

        Ok(())
    }

    /// Add medical history reference (IPFS hash or record ID)
    pub fn add_medical_history_reference(
        env: Env,
        caller: Address,
        patient: Address,
        reference: String,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Only profile owner can update
        if caller != patient {
            return Err(ContractError::Unauthorized);
        }

        let profile_key = (symbol_short!("PAT_PROF"), patient.clone());
        let mut profile: PatientProfile = env
            .storage()
            .persistent()
            .get(&profile_key)
            .ok_or(ContractError::UserNotFound)?;

        profile.medical_history_refs.push_back(reference);
        profile.updated_at = env.ledger().timestamp();

        env.storage().persistent().set(&profile_key, &profile);
        events::publish_profile_updated(&env, patient);

        Ok(())
    }

    /// Get patient profile
    pub fn get_profile(env: Env, patient: Address) -> Result<PatientProfile, ContractError> {
        let profile_key = (symbol_short!("PAT_PROF"), patient);
        env.storage()
            .persistent()
            .get(&profile_key)
            .ok_or(ContractError::UserNotFound)
    }

    /// Check if patient profile exists
    pub fn profile_exists(env: Env, patient: Address) -> bool {
        let profile_key = (symbol_short!("PAT_PROF"), patient);
        env.storage().persistent().has(&profile_key)
    }

    /// Grants a custom permission to a user.
    /// Requires the caller to have ManageUsers permission.
    pub fn grant_custom_permission(
        env: Env,
        caller: Address,
        user: Address,
        permission: Permission,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        // Unified check: covers direct role, custom grants, and delegated roles
        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Err(ContractError::Unauthorized);
        }
        rbac::grant_custom_permission(&env, user, permission)
            .map_err(|_| ContractError::UserNotFound)?;
        Ok(())
    }

    /// Revokes a custom permission from a user.
    /// Requires the caller to have ManageUsers permission.
    pub fn revoke_custom_permission(
        env: Env,
        caller: Address,
        user: Address,
        permission: Permission,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        // Unified check: covers direct role, custom grants, and delegated roles
        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Err(ContractError::Unauthorized);
        }
        rbac::revoke_custom_permission(&env, user, permission)
            .map_err(|_| ContractError::UserNotFound)?;
        Ok(())
    }

    /// Delegates a role to another user with an expiration timestamp.
    /// The delegator must authenticate the transaction.
    pub fn delegate_role(
        env: Env,
        delegator: Address,
        delegatee: Address,
        role: Role,
        expires_at: u64,
    ) -> Result<(), ContractError> {
        delegator.require_auth();
        rbac::delegate_role(&env, delegator, delegatee, role, expires_at);
        Ok(())
    }

    /// Pauses contract operations for a given scope.
    pub fn pause_contract(
        env: Env,
        caller: Address,
        scope: circuit_breaker::PauseScope,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        circuit_breaker::pause_contract(&env, &caller, scope)
    }

    /// Resumes contract operations for a given scope.
    pub fn resume_contract(
        env: Env,
        caller: Address,
        scope: circuit_breaker::PauseScope,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        circuit_breaker::resume_contract(&env, &caller, scope)
    }

    /// Creates an ACL group.
    pub fn create_acl_group(
        env: Env,
        caller: Address,
        group_name: String,
        permissions: Vec<Permission>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Err(ContractError::Unauthorized);
        }
        rbac::create_group(&env, group_name, permissions);
        Ok(())
    }

    /// Adds a user to an ACL group.
    pub fn add_user_to_group(
        env: Env,
        caller: Address,
        user: Address,
        group_name: String,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Err(ContractError::Unauthorized);
        }
        rbac::add_to_group(&env, user, group_name).map_err(|_| ContractError::InvalidInput)
    }

    /// Removes a user from an ACL group.
    pub fn remove_user_from_group(
        env: Env,
        caller: Address,
        user: Address,
        group_name: String,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Err(ContractError::Unauthorized);
        }
        rbac::remove_from_group(&env, user, group_name);
        Ok(())
    }

    /// Returns all ACL groups assigned to a user.
    pub fn get_user_groups(env: Env, user: Address) -> Vec<String> {
        env.storage()
            .persistent()
            .get(&rbac::user_groups_key(&user))
            .unwrap_or(Vec::new(&env))
    }

    /// Checks if a user has a specific permission.
    /// Returns true if the user has the permission, false otherwise.
    pub fn check_permission(env: Env, user: Address, permission: Permission) -> bool {
        rbac::has_permission(&env, &user, &permission)
    }

    // ======================== Admin Tier Management ========================

    /// Promotes or assigns a target address to the specified admin tier.
    ///
    /// Only a `SuperAdmin` may call this.
    pub fn promote_admin(
        env: Env,
        caller: Address,
        target: Address,
        tier: AdminTier,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if !admin_tiers::promote_admin(&env, &caller, &target, tier) {
            return Err(ContractError::Unauthorized);
        }
        admin_tiers::track_admin(&env, &target);
        Ok(())
    }

    /// Removes the admin tier from the target address entirely.
    ///
    /// Only a `SuperAdmin` may call this.
    pub fn demote_admin(
        env: Env,
        caller: Address,
        target: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if !admin_tiers::demote_admin(&env, &caller, &target) {
            return Err(ContractError::Unauthorized);
        }
        admin_tiers::untrack_admin(&env, &target);
        Ok(())
    }

    /// Returns the admin tier of the given address, if any.
    pub fn get_admin_tier(env: Env, admin: Address) -> Option<AdminTier> {
        admin_tiers::get_admin_tier(&env, &admin)
    }

    // ======================== Internal Helpers ========================

    /// Unified check: returns true if caller has at least the specified admin
    /// tier, OR is the legacy ADMIN address, OR has SystemAdmin RBAC permission.
    fn has_admin_access(env: &Env, caller: &Address, min_tier: &AdminTier) -> bool {
        // 1. Check tiered admin system
        if admin_tiers::require_tier(env, caller, min_tier) {
            return true;
        }
        // 2. Fall back to legacy admin address
        if let Some(admin) = env.storage().instance().get::<Symbol, Address>(&ADMIN) {
            if *caller == admin {
                return true;
            }
        }
        // 3. Fall back to RBAC SystemAdmin
        rbac::has_permission(env, caller, &Permission::SystemAdmin)
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod test_pause;
#[cfg(test)]
mod test_rbac;

#[cfg(test)]
mod test_batch;

#[cfg(test)]
mod test_admin_tiers;
