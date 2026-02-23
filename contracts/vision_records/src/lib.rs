#![no_std]
pub mod rbac;
pub mod validation;

pub mod appointment;
pub mod audit;
pub mod emergency;
pub mod errors;
pub mod events;
pub mod provider;
pub mod rate_limit;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

pub use appointment::{Appointment, AppointmentHistoryEntry, AppointmentStatus, AppointmentType};
pub use audit::{AccessAction, AccessResult, AuditEntry};
pub use emergency::{EmergencyAccess, EmergencyAuditEntry, EmergencyCondition, EmergencyStatus};
pub use errors::{
    create_error_context, log_error, ContractError, ErrorCategory, ErrorLogEntry, ErrorSeverity,
};
pub use provider::{Certification, License, Location, Provider, VerificationStatus};
pub use rate_limit::{RateLimitConfig, RateLimitStats, RateLimitStatus};

/// Storage keys for the contract
const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");

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

    /// Register a new user
    pub fn register_user(
        env: Env,
        caller: Address,
        user: Address,
        role: Role,
        name: String,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Check rate limit
        let operation = String::from_str(&env, "register_user");
        let (allowed, current_count, max_requests, reset_at) =
            rate_limit::check_rate_limit(&env, &caller, &operation);
        if !allowed {
            events::publish_rate_limit_exceeded(
                &env,
                caller.clone(),
                operation,
                current_count,
                max_requests,
                reset_at,
            );
            let context = create_error_context(
                &env,
                ContractError::RateLimitExceeded,
                Some(caller.clone()),
                Some(String::from_str(&env, "register_user")),
            );
            log_error(
                &env,
                ContractError::RateLimitExceeded,
                Some(caller),
                None,
                None,
            );
            events::publish_error(&env, ContractError::RateLimitExceeded as u32, context);
            return Err(ContractError::RateLimitExceeded);
        }

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
        caller.require_auth();

        // Check rate limit
        let operation = String::from_str(&env, "add_record");
        let (allowed, current_count, max_requests, reset_at) =
            rate_limit::check_rate_limit(&env, &caller, &operation);
        if !allowed {
            events::publish_rate_limit_exceeded(
                &env,
                caller.clone(),
                operation,
                current_count,
                max_requests,
                reset_at,
            );
            let context = create_error_context(
                &env,
                ContractError::RateLimitExceeded,
                Some(caller.clone()),
                Some(String::from_str(&env, "add_record")),
            );
            log_error(
                &env,
                ContractError::RateLimitExceeded,
                Some(caller),
                None,
                None,
            );
            events::publish_error(&env, ContractError::RateLimitExceeded as u32, context);
            return Err(ContractError::RateLimitExceeded);
        }

        validation::validate_data_hash(&data_hash)?;

        let has_perm = if caller == provider {
            rbac::has_permission(&env, &caller, &Permission::WriteRecord)
        } else {
            rbac::has_delegated_permission(&env, &provider, &caller, &Permission::WriteRecord)
        };

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

        // Log successful write
        let audit_entry = audit::create_audit_entry(
            &env,
            caller.clone(),
            patient.clone(),
            Some(record_id),
            AccessAction::Write,
            AccessResult::Success,
            None,
        );
        audit::add_audit_entry(&env, &audit_entry);
        events::publish_audit_log_entry(&env, &audit_entry);

        events::publish_record_added(&env, record_id, patient, provider, record_type);

        Ok(record_id)
    }

    /// Get a vision record by ID
    /// Requires authentication and logs access attempt
    pub fn get_record(
        env: Env,
        caller: Address,
        record_id: u64,
    ) -> Result<VisionRecord, ContractError> {
        caller.require_auth();

        // Check rate limit
        let operation = String::from_str(&env, "get_record");
        let (allowed, current_count, max_requests, reset_at) =
            rate_limit::check_rate_limit(&env, &caller, &operation);
        if !allowed {
            events::publish_rate_limit_exceeded(
                &env,
                caller.clone(),
                operation,
                current_count,
                max_requests,
                reset_at,
            );
            let context = create_error_context(
                &env,
                ContractError::RateLimitExceeded,
                Some(caller.clone()),
                Some(String::from_str(&env, "get_record")),
            );
            log_error(
                &env,
                ContractError::RateLimitExceeded,
                Some(caller),
                None,
                None,
            );
            events::publish_error(&env, ContractError::RateLimitExceeded as u32, context);
            return Err(ContractError::RateLimitExceeded);
        }

        let key = (symbol_short!("RECORD"), record_id);
        match env.storage().persistent().get::<_, VisionRecord>(&key) {
            Some(record) => {
                // Check access permissions
                let has_access = if caller == record.patient || caller == record.provider {
                    // Patient can always read their own records
                    // Provider can read records they created
                    true
                } else {
                    // Check if caller has ReadAnyRecord permission or has been granted access
                    rbac::has_permission(&env, &caller, &Permission::ReadAnyRecord) || {
                        let access_level =
                            Self::check_access(env.clone(), record.patient.clone(), caller.clone());
                        access_level != AccessLevel::None
                    }
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

                Ok(record)
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

        // Check rate limit
        let operation = String::from_str(&env, "grant_access");
        let (allowed, current_count, max_requests, reset_at) =
            rate_limit::check_rate_limit(&env, &caller, &operation);
        if !allowed {
            events::publish_rate_limit_exceeded(
                &env,
                caller.clone(),
                operation,
                current_count,
                max_requests,
                reset_at,
            );
            let context = create_error_context(
                &env,
                ContractError::RateLimitExceeded,
                Some(caller.clone()),
                Some(String::from_str(&env, "grant_access")),
            );
            log_error(
                &env,
                ContractError::RateLimitExceeded,
                Some(caller),
                None,
                None,
            );
            events::publish_error(&env, ContractError::RateLimitExceeded as u32, context);
            return Err(ContractError::RateLimitExceeded);
        }

        validation::validate_duration(duration_seconds)?;

        let has_perm = if caller == patient {
            true // Patient manages own access
        } else {
            rbac::has_delegated_permission(&env, &patient, &caller, &Permission::ManageAccess)
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

        // Log successful access grant
        let audit_entry = audit::create_audit_entry(
            &env,
            caller.clone(),
            patient.clone(),
            None,
            AccessAction::GrantAccess,
            AccessResult::Success,
            None,
        );
        audit::add_audit_entry(&env, &audit_entry);
        events::publish_audit_log_entry(&env, &audit_entry);

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
        caller: Address,
        patient: Address,
        grantee: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Verify caller has permission to revoke
        if caller != patient && !rbac::has_permission(&env, &caller, &Permission::SystemAdmin) {
            // Log failed revoke attempt
            let audit_entry = audit::create_audit_entry(
                &env,
                caller.clone(),
                patient.clone(),
                None,
                AccessAction::RevokeAccess,
                AccessResult::Denied,
                Some(String::from_str(&env, "Insufficient permissions")),
            );
            audit::add_audit_entry(&env, &audit_entry);
            events::publish_audit_log_entry(&env, &audit_entry);
            return Err(ContractError::Unauthorized);
        }

        let key = (symbol_short!("ACCESS"), patient.clone(), grantee.clone());
        env.storage().persistent().remove(&key);

        // Log successful access revoke
        let audit_entry = audit::create_audit_entry(
            &env,
            caller.clone(),
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

        let mut provider_data = match provider::get_provider(&env, &provider) {
            Some(data) => data,
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::ProviderNotFound,
                    Some(caller.clone()),
                    Some(String::from_str(&env, "verify_provider")),
                );
                log_error(
                    &env,
                    ContractError::ProviderNotFound,
                    Some(caller),
                    None,
                    None,
                );
                events::publish_error(&env, ContractError::ProviderNotFound as u32, context);
                return Err(ContractError::ProviderNotFound);
            }
        };

        if status == VerificationStatus::Pending {
            let context = create_error_context(
                &env,
                ContractError::InvalidVerificationStatus,
                Some(caller.clone()),
                Some(String::from_str(&env, "verify_provider")),
            );
            log_error(
                &env,
                ContractError::InvalidVerificationStatus,
                Some(caller),
                None,
                None,
            );
            events::publish_error(
                &env,
                ContractError::InvalidVerificationStatus as u32,
                context,
            );
            return Err(ContractError::InvalidVerificationStatus);
        }

        provider_data.verification_status = status.clone();
        provider_data.verified_at = Some(env.ledger().timestamp());
        provider_data.verified_by = Some(caller.clone());

        // Status index is updated automatically in set_provider
        provider::set_provider(&env, &provider_data);

        // Grant rate limit bypass for verified providers
        if status == VerificationStatus::Verified {
            rate_limit::set_rate_limit_bypass(&env, &provider, true);
            events::publish_rate_limit_bypass_updated(&env, provider.clone(), true, caller.clone());
        } else {
            // Remove bypass if status changes from Verified to something else
            rate_limit::set_rate_limit_bypass(&env, &provider, false);
            events::publish_rate_limit_bypass_updated(
                &env,
                provider.clone(),
                false,
                caller.clone(),
            );
        }

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
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "update_provider")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        let mut provider_data = match provider::get_provider(&env, &provider) {
            Some(data) => data,
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::ProviderNotFound,
                    Some(caller.clone()),
                    Some(String::from_str(&env, "update_provider")),
                );
                log_error(
                    &env,
                    ContractError::ProviderNotFound,
                    Some(caller),
                    None,
                    None,
                );
                events::publish_error(&env, ContractError::ProviderNotFound as u32, context);
                return Err(ContractError::ProviderNotFound);
            }
        };

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
        match provider::get_provider(&env, &provider) {
            Some(provider_data) => Ok(provider_data),
            None => {
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
        caller.require_auth();

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

        let retry_key = (symbol_short!("RETRY"), caller.clone(), operation.clone());
        let retry_count: u32 = env.storage().persistent().get(&retry_key).unwrap_or(0);

        if retry_count >= max_retries {
            env.storage().persistent().remove(&retry_key);
            Ok(false)
        } else {
            env.storage()
                .persistent()
                .set(&retry_key, &(retry_count + 1));
            Ok(true)
        }
    }

    /// Resets the retry count for a specific operation and caller.
    /// This allows the operation to be retried from the beginning.
    pub fn reset_retry_count(
        env: Env,
        caller: Address,
        operation: String,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let retry_key = (symbol_short!("RETRY"), caller, operation);
        env.storage().persistent().remove(&retry_key);
        Ok(())
    }

    // ======================== Emergency Access Endpoints ========================

    /// Grants emergency access to a patient's records for critical care situations.
    /// Requires the requester to be a verified provider with appropriate permissions.
    /// Emergency access is always time-limited and requires attestation.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn grant_emergency_access(
        env: Env,
        caller: Address,
        patient: Address,
        condition: EmergencyCondition,
        attestation: String,
        duration_seconds: u64,
        emergency_contacts: Vec<Address>,
    ) -> Result<u64, ContractError> {
        caller.require_auth();

        // Validate attestation is not empty
        if attestation.is_empty() {
            let context = create_error_context(
                &env,
                ContractError::InvalidAttestation,
                Some(caller.clone()),
                Some(String::from_str(&env, "grant_emergency_access")),
            );
            log_error(
                &env,
                ContractError::InvalidAttestation,
                Some(caller),
                None,
                None,
            );
            events::publish_error(&env, ContractError::InvalidAttestation as u32, context);
            return Err(ContractError::InvalidAttestation);
        }

        // Validate duration (max 24 hours for emergency access)
        const MAX_EMERGENCY_DURATION: u64 = 86400; // 24 hours in seconds
        if duration_seconds == 0 || duration_seconds > MAX_EMERGENCY_DURATION {
            let context = create_error_context(
                &env,
                ContractError::InvalidInput,
                Some(caller.clone()),
                Some(String::from_str(&env, "grant_emergency_access")),
            );
            log_error(&env, ContractError::InvalidInput, Some(caller), None, None);
            events::publish_error(&env, ContractError::InvalidInput as u32, context);
            return Err(ContractError::InvalidInput);
        }

        // Requester must be a verified provider or have SystemAdmin permission
        let is_provider = provider::get_provider(&env, &caller)
            .map(|p| p.verification_status == VerificationStatus::Verified)
            .unwrap_or(false);

        if !is_provider && !rbac::has_permission(&env, &caller, &Permission::SystemAdmin) {
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "grant_emergency_access")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        let access_id = emergency::increment_emergency_counter(&env);
        let granted_at = env.ledger().timestamp();
        let expires_at = granted_at + duration_seconds;

        let emergency_access = EmergencyAccess {
            id: access_id,
            patient: patient.clone(),
            requester: caller.clone(),
            condition: condition.clone(),
            attestation: attestation.clone(),
            granted_at,
            expires_at,
            status: EmergencyStatus::Active,
            notified_contacts: emergency_contacts.clone(),
        };

        emergency::set_emergency_access(&env, &emergency_access);

        // Create audit entry
        let audit_entry = EmergencyAuditEntry {
            access_id,
            actor: caller.clone(),
            action: String::from_str(&env, "GRANTED"),
            timestamp: granted_at,
        };
        emergency::add_audit_entry(&env, &audit_entry);

        // Publish events
        events::publish_emergency_access_granted(
            &env,
            access_id,
            patient.clone(),
            caller.clone(),
            condition,
            expires_at,
        );

        // Notify emergency contacts
        for contact in emergency_contacts.iter() {
            events::publish_emergency_contact_notified(
                &env,
                access_id,
                patient.clone(),
                contact.clone(),
            );
        }

        Ok(access_id)
    }

    /// Revokes an active emergency access grant.
    /// Can be called by the patient, the original requester, or a SystemAdmin.
    pub fn revoke_emergency_access(
        env: Env,
        caller: Address,
        access_id: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let emergency_access = match emergency::get_emergency_access(&env, access_id) {
            Some(access) => access,
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::EmergencyAccessNotFound,
                    Some(caller.clone()),
                    Some(String::from_str(&env, "revoke_emergency_access")),
                );
                log_error(
                    &env,
                    ContractError::EmergencyAccessNotFound,
                    Some(caller),
                    None,
                    None,
                );
                events::publish_error(&env, ContractError::EmergencyAccessNotFound as u32, context);
                return Err(ContractError::EmergencyAccessNotFound);
            }
        };

        // Check authorization: patient, original requester, or SystemAdmin
        let is_authorized = caller == emergency_access.patient
            || caller == emergency_access.requester
            || rbac::has_permission(&env, &caller, &Permission::SystemAdmin);

        if !is_authorized {
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "revoke_emergency_access")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        if emergency_access.status != EmergencyStatus::Active {
            let error = if emergency_access.status == EmergencyStatus::Expired {
                ContractError::EmergencyAccessExpired
            } else {
                ContractError::EmergencyAccessRevoked
            };
            let context = create_error_context(
                &env,
                error,
                Some(caller.clone()),
                Some(String::from_str(&env, "revoke_emergency_access")),
            );
            log_error(&env, error, Some(caller), None, None);
            events::publish_error(&env, error as u32, context);
            return Err(error);
        }

        emergency::revoke_emergency_access(&env, access_id);

        // Create audit entry
        let audit_entry = EmergencyAuditEntry {
            access_id,
            actor: caller.clone(),
            action: String::from_str(&env, "REVOKED"),
            timestamp: env.ledger().timestamp(),
        };
        emergency::add_audit_entry(&env, &audit_entry);

        // Publish event
        events::publish_emergency_access_revoked(&env, access_id, emergency_access.patient, caller);

        Ok(())
    }

    /// Checks if emergency access is currently active for a patient-requester pair.
    /// Returns the emergency access if active, None otherwise.
    pub fn check_emergency_access(
        env: Env,
        patient: Address,
        requester: Address,
    ) -> Option<EmergencyAccess> {
        emergency::has_active_emergency_access(&env, &patient, &requester)
    }

    /// Uses emergency access to read a patient's record.
    /// Creates an audit trail and publishes an event.
    pub fn access_record_via_emergency(
        env: Env,
        caller: Address,
        patient: Address,
        record_id: Option<u64>,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let emergency_access = match emergency::has_active_emergency_access(&env, &patient, &caller)
        {
            Some(access) => access,
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::EmergencyAccessDenied,
                    Some(caller.clone()),
                    Some(String::from_str(&env, "access_record_via_emergency")),
                );
                log_error(
                    &env,
                    ContractError::EmergencyAccessDenied,
                    Some(caller),
                    None,
                    None,
                );
                events::publish_error(&env, ContractError::EmergencyAccessDenied as u32, context);
                return Err(ContractError::EmergencyAccessDenied);
            }
        };

        // Verify access hasn't expired
        if emergency_access.expires_at <= env.ledger().timestamp() {
            // Log failed emergency access attempt
            let audit_entry = audit::create_audit_entry(
                &env,
                caller.clone(),
                patient.clone(),
                record_id,
                AccessAction::EmergencyAccess,
                AccessResult::Expired,
                Some(String::from_str(&env, "Emergency access expired")),
            );
            audit::add_audit_entry(&env, &audit_entry);
            events::publish_audit_log_entry(&env, &audit_entry);

            let context = create_error_context(
                &env,
                ContractError::EmergencyAccessExpired,
                Some(caller.clone()),
                Some(String::from_str(&env, "access_record_via_emergency")),
            );
            log_error(
                &env,
                ContractError::EmergencyAccessExpired,
                Some(caller),
                None,
                None,
            );
            events::publish_error(&env, ContractError::EmergencyAccessExpired as u32, context);
            return Err(ContractError::EmergencyAccessExpired);
        }

        // Create emergency audit entry
        let emergency_audit_entry = EmergencyAuditEntry {
            access_id: emergency_access.id,
            actor: caller.clone(),
            action: String::from_str(&env, "ACCESSED"),
            timestamp: env.ledger().timestamp(),
        };
        emergency::add_audit_entry(&env, &emergency_audit_entry);

        // Create general audit entry
        let audit_entry = audit::create_audit_entry(
            &env,
            caller.clone(),
            patient.clone(),
            record_id,
            AccessAction::EmergencyAccess,
            AccessResult::Success,
            None,
        );
        audit::add_audit_entry(&env, &audit_entry);
        events::publish_audit_log_entry(&env, &audit_entry);

        // Publish event
        events::publish_emergency_access_used(
            &env,
            emergency_access.id,
            patient,
            caller,
            record_id,
        );

        Ok(())
    }

    /// Retrieves emergency access information by ID.
    pub fn get_emergency_access(
        env: Env,
        access_id: u64,
    ) -> Result<EmergencyAccess, ContractError> {
        match emergency::get_emergency_access(&env, access_id) {
            Some(access) => Ok(access),
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::EmergencyAccessNotFound,
                    None,
                    Some(String::from_str(&env, "get_emergency_access")),
                );
                log_error(
                    &env,
                    ContractError::EmergencyAccessNotFound,
                    None,
                    None,
                    None,
                );
                events::publish_error(&env, ContractError::EmergencyAccessNotFound as u32, context);
                Err(ContractError::EmergencyAccessNotFound)
            }
        }
    }

    /// Retrieves all active emergency accesses for a patient.
    pub fn get_patient_emergency_accesses(env: Env, patient: Address) -> Vec<EmergencyAccess> {
        emergency::get_patient_emergency_accesses(&env, &patient)
    }

    /// Retrieves audit trail for an emergency access ID.
    pub fn get_emergency_audit_trail(
        env: Env,
        access_id: u64,
    ) -> Result<Vec<EmergencyAuditEntry>, ContractError> {
        match emergency::get_emergency_access(&env, access_id) {
            Some(_) => Ok(emergency::get_audit_entries(&env, access_id)),
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::EmergencyAccessNotFound,
                    None,
                    Some(String::from_str(&env, "get_emergency_audit_trail")),
                );
                log_error(
                    &env,
                    ContractError::EmergencyAccessNotFound,
                    None,
                    None,
                    None,
                );
                events::publish_error(&env, ContractError::EmergencyAccessNotFound as u32, context);
                Err(ContractError::EmergencyAccessNotFound)
            }
        }
    }

    /// Expires emergency accesses that have passed their expiration time.
    /// Returns the number of accesses expired.
    pub fn expire_emergency_accesses(env: Env) -> u32 {
        emergency::expire_emergency_accesses(&env)
    }

    // ======================== Appointment Scheduling Endpoints ========================

    /// Schedules a new appointment between a patient and provider.
    /// Requires the caller to be the patient, provider, or have ManageUsers permission.
    #[allow(clippy::too_many_arguments)]
    pub fn schedule_appointment(
        env: Env,
        caller: Address,
        patient: Address,
        provider: Address,
        appointment_type: AppointmentType,
        scheduled_at: u64,
        duration_minutes: u32,
        notes: Option<String>,
    ) -> Result<u64, ContractError> {
        caller.require_auth();

        // Validate authorization: patient, provider, or admin
        let is_authorized = caller == patient
            || caller == provider
            || rbac::has_permission(&env, &caller, &Permission::ManageUsers);

        if !is_authorized {
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "schedule_appointment")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        // Validate scheduled time is in the future
        let current_time = env.ledger().timestamp();
        if scheduled_at <= current_time {
            let context = create_error_context(
                &env,
                ContractError::InvalidAppointmentTime,
                Some(caller.clone()),
                Some(String::from_str(&env, "schedule_appointment")),
            );
            log_error(
                &env,
                ContractError::InvalidAppointmentTime,
                Some(caller),
                None,
                None,
            );
            events::publish_error(&env, ContractError::InvalidAppointmentTime as u32, context);
            return Err(ContractError::InvalidAppointmentTime);
        }

        // Validate duration (1 minute to 8 hours)
        if duration_minutes == 0 || duration_minutes > 480 {
            let context = create_error_context(
                &env,
                ContractError::InvalidInput,
                Some(caller.clone()),
                Some(String::from_str(&env, "schedule_appointment")),
            );
            log_error(&env, ContractError::InvalidInput, Some(caller), None, None);
            events::publish_error(&env, ContractError::InvalidInput as u32, context);
            return Err(ContractError::InvalidInput);
        }

        let appointment_id = appointment::increment_appointment_counter(&env);
        let created_at = current_time;

        let appointment = Appointment {
            id: appointment_id,
            patient: patient.clone(),
            provider: provider.clone(),
            appointment_type: appointment_type.clone(),
            scheduled_at,
            duration_minutes,
            status: AppointmentStatus::Scheduled,
            notes,
            created_at,
            updated_at: created_at,
            verified_at: None,
            verified_by: None,
            reminder_sent: false,
        };

        appointment::set_appointment(&env, &appointment);

        // Create history entry
        let history_entry = AppointmentHistoryEntry {
            appointment_id,
            action: String::from_str(&env, "CREATED"),
            actor: caller.clone(),
            timestamp: created_at,
            previous_status: AppointmentStatus::None,
            new_status: AppointmentStatus::Scheduled,
            notes: None,
        };
        appointment::add_history_entry(&env, &history_entry);

        // Publish event
        events::publish_appointment_scheduled(
            &env,
            appointment_id,
            patient,
            provider,
            appointment_type,
            scheduled_at,
        );

        Ok(appointment_id)
    }

    /// Confirms an appointment.
    /// Can be called by patient, provider, or admin.
    pub fn confirm_appointment(
        env: Env,
        caller: Address,
        appointment_id: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let mut appointment = match appointment::get_appointment(&env, appointment_id) {
            Some(apt) => apt,
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::AppointmentNotFound,
                    Some(caller.clone()),
                    Some(String::from_str(&env, "confirm_appointment")),
                );
                log_error(
                    &env,
                    ContractError::AppointmentNotFound,
                    Some(caller),
                    None,
                    None,
                );
                events::publish_error(&env, ContractError::AppointmentNotFound as u32, context);
                return Err(ContractError::AppointmentNotFound);
            }
        };

        // Validate authorization
        let is_authorized = caller == appointment.patient
            || caller == appointment.provider
            || rbac::has_permission(&env, &caller, &Permission::ManageUsers);

        if !is_authorized {
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "confirm_appointment")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        // Validate status can be changed to Confirmed
        if appointment.status != AppointmentStatus::Scheduled {
            let context = create_error_context(
                &env,
                ContractError::AppointmentCannotBeModified,
                Some(caller.clone()),
                Some(String::from_str(&env, "confirm_appointment")),
            );
            log_error(
                &env,
                ContractError::AppointmentCannotBeModified,
                Some(caller),
                None,
                None,
            );
            events::publish_error(
                &env,
                ContractError::AppointmentCannotBeModified as u32,
                context,
            );
            return Err(ContractError::AppointmentCannotBeModified);
        }

        let previous_status = appointment.status.clone();
        appointment.status = AppointmentStatus::Confirmed;
        appointment.updated_at = env.ledger().timestamp();
        appointment::set_appointment(&env, &appointment);

        // Create history entry
        let history_entry = AppointmentHistoryEntry {
            appointment_id,
            action: String::from_str(&env, "CONFIRMED"),
            actor: caller.clone(),
            timestamp: env.ledger().timestamp(),
            previous_status,
            new_status: AppointmentStatus::Confirmed,
            notes: None,
        };
        appointment::add_history_entry(&env, &history_entry);

        // Publish event
        events::publish_appointment_confirmed(
            &env,
            appointment_id,
            appointment.patient,
            appointment.provider,
            caller,
        );

        Ok(())
    }

    /// Cancels an appointment.
    /// Can be called by patient, provider, or admin.
    pub fn cancel_appointment(
        env: Env,
        caller: Address,
        appointment_id: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let mut appointment = match appointment::get_appointment(&env, appointment_id) {
            Some(apt) => apt,
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::AppointmentNotFound,
                    Some(caller.clone()),
                    Some(String::from_str(&env, "cancel_appointment")),
                );
                log_error(
                    &env,
                    ContractError::AppointmentNotFound,
                    Some(caller),
                    None,
                    None,
                );
                events::publish_error(&env, ContractError::AppointmentNotFound as u32, context);
                return Err(ContractError::AppointmentNotFound);
            }
        };

        // Validate authorization
        let is_authorized = caller == appointment.patient
            || caller == appointment.provider
            || rbac::has_permission(&env, &caller, &Permission::ManageUsers);

        if !is_authorized {
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "cancel_appointment")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        // Validate status can be cancelled
        if appointment.status == AppointmentStatus::Cancelled
            || appointment.status == AppointmentStatus::Completed
        {
            let context = create_error_context(
                &env,
                ContractError::AppointmentCannotBeModified,
                Some(caller.clone()),
                Some(String::from_str(&env, "cancel_appointment")),
            );
            log_error(
                &env,
                ContractError::AppointmentCannotBeModified,
                Some(caller),
                None,
                None,
            );
            events::publish_error(
                &env,
                ContractError::AppointmentCannotBeModified as u32,
                context,
            );
            return Err(ContractError::AppointmentCannotBeModified);
        }

        let previous_status = appointment.status.clone();
        appointment.status = AppointmentStatus::Cancelled;
        appointment.updated_at = env.ledger().timestamp();
        appointment::set_appointment(&env, &appointment);

        // Create history entry
        let history_entry = AppointmentHistoryEntry {
            appointment_id,
            action: String::from_str(&env, "CANCELLED"),
            actor: caller.clone(),
            timestamp: env.ledger().timestamp(),
            previous_status,
            new_status: AppointmentStatus::Cancelled,
            notes: None,
        };
        appointment::add_history_entry(&env, &history_entry);

        // Publish event
        events::publish_appointment_cancelled(
            &env,
            appointment_id,
            appointment.patient,
            appointment.provider,
            caller,
        );

        Ok(())
    }

    /// Reschedules an appointment to a new time.
    /// Can be called by patient, provider, or admin.
    pub fn reschedule_appointment(
        env: Env,
        caller: Address,
        appointment_id: u64,
        new_scheduled_at: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let mut appointment = match appointment::get_appointment(&env, appointment_id) {
            Some(apt) => apt,
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::AppointmentNotFound,
                    Some(caller.clone()),
                    Some(String::from_str(&env, "reschedule_appointment")),
                );
                log_error(
                    &env,
                    ContractError::AppointmentNotFound,
                    Some(caller),
                    None,
                    None,
                );
                events::publish_error(&env, ContractError::AppointmentNotFound as u32, context);
                return Err(ContractError::AppointmentNotFound);
            }
        };

        // Validate authorization
        let is_authorized = caller == appointment.patient
            || caller == appointment.provider
            || rbac::has_permission(&env, &caller, &Permission::ManageUsers);

        if !is_authorized {
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "reschedule_appointment")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        // Validate new time is in the future
        let current_time = env.ledger().timestamp();
        if new_scheduled_at <= current_time {
            let context = create_error_context(
                &env,
                ContractError::InvalidAppointmentTime,
                Some(caller.clone()),
                Some(String::from_str(&env, "reschedule_appointment")),
            );
            log_error(
                &env,
                ContractError::InvalidAppointmentTime,
                Some(caller),
                None,
                None,
            );
            events::publish_error(&env, ContractError::InvalidAppointmentTime as u32, context);
            return Err(ContractError::InvalidAppointmentTime);
        }

        // Validate status can be rescheduled
        if appointment.status == AppointmentStatus::Cancelled
            || appointment.status == AppointmentStatus::Completed
        {
            let context = create_error_context(
                &env,
                ContractError::AppointmentCannotBeModified,
                Some(caller.clone()),
                Some(String::from_str(&env, "reschedule_appointment")),
            );
            log_error(
                &env,
                ContractError::AppointmentCannotBeModified,
                Some(caller),
                None,
                None,
            );
            events::publish_error(
                &env,
                ContractError::AppointmentCannotBeModified as u32,
                context,
            );
            return Err(ContractError::AppointmentCannotBeModified);
        }

        let old_scheduled_at = appointment.scheduled_at;
        appointment.scheduled_at = new_scheduled_at;
        appointment.status = AppointmentStatus::Rescheduled;
        appointment.updated_at = env.ledger().timestamp();
        appointment.reminder_sent = false; // Reset reminder flag
        appointment::set_appointment(&env, &appointment);

        // Create history entry
        let history_entry = AppointmentHistoryEntry {
            appointment_id,
            action: String::from_str(&env, "RESCHEDULED"),
            actor: caller.clone(),
            timestamp: env.ledger().timestamp(),
            previous_status: AppointmentStatus::None,
            new_status: AppointmentStatus::Rescheduled,
            notes: None,
        };
        appointment::add_history_entry(&env, &history_entry);

        // Publish event
        events::publish_appointment_rescheduled(
            &env,
            appointment_id,
            appointment.patient,
            appointment.provider,
            old_scheduled_at,
            new_scheduled_at,
            caller,
        );

        Ok(())
    }

    /// Marks an appointment as completed.
    /// Can be called by provider or admin.
    pub fn complete_appointment(
        env: Env,
        caller: Address,
        appointment_id: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let mut appointment = match appointment::get_appointment(&env, appointment_id) {
            Some(apt) => apt,
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::AppointmentNotFound,
                    Some(caller.clone()),
                    Some(String::from_str(&env, "complete_appointment")),
                );
                log_error(
                    &env,
                    ContractError::AppointmentNotFound,
                    Some(caller),
                    None,
                    None,
                );
                events::publish_error(&env, ContractError::AppointmentNotFound as u32, context);
                return Err(ContractError::AppointmentNotFound);
            }
        };

        // Validate authorization: provider or admin
        let is_authorized = caller == appointment.provider
            || rbac::has_permission(&env, &caller, &Permission::ManageUsers);

        if !is_authorized {
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "complete_appointment")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        // Validate status can be completed
        if appointment.status == AppointmentStatus::Cancelled
            || appointment.status == AppointmentStatus::Completed
        {
            let context = create_error_context(
                &env,
                ContractError::AppointmentCannotBeModified,
                Some(caller.clone()),
                Some(String::from_str(&env, "complete_appointment")),
            );
            log_error(
                &env,
                ContractError::AppointmentCannotBeModified,
                Some(caller),
                None,
                None,
            );
            events::publish_error(
                &env,
                ContractError::AppointmentCannotBeModified as u32,
                context,
            );
            return Err(ContractError::AppointmentCannotBeModified);
        }

        let previous_status = appointment.status.clone();
        appointment.status = AppointmentStatus::Completed;
        appointment.updated_at = env.ledger().timestamp();
        appointment::set_appointment(&env, &appointment);

        // Create history entry
        let history_entry = AppointmentHistoryEntry {
            appointment_id,
            action: String::from_str(&env, "COMPLETED"),
            actor: caller.clone(),
            timestamp: env.ledger().timestamp(),
            previous_status,
            new_status: AppointmentStatus::Completed,
            notes: None,
        };
        appointment::add_history_entry(&env, &history_entry);

        // Publish event
        events::publish_appointment_completed(
            &env,
            appointment_id,
            appointment.patient,
            appointment.provider,
            caller,
        );

        Ok(())
    }

    /// Verifies an appointment.
    /// Requires ManageUsers permission.
    pub fn verify_appointment(
        env: Env,
        caller: Address,
        appointment_id: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "verify_appointment")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        let mut appointment = match appointment::get_appointment(&env, appointment_id) {
            Some(apt) => apt,
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::AppointmentNotFound,
                    Some(caller.clone()),
                    Some(String::from_str(&env, "verify_appointment")),
                );
                log_error(
                    &env,
                    ContractError::AppointmentNotFound,
                    Some(caller),
                    None,
                    None,
                );
                events::publish_error(&env, ContractError::AppointmentNotFound as u32, context);
                return Err(ContractError::AppointmentNotFound);
            }
        };

        appointment.verified_at = Some(env.ledger().timestamp());
        appointment.verified_by = Some(caller.clone());
        appointment.updated_at = env.ledger().timestamp();
        appointment::set_appointment(&env, &appointment);

        // Create history entry
        let history_entry = AppointmentHistoryEntry {
            appointment_id,
            action: String::from_str(&env, "VERIFIED"),
            actor: caller.clone(),
            timestamp: env.ledger().timestamp(),
            previous_status: AppointmentStatus::None,
            new_status: AppointmentStatus::None,
            notes: None,
        };
        appointment::add_history_entry(&env, &history_entry);

        // Publish event
        events::publish_appointment_verified(
            &env,
            appointment_id,
            appointment.patient,
            appointment.provider,
            caller,
        );

        Ok(())
    }

    /// Sends reminders for appointments that need them.
    /// Returns the number of reminders sent.
    pub fn send_appointment_reminders(
        env: Env,
        reminder_window_seconds: u64,
    ) -> Result<u32, ContractError> {
        let appointments =
            appointment::get_appointments_needing_reminders(&env, reminder_window_seconds);

        let mut reminder_count = 0u32;

        for i in 0..appointments.len() {
            if let Some(apt) = appointments.get(i) {
                // Mark reminder as sent
                if appointment::mark_reminder_sent(&env, apt.id).is_some() {
                    let patient = apt.patient.clone();
                    let provider = apt.provider.clone();

                    // Publish reminder event
                    events::publish_appointment_reminder(
                        &env,
                        apt.id,
                        patient.clone(),
                        provider,
                        apt.scheduled_at,
                    );

                    // Create history entry
                    let history_entry = AppointmentHistoryEntry {
                        appointment_id: apt.id,
                        action: String::from_str(&env, "REMINDER_SENT"),
                        actor: patient, // System action
                        timestamp: env.ledger().timestamp(),
                        previous_status: AppointmentStatus::None,
                        new_status: AppointmentStatus::None,
                        notes: None,
                    };
                    appointment::add_history_entry(&env, &history_entry);

                    reminder_count += 1;
                }
            }
        }

        Ok(reminder_count)
    }

    /// Retrieves an appointment by ID.
    pub fn get_appointment(env: Env, appointment_id: u64) -> Result<Appointment, ContractError> {
        match appointment::get_appointment(&env, appointment_id) {
            Some(apt) => Ok(apt),
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::AppointmentNotFound,
                    None,
                    Some(String::from_str(&env, "get_appointment")),
                );
                log_error(&env, ContractError::AppointmentNotFound, None, None, None);
                events::publish_error(&env, ContractError::AppointmentNotFound as u32, context);
                Err(ContractError::AppointmentNotFound)
            }
        }
    }

    /// Retrieves all appointments for a patient.
    pub fn get_patient_appointments(env: Env, patient: Address) -> Vec<Appointment> {
        appointment::get_patient_appointments(&env, &patient)
    }

    /// Retrieves all appointments for a provider.
    pub fn get_provider_appointments(env: Env, provider: Address) -> Vec<Appointment> {
        appointment::get_provider_appointments(&env, &provider)
    }

    /// Retrieves upcoming appointments for a patient.
    pub fn get_patient_upcoming(env: Env, patient: Address) -> Vec<Appointment> {
        appointment::get_upcoming_patient_appointments(&env, &patient)
    }

    /// Retrieves appointment history for an appointment ID.
    pub fn get_appointment_history(
        env: Env,
        appointment_id: u64,
    ) -> Result<Vec<AppointmentHistoryEntry>, ContractError> {
        match appointment::get_appointment(&env, appointment_id) {
            Some(_) => Ok(appointment::get_appointment_history(&env, appointment_id)),
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::AppointmentNotFound,
                    None,
                    Some(String::from_str(&env, "get_appointment_history")),
                );
                log_error(&env, ContractError::AppointmentNotFound, None, None, None);
                events::publish_error(&env, ContractError::AppointmentNotFound as u32, context);
                Err(ContractError::AppointmentNotFound)
            }
        }
    }

    // ======================== Audit Logging Endpoints ========================

    /// Retrieves an audit entry by ID.
    pub fn get_audit_entry(env: Env, entry_id: u64) -> Result<AuditEntry, ContractError> {
        match audit::get_audit_entry(&env, entry_id) {
            Some(entry) => Ok(entry),
            None => {
                let context = create_error_context(
                    &env,
                    ContractError::RecordNotFound, // Reuse RecordNotFound for audit entries
                    None,
                    Some(String::from_str(&env, "get_audit_entry")),
                );
                log_error(&env, ContractError::RecordNotFound, None, None, None);
                events::publish_error(&env, ContractError::RecordNotFound as u32, context);
                Err(ContractError::RecordNotFound)
            }
        }
    }

    /// Retrieves all audit entries for a specific record.
    pub fn get_record_audit_log(env: Env, record_id: u64) -> Vec<AuditEntry> {
        audit::get_record_audit_log(&env, record_id)
    }

    /// Retrieves all audit entries for a specific user (actor).
    pub fn get_user_audit_log(env: Env, user: Address) -> Vec<AuditEntry> {
        audit::get_user_audit_log(&env, &user)
    }

    /// Retrieves all audit entries for a specific patient.
    pub fn get_patient_audit_log(env: Env, patient: Address) -> Vec<AuditEntry> {
        audit::get_patient_audit_log(&env, &patient)
    }

    /// Retrieves audit entries filtered by action type.
    pub fn get_audit_log_by_action(env: Env, action: AccessAction) -> Vec<AuditEntry> {
        audit::get_audit_log_by_action(&env, action)
    }

    /// Retrieves audit entries filtered by result.
    pub fn get_audit_log_by_result(env: Env, result: AccessResult) -> Vec<AuditEntry> {
        audit::get_audit_log_by_result(&env, result)
    }

    /// Retrieves audit entries within a time range.
    pub fn get_audit_log_by_time_range(
        env: Env,
        start_time: u64,
        end_time: u64,
    ) -> Vec<AuditEntry> {
        audit::get_audit_log_by_time_range(&env, start_time, end_time)
    }

    /// Retrieves recent audit entries (last N entries).
    pub fn get_recent_audit_log(env: Env, limit: u64) -> Vec<AuditEntry> {
        audit::get_recent_audit_log(&env, limit)
    }

    // ── Rate Limiting Functions ────────────────────────────────────────────────

    /// Sets rate limit configuration for an operation
    pub fn set_rate_limit_config(
        env: Env,
        caller: Address,
        operation: String,
        max_requests: u32,
        window_seconds: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        if !rbac::has_permission(&env, &caller, &Permission::SystemAdmin) {
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "set_rate_limit_config")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        let config = RateLimitConfig {
            max_requests,
            window_seconds,
            operation: operation.clone(),
        };
        rate_limit::set_rate_limit_config(&env, &config);
        events::publish_rate_limit_config_updated(
            &env,
            operation,
            max_requests,
            window_seconds,
            caller,
        );

        Ok(())
    }

    /// Gets rate limit configuration for an operation
    pub fn get_rate_limit_config(env: Env, operation: String) -> Option<RateLimitConfig> {
        rate_limit::get_rate_limit_config(&env, &operation)
    }

    /// Gets rate limit status for an address and operation
    pub fn get_rate_limit_status(
        env: Env,
        address: Address,
        operation: String,
    ) -> Option<RateLimitStatus> {
        rate_limit::get_rate_limit_status(&env, &address, &operation)
    }

    /// Sets rate limit bypass for an address (admin only)
    pub fn set_rate_limit_bypass(
        env: Env,
        caller: Address,
        address: Address,
        bypass: bool,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        if !rbac::has_permission(&env, &caller, &Permission::SystemAdmin) {
            let context = create_error_context(
                &env,
                ContractError::Unauthorized,
                Some(caller.clone()),
                Some(String::from_str(&env, "set_rate_limit_bypass")),
            );
            log_error(&env, ContractError::Unauthorized, Some(caller), None, None);
            events::publish_error(&env, ContractError::Unauthorized as u32, context);
            return Err(ContractError::Unauthorized);
        }

        rate_limit::set_rate_limit_bypass(&env, &address, bypass);
        events::publish_rate_limit_bypass_updated(&env, address, bypass, caller);

        Ok(())
    }

    /// Checks if an address has rate limit bypass
    pub fn has_rate_limit_bypass(env: Env, address: Address) -> bool {
        rate_limit::has_rate_limit_bypass(&env, &address)
    }

    /// Gets all rate limit configurations
    pub fn get_all_rate_limit_configs(env: Env) -> Vec<RateLimitConfig> {
        rate_limit::get_all_rate_limit_configs(&env)
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod test_rbac;
