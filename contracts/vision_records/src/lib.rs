#![no_std]
mod events;
pub mod rbac;
pub mod validation;

pub mod errors;
pub mod events;
pub mod examination;
pub mod provider;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
    Symbol, Vec,
};

pub use errors::{
    create_error_context, log_error, ContractError, ErrorCategory, ErrorLogEntry, ErrorSeverity,
};
pub use examination::{
    EyeExamination, FundusPhotography, IntraocularPressure, OptFundusPhotography,
    OptPhysicalMeasurement, OptRetinalImaging, OptVisualField, PhysicalMeasurement, RetinalImaging,
    SlitLampFindings, VisualAcuity, VisualField,
};
pub use provider::{Certification, License, Location, Provider, VerificationStatus};

/// Storage keys for the contract
const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");
const RATE_CFG: Symbol = symbol_short!("RL_CFG");
const RATE_TRACK: Symbol = symbol_short!("RL_TRK");

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

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

pub use rbac::{Permission, Role};

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

/// Input for batch record creation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchRecordInput {
    pub patient: Address,
    pub record_type: RecordType,
    pub data_hash: String,
}

/// Input for batch access grants
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchGrantInput {
    pub grantee: Address,
    pub level: AccessLevel,
    pub duration_seconds: u64,
}

/// Contract errors
#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ContractError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    UserNotFound = 4,
    RecordNotFound = 5,
    InvalidInput = 6,
    AccessDenied = 7,
    Paused = 8,
}

#[contract]
pub struct VisionRecordsContract;

#[contractimpl]
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

        events::publish_initialized(&env, admin);

        Ok(())
    }

    /// Get the admin address
    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)
    }

    /// Check if the contract is initialized
    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&INITIALIZED)
    }

    /// Configure per-address rate limiting for this contract.
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

        let admin = Self::get_admin(env.clone())?;
        let has_system_admin = rbac::has_permission(&env, &caller, &Permission::SystemAdmin);

        if caller != admin && !has_system_admin {
            return Err(ContractError::Unauthorized);
        }

        env.storage().instance().set(
            &RATE_CFG,
            &(max_requests_per_window, window_duration_seconds),
        );

        Ok(())
    }

    /// Return the current rate limiting configuration, if any.
    pub fn get_rate_limit_config(env: Env) -> Option<(u64, u64)> {
        env.storage().instance().get(&RATE_CFG)
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
        if let Some(user_data) = env.storage().persistent().get(&key) {
            Ok(user_data)
        } else {
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

        Self::enforce_rate_limit(&env, &caller)?;

        validation::validate_data_hash(&data_hash)?;

        let has_perm = if caller == provider {
            rbac::has_permission(&env, &caller, &Permission::WriteRecord)
        } else {
            rbac::has_delegated_permission(&env, &provider, &caller, &Permission::WriteRecord)
        };

        if !has_perm && !rbac::has_permission(&env, &caller, &Permission::SystemAdmin) {
            return Err(ContractError::Unauthorized);
        }

        // Generate record ID
        let counter_key = symbol_short!("REC_CTR");
        let record_id: u64 = env.storage().instance().get(&counter_key).unwrap_or(0u64).saturating_add(1u64);
        env.storage().instance().set(&counter_key, &record_id);

        let record = VisionRecord {
            id: record_id,
            patient: patient.clone(),
            provider: provider.clone(),
            record_type: record_type.clone(),
            data_hash,
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
        extend_ttl_address_key(&env, &patient_key);

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

        // Check provider has WriteRecord permission once for the whole batch
        if !rbac::has_permission(&env, &provider, &Permission::WriteRecord)
            && !rbac::has_permission(&env, &provider, &Permission::SystemAdmin)
        {
            return Err(ContractError::Unauthorized);
        }

        let counter_key = symbol_short!("REC_CTR");
        let mut current_id: u64 = env.storage().instance().get(&counter_key).unwrap_or(0);
        let mut record_ids = Vec::new(&env);

        for input in records.iter() {
            current_id += 1;

            let record = VisionRecord {
                id: current_id,
                patient: input.patient.clone(),
                provider: provider.clone(),
                record_type: input.record_type.clone(),
                data_hash: input.data_hash.clone(),
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

    /// Get a vision record by ID
    pub fn get_record(env: Env, record_id: u64) -> Result<VisionRecord, ContractError> {
        let key = (symbol_short!("RECORD"), record_id);
        if let Some(record) = env.storage().persistent().get(&key) {
            Ok(record)
        } else {
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

        let record = Self::get_record(env.clone(), record_id)?;

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
        let record = Self::get_record(env.clone(), record_id)?;

        let has_perm = if caller == record.patient || caller == record.provider {
            true
        } else {
            let access = Self::check_access(env.clone(), record.patient.clone(), caller.clone());
            access == AccessLevel::Read
                || access == AccessLevel::Write
                || access == AccessLevel::Full
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
            rbac::has_delegated_permission(&env, &patient, &caller, &Permission::ManageAccess)
                || rbac::has_permission(&env, &caller, &Permission::SystemAdmin)
        };

        if !has_perm {
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

    /// Check access level
    pub fn check_access(env: Env, patient: Address, grantee: Address) -> AccessLevel {
        let key = (symbol_short!("ACCESS"), patient, grantee);

        if let Some(grant) = env.storage().persistent().get::<_, AccessGrant>(&key) {
            if grant.expires_at > env.ledger().timestamp() {
                return grant.level;
            }
        }

        AccessLevel::None
    }

    /// Revoke access
    pub fn revoke_access(
        env: Env,
        patient: Address,
        grantee: Address,
    ) -> Result<(), ContractError> {
        circuit_breaker::require_not_paused(
            &env,
            &circuit_breaker::PauseScope::Function(symbol_short!("REV_ACC")),
        )?;
        patient.require_auth();

        let key = (symbol_short!("ACCESS"), patient.clone(), grantee.clone());
        env.storage().persistent().remove(&key);

        events::publish_access_revoked(&env, patient, grantee);

        Ok(())
    }

    /// Get the total number of records
    pub fn get_record_count(env: Env) -> u64 {
        let counter_key = symbol_short!("REC_CTR");
        env.storage().instance().get(&counter_key).unwrap_or(0)
    }

    /// Contract version
    pub fn version() -> u32 {
        1
    }

    // ======================== RBAC Endpoints ========================

    /// Grants a custom permission to a user.
    /// Requires the caller to have ManageUsers permission.
    pub fn grant_custom_permission(
        env: Env,
        caller: Address,
        user: Address,
        permission: Permission,
    ) -> Result<(), ContractError> {
        caller.require_auth();
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

    /// Checks if a user has a specific permission.
    /// Returns true if the user has the permission, false otherwise.
    pub fn check_permission(env: Env, user: Address, permission: Permission) -> bool {
        rbac::has_permission(&env, &user, &permission)
    }

#[cfg(test)]
mod test;

#[cfg(test)]
mod test;

#[cfg(test)]
mod test_pause;
#[cfg(test)]
mod test_rbac;

#[cfg(test)]
mod test_batch;
