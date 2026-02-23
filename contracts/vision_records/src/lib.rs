#![no_std]
pub mod rbac;

pub mod events;

pub mod patient_profile;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

use crate::patient_profile::{EmergencyContact, InsuranceInfo, PatientProfile};

/// Storage keys for the contract
const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");

pub use rbac::{Permission, Role};

/// Access levels for record sharing
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessLevel {
    None,
    Read,
    Write,
    Full,
}

/// Vision record types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordType {
    Examination,
    Prescription,
    Diagnosis,
    Treatment,
    Surgery,
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



/// Contract errors
/// Contract errors
#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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
    /// Initialize the contract with an admin address
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(ContractError::AlreadyInitialized);
        }

        // admin.require_auth();

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);

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
            return Err(ContractError::Unauthorized);
        }

        let user_data = User {
            address: user.clone(),
            role: role.clone(),
            name: name.clone(),
            registered_at: env.ledger().timestamp(),
            is_active: true,
        };

        let key = (symbol_short!("USER"), user.clone());
        env.storage().persistent().set(&key, &user_data);

        events::publish_user_registered(&env, user, role, name);

        Ok(())
    }

    /// Get user information
    pub fn get_user(env: Env, user: Address) -> Result<User, ContractError> {
        let key = (symbol_short!("USER"), user);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::UserNotFound)
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

        events::publish_record_added(&env, record_id, patient, provider, record_type);

        Ok(record_id)
    }

    /// Get a vision record by ID
    pub fn get_record(env: Env, record_id: u64) -> Result<VisionRecord, ContractError> {
        let key = (symbol_short!("RECORD"), record_id);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::RecordNotFound)
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
        2  // Updated for patient profile management
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
            emergency_contact: None,
            insurance_info: None,
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
        
        profile.emergency_contact = contact;
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
        
        profile.insurance_info = insurance_info;
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

    // ======================== RBAC Endpoints ========================

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

    pub fn check_permission(env: Env, user: Address, permission: Permission) -> bool {
        rbac::has_permission(&env, &user, &permission)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events};
    use soroban_sdk::{Env, IntoVal, TryIntoVal};




}

#[cfg(test)]
mod test_rbac;

