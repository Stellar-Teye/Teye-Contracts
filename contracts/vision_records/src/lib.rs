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

    // ======================== ACL Group Endpoints ========================

    pub fn create_acl_group(
        env: Env,
        caller: Address,
        name: String,
        permissions: Vec<Permission>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Err(ContractError::Unauthorized);
        }
        rbac::create_group(&env, name, permissions);
        Ok(())
    }

    pub fn delete_acl_group(env: Env, caller: Address, name: String) -> Result<(), ContractError> {
        caller.require_auth();
        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Err(ContractError::Unauthorized);
        }
        rbac::delete_group(&env, name);
        Ok(())
    }

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
        rbac::add_to_group(&env, user, group_name).map_err(|_| ContractError::InvalidInput)?;
        Ok(())
    }

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

    pub fn get_user_groups(env: Env, user: Address) -> Vec<String> {
        env.storage()
            .persistent()
            .get(&rbac::user_groups_key(&user))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_acl_group_permissions(env: Env, group_name: String) -> Vec<Permission> {
        rbac::get_group_permissions(&env, &group_name)
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod test_rbac;
