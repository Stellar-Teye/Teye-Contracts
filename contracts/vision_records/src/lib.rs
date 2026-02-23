#![no_std]
pub mod rbac;
pub mod validation;

pub mod errors;
pub mod events;
pub mod provider;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

pub use errors::{
    create_error_context, log_error, ContractError, ErrorCategory, ErrorLogEntry, ErrorSeverity,
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

        events::publish_record_added(&env, record_id, patient, provider, record_type);

        Ok(record_id)
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

    /// Registers a new healthcare provider in the system.
    /// Requires the caller to have ManageUsers permission.
    /// Returns the provider ID assigned to the new provider.
    #[allow(clippy::too_many_arguments)]
    pub fn register_provider(
        env: Env,
        caller: Address,
        provider: Address,
        name: String,
        licenses: Vec<License>,
        specialties: Vec<String>,
        certifications: Vec<Certification>,
        locations: Vec<Location>,
    ) -> Result<u64, ContractError> {
        caller.require_auth();

        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Err(ContractError::Unauthorized);
        }

        if provider::get_provider(&env, &provider).is_some() {
            let resource_id = String::from_str(&env, "register_provider");
            let context = create_error_context(
                &env,
                ContractError::ProviderAlreadyRegistered,
                Some(caller.clone()),
                Some(resource_id.clone()),
            );
            log_error(
                &env,
                ContractError::ProviderAlreadyRegistered,
                Some(caller),
                Some(resource_id),
                None,
            );
            events::publish_error(
                &env,
                ContractError::ProviderAlreadyRegistered as u32,
                context,
            );
            return Err(ContractError::ProviderAlreadyRegistered);
        }

        let provider_id = provider::increment_provider_counter(&env);
        provider::add_provider_id(&env, provider_id, &provider);

        let provider_data = Provider {
            address: provider.clone(),
            name: name.clone(),
            licenses: licenses.clone(),
            specialties: specialties.clone(),
            certifications: certifications.clone(),
            locations: locations.clone(),
            verification_status: VerificationStatus::Pending,
            registered_at: env.ledger().timestamp(),
            verified_at: None,
            verified_by: None,
            is_active: true,
        };

        provider::set_provider(&env, &provider_data);

        for specialty in specialties.iter() {
            provider::add_provider_to_specialty_index(&env, &specialty, &provider);
        }

        events::publish_provider_registered(&env, provider.clone(), name, provider_id);

        Ok(provider_id)
    }

    /// Verifies or updates the verification status of a provider.
    /// Requires the caller to have ManageUsers permission.
    /// Cannot set status to Pending.
    pub fn verify_provider(
        env: Env,
        caller: Address,
        provider: Address,
        status: VerificationStatus,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Err(ContractError::Unauthorized);
        }

        let mut provider_data =
            provider::get_provider(&env, &provider).ok_or(ContractError::ProviderNotFound)?;

        if status == VerificationStatus::Pending {
            return Err(ContractError::InvalidVerificationStatus);
        }

        provider_data.verification_status = status.clone();
        provider_data.verified_at = Some(env.ledger().timestamp());
        provider_data.verified_by = Some(caller.clone());

        // Status index is updated automatically in set_provider
        provider::set_provider(&env, &provider_data);

        events::publish_provider_verified(&env, provider, caller, status);

        Ok(())
    }

    /// Updates provider information including name, licenses, specialties, certifications, and locations.
    /// The provider can update their own information, or users with ManageUsers permission can update any provider.
    #[allow(clippy::too_many_arguments)]
    pub fn update_provider(
        env: Env,
        caller: Address,
        provider: Address,
        name: Option<String>,
        licenses: Option<Vec<License>>,
        specialties: Option<Vec<String>>,
        certifications: Option<Vec<Certification>>,
        locations: Option<Vec<Location>>,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        if caller != provider && !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Err(ContractError::Unauthorized);
        }

        let mut provider_data =
            provider::get_provider(&env, &provider).ok_or(ContractError::ProviderNotFound)?;

        if let Some(new_name) = name {
            provider_data.name = new_name;
        }

        if let Some(new_licenses) = licenses {
            provider_data.licenses = new_licenses;
        }

        if let Some(new_specialties) = specialties {
            for old_specialty in provider_data.specialties.iter() {
                provider::remove_provider_from_specialty_index(&env, &old_specialty, &provider);
            }
            provider_data.specialties = new_specialties.clone();
            for specialty in new_specialties.iter() {
                provider::add_provider_to_specialty_index(&env, &specialty, &provider);
            }
        }

        if let Some(new_certifications) = certifications {
            provider_data.certifications = new_certifications;
        }

        if let Some(new_locations) = locations {
            provider_data.locations = new_locations;
        }

        provider::set_provider(&env, &provider_data);

        events::publish_provider_updated(&env, provider);

        Ok(())
    }

    /// Retrieves provider information by address.
    /// Returns the provider data if found, or an error if the provider is not registered.
    pub fn get_provider(env: Env, provider: Address) -> Result<Provider, ContractError> {
        if let Some(provider_data) = provider::get_provider(&env, &provider) {
            Ok(provider_data)
        } else {
            let resource_id = String::from_str(&env, "get_provider");
            let context = create_error_context(
                &env,
                ContractError::ProviderNotFound,
                Some(provider.clone()),
                Some(resource_id.clone()),
            );
            log_error(
                &env,
                ContractError::ProviderNotFound,
                Some(provider),
                Some(resource_id),
                None,
            );
            events::publish_error(&env, ContractError::ProviderNotFound as u32, context);
            Err(ContractError::ProviderNotFound)
        }
    }

    /// Searches for providers by specialty.
    /// Returns a vector of provider addresses matching the specified specialty.
    pub fn search_providers_by_specialty(env: Env, specialty: String) -> Vec<Address> {
        provider::get_providers_by_specialty(&env, &specialty)
    }

    /// Searches for providers by verification status.
    /// Returns a vector of active provider addresses with the specified verification status.
    /// Uses an efficient status index to avoid exceeding Soroban's 100-key limit.
    pub fn search_providers_by_status(env: Env, status: VerificationStatus) -> Vec<Address> {
        provider::get_providers_by_status(&env, &status)
    }

    /// Returns the total number of registered providers in the system.
    pub fn get_provider_count(env: Env) -> u64 {
        provider::get_provider_counter(&env)
    }

    /// Retrieves a provider address by provider ID.
    /// Returns None if the provider ID does not exist.
    #[allow(dead_code)]
    fn get_provider_address_by_id(env: &Env, provider_id: u64) -> Option<Address> {
        let id_key = (symbol_short!("PROV_ID"), provider_id);
        env.storage().persistent().get(&id_key)
    }

    /// Retrieves the complete error log containing all logged errors.
    /// The log is limited to the most recent 100 entries.
    pub fn get_error_log(env: Env) -> Vec<ErrorLogEntry> {
        errors::get_error_log(&env)
    }

    /// Returns the total count of errors that have been logged since contract initialization.
    pub fn get_error_count(env: Env) -> u64 {
        errors::get_error_count(&env)
    }

    /// Clears the error log and resets the error count.
    /// Requires the caller to have SystemAdmin permission.
    pub fn clear_error_log(env: Env, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        if !rbac::has_permission(&env, &caller, &Permission::SystemAdmin) {
            let resource_id = String::from_str(&env, "clear_error_log");
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
        errors::clear_error_log(&env);
        Ok(())
    }

    /// Checks if an operation can be retried based on the current retry count.
    /// Returns true if the operation can be retried, false if max retries have been reached.
    /// Max retries must be between 1 and 10.
    pub fn retry_operation(
        env: Env,
        caller: Address,
        operation: String,
        max_retries: u32,
    ) -> Result<bool, ContractError> {
        if max_retries == 0 || max_retries > 10 {
            let resource_id = String::from_str(&env, "retry_operation");
            let context = create_error_context(
                &env,
                ContractError::InvalidInput,
                Some(caller.clone()),
                Some(resource_id.clone()),
            );
            log_error(
                &env,
                ContractError::InvalidInput,
                Some(caller),
                Some(resource_id),
                None,
            );
            events::publish_error(&env, ContractError::InvalidInput as u32, context);
            return Err(ContractError::InvalidInput);
        }
        Ok(errors::retry_operation(
            &env,
            &caller,
            &operation,
            max_retries,
        ))
    }

    /// Resets the retry count for a specific operation and caller.
    /// This allows the operation to be retried from the beginning.
    pub fn reset_retry_count(env: Env, caller: Address, operation: String) {
        errors::reset_retry_count(&env, &caller, &operation);
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod test_rbac;
