#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
    Symbol, Vec,
};

mod prescription;
mod prescription_tests;
pub mod rbac;

pub mod events;

pub use crate::prescription::{
    ContactLensData, LensType, OptionalContactLensData, Prescription, PrescriptionData,
};

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

/// Emergency access structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct EmergencyAccess {
    pub patient: Address,
    pub status: EmergencyStatus,
    pub expires_at: u64,
}

/// Audit entry for emergency access logs
#[contracttype]
#[derive(Clone, Debug)]
pub struct EmergencyAuditEntry {
    pub access_id: u64,
    pub actor: Address,
    pub action: String,
    pub timestamp: u64,
}

/// Contract errors
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
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

    /// Add a new prescription
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
        1
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

    // ==================== Emergency Access Endpoints ====================

    /// Retrieve an emergency access grant.
    pub fn get_emergency_access(
        env: Env,
        access_id: u64,
    ) -> Result<EmergencyAccess, ContractError> {
        let key = (symbol_short!("EMRG"), access_id);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::RecordNotFound)
    }

    /// Check whether an emergency grant is currently valid.
    pub fn is_emergency_access_valid(env: Env, access_id: u64) -> bool {
        let key = (symbol_short!("EMRG"), access_id);
        if let Some(grant) = env.storage().persistent().get::<_, EmergencyAccess>(&key) {
            return grant.status == EmergencyStatus::Active
                && grant.expires_at > env.ledger().timestamp();
        }
        false
    }

    /// Revoke an active emergency grant. Only the original patient or admin may do this.
    pub fn revoke_emergency_access(
        env: Env,
        caller: Address,
        access_id: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)?;

        let key = (symbol_short!("EMRG"), access_id);
        let mut grant: EmergencyAccess = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::RecordNotFound)?;

        if caller != grant.patient && caller != admin {
            return Err(ContractError::Unauthorized);
        }

        grant.status = EmergencyStatus::Revoked;
        env.storage().persistent().set(&key, &grant);

        Self::write_emergency_audit(
            &env,
            access_id,
            caller,
            String::from_str(&env, "REVOKED"),
            env.ledger().timestamp(),
        );

        Ok(())
    }

    /// Record that a requester actually accessed a record under emergency authority.
    pub fn log_emergency_record_access(
        env: Env,
        requester: Address,
        access_id: u64,
    ) -> Result<(), ContractError> {
        requester.require_auth();

        if !Self::is_emergency_access_valid(env.clone(), access_id) {
            return Err(ContractError::AccessDenied);
        }

        Self::write_emergency_audit(
            &env,
            access_id,
            requester,
            String::from_str(&env, "ACCESSED"),
            env.ledger().timestamp(),
        );

        Ok(())
    }

    fn write_emergency_audit(
        env: &Env,
        access_id: u64,
        actor: Address,
        action: String,
        timestamp: u64,
    ) {
        let audit_key = (symbol_short!("EMRG_LOG"), access_id);
        let mut log: Vec<EmergencyAuditEntry> = env
            .storage()
            .persistent()
            .get(&audit_key)
            .unwrap_or(Vec::new(env));

        log.push_back(EmergencyAuditEntry {
            access_id,
            actor,
            action,
            timestamp,
        });

        env.storage().persistent().set(&audit_key, &log);
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events};
    use soroban_sdk::{Env, IntoVal, TryIntoVal};

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);
        let events = env.events().all();

        assert!(client.is_initialized());
        assert_eq!(client.get_admin(), admin);
        let our_events: soroban_sdk::Vec<(
            soroban_sdk::Address,
            soroban_sdk::Vec<soroban_sdk::Val>,
            soroban_sdk::Val,
        )> = events;

        assert!(!our_events.is_empty());
        let event = our_events.get(our_events.len() - 1).unwrap();
        assert_eq!(event.1, (symbol_short!("INIT"),).into_val(&env));
        let payload: events::InitializedEvent = event.2.try_into_val(&env).unwrap();
        assert_eq!(payload.admin, admin);
    }

    #[test]
    fn test_register_user() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let user = Address::generate(&env);
        let name = String::from_str(&env, "Dr. Smith");

        client.register_user(&user, &Role::Optometrist, &name);
        let events = env.events().all();

        let user_data = client.get_user(&user);
        assert_eq!(user_data.role, Role::Optometrist);
        assert!(user_data.is_active);

        assert!(!events.is_empty());
        let event = events.get(events.len() - 1).unwrap();
        assert_eq!(
            event.1,
            (symbol_short!("USR_REG"), user.clone()).into_val(&env)
        );
        let payload: events::UserRegisteredEvent = event.2.try_into_val(&env).unwrap();
        assert_eq!(payload.user, user);
        assert_eq!(payload.role, Role::Optometrist);
        assert_eq!(payload.name, name);
    }

    #[test]
    fn test_add_record() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        let data_hash = String::from_str(&env, "QmHash123");

        let record_id =
            client.add_record(&patient, &provider, &RecordType::Examination, &data_hash);
        let events = env.events().all();

        assert_eq!(record_id, 1);

        let record = client.get_record(&record_id);
        assert_eq!(record.patient, patient);
        assert_eq!(record.provider, provider);

        assert!(!events.is_empty());
        let event = events.get(events.len() - 1).unwrap();
        assert_eq!(
            event.1,
            (symbol_short!("REC_ADD"), patient.clone(), provider.clone()).into_val(&env)
        );
        let payload: events::RecordAddedEvent = event.2.try_into_val(&env).unwrap();
        assert_eq!(payload.record_id, record_id);
        assert_eq!(payload.patient, patient);
        assert_eq!(payload.provider, provider);
        assert_eq!(payload.record_type, RecordType::Examination);
    }

    #[test]
    fn test_grant_access() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let doctor = Address::generate(&env);

        client.grant_access(&patient, &doctor, &AccessLevel::Read, &86400);
        let events = env.events().all();

        assert!(!events.is_empty());
        let grant_event = events
            .iter()
            .find(|e| {
                let t: &soroban_sdk::Vec<soroban_sdk::Val> = &e.1;
                if !t.is_empty() {
                    let topic0: soroban_sdk::Symbol = t.get(0).unwrap().into_val(&env);
                    return topic0 == symbol_short!("ACC_GRT");
                }
                false
            })
            .expect("ACC_GRT event not found");

        assert_eq!(
            grant_event.1,
            (symbol_short!("ACC_GRT"), patient.clone(), doctor.clone()).into_val(&env)
        );
        let grant_payload: events::AccessGrantedEvent = grant_event.2.try_into_val(&env).unwrap();
        assert_eq!(grant_payload.patient, patient);
        assert_eq!(grant_payload.grantee, doctor);
        assert_eq!(grant_payload.level, AccessLevel::Read);
        assert_eq!(grant_payload.duration_seconds, 86400);

        assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);

        client.revoke_access(&patient, &doctor);
        let all_events = env.events().all();

        assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
        let revoke_event = all_events.get(all_events.len() - 1).unwrap();
        assert_eq!(
            revoke_event.1,
            (symbol_short!("ACC_REV"), patient.clone(), doctor.clone()).into_val(&env)
        );
        let revoke_payload: events::AccessRevokedEvent = revoke_event.2.try_into_val(&env).unwrap();
        assert_eq!(revoke_payload.patient, patient);
        assert_eq!(revoke_payload.grantee, doctor);
    }
}

#[cfg(test)]
mod test_rbac;