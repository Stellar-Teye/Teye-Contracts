#![no_std]
#![allow(clippy::too_many_arguments)]

extern crate alloc;
use alloc::{
    string::{String as StdString, ToString},
    vec::Vec as StdVec,
};

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
pub mod session;
pub mod types;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env, String,
    Symbol, Vec,
};

use teye_common::{
    admin_tiers, concurrency::ConflictEntry, concurrency::FieldChange,
    concurrency::ResolutionStrategy, multisig, progressive_auth, risk_engine,
    whitelist, AdminTier, KeyManager, MeteringOpType, UpdateOutcome, VersionStamp,
};

/// Re-export the contract-specific error type at the crate root.
pub use crate::types::ContractError;

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
pub use rbac::{Permission, Role, AccessPolicy, PolicyContext, evaluate_access_policies, set_user_credential, set_record_sensitivity, create_access_policy, CredentialType, SensitivityLevel, TimeRestriction};

// --- FIX: External KeyManager Client Definition ---
#[soroban_sdk::contractclient(name = "KeyManagerContractClient")]
pub trait KeyManagerInterface {
    fn derive_record_key(env: Env, root_key_id: BytesN<32>, record_id: u64) -> DerivedKey;
    fn derive_record_key_with_version(env: Env, root_key_id: BytesN<32>, record_id: u64, version: u32) -> DerivedKey;
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct DerivedKey {
    pub key: BytesN<32>,
    pub version: u32,
}

/// Storage keys for the contract
const ADMIN: Symbol = symbol_short!("ADMIN");
const PENDING_ADMIN: Symbol = symbol_short!("PEND_ADM");
const INITIALIZED: Symbol = symbol_short!("INIT");
const RATE_CFG: Symbol = symbol_short!("RL_IN_CFG");
const RATE_TRACK: Symbol = symbol_short!("RL_IN_TRK");

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

const ENC_CUR: Symbol = symbol_short!("ENC_CUR");
const ENC_KEY: Symbol = symbol_short!("ENC_KEY");
const KEY_MGR: Symbol = symbol_short!("KEY_MGR");
const KEY_MGR_KEY: Symbol = symbol_short!("KEY_MGRK");

/// Extends the time-to-live (TTL) for a storage key containing an Address.
fn extend_ttl_address_key(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Extends the time-to-live (TTL) for a storage key containing a u64 value.
fn extend_ttl_u64_key(env: &Env, key: &(Symbol, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Extends the time-to-live (TTL) for an access grant storage key.
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

fn rate_limit_action_hash(
    env: &Env,
    max_requests_per_window: u64,
    window_duration_seconds: u64,
) -> BytesN<32> {
    let mut payload = Bytes::new(env);
    payload.append(&Bytes::from_slice(env, b"SET_RATE"));
    payload.append(&Bytes::from_slice(env, &max_requests_per_window.to_be_bytes()));
    payload.append(&Bytes::from_slice(env, &window_duration_seconds.to_be_bytes()));
    env.crypto().sha256(&payload).into()
}

fn encryption_key_action_hash(env: &Env, version: &String, key: &String) -> BytesN<32> {
    let mut payload = Bytes::new(env);
    payload.append(&Bytes::from_slice(env, b"SET_ENC"));
    let version_std = version.to_string();
    let key_std = key.to_string();
    payload.append(&Bytes::from_slice(env, version_std.as_bytes()));
    payload.append(&Bytes::from_slice(env, key_std.as_bytes()));
    env.crypto().sha256(&payload).into()
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsentType { Treatment, Research, Sharing }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessLevel { None, Read, Write, Admin }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordType { Examination, Prescription, Diagnosis, Treatment, Surgery, LabResult }

#[contracttype]
#[derive(Clone, Debug)]
pub struct User {
    pub address: Address,
    pub role: Role,
    pub name: String,
    pub registered_at: u64,
    pub is_active: bool,
}

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

#[contract]
pub struct VisionRecordsContract;

#[contractimpl]
impl VisionRecordsContract {
    fn meter_op(_env: &Env, _caller: &Address, _op_type: MeteringOpType) {}

    fn emit_access_violation(env: &Env, caller: &Address, action: &str, required_permission: &str) {
        events::publish_access_violation(
            env,
            caller.clone(),
            String::from_str(env, action),
            String::from_str(env, required_permission),
        );
    }

    fn unauthorized<T>(env: &Env, caller: &Address, action: &str, req_perm: &str) -> Result<T, ContractError> {
        Self::emit_access_violation(env, caller, action, req_perm);
        Err(ContractError::Unauthorized)
    }

    fn get_key_manager_config(env: &Env) -> Option<(Address, BytesN<32>)> {
        let manager: Option<Address> = env.storage().instance().get(&KEY_MGR);
        let key_id: Option<BytesN<32>> = env.storage().instance().get(&KEY_MGR_KEY);
        match (manager, key_id) {
            (Some(mgr), Some(key)) => Some((mgr, key)),
            _ => None,
        }
    }

    fn enforce_rate_limit(env: &Env, caller: &Address) -> Result<(), ContractError> {
        let cfg: Option<(u64, u64)> = env.storage().instance().get(&RATE_CFG);
        let (max_req, window_sec) = match cfg {
            Some(c) => c,
            None => return Ok(()),
        };
        if max_req == 0 || window_sec == 0 { return Ok(()); }

        let now = env.ledger().timestamp();
        let key = (RATE_TRACK, caller.clone());
        let mut state: (u64, u64) = env.storage().persistent().get(&key).unwrap_or((0, now));

        if now >= state.1.saturating_add(window_sec) {
            state.0 = 0;
            state.1 = now;
        }

        state.0 = state.0.saturating_add(1);
        if state.0 > max_req { return Err(ContractError::RateLimitExceeded); }

        env.storage().persistent().set(&key, &state);
        Ok(())
    }

    /// Initialize the contract with an admin address
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(ContractError::AlreadyInitialized);
        }
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);
        rbac::assign_role(&env, admin.clone(), Role::Admin, 0);
        admin_tiers::set_super_admin(&env, &admin);
        admin_tiers::track_admin(&env, &admin);
        events::publish_initialized(&env, admin);
        Ok(())
    }

    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        env.storage().instance().get(&ADMIN).ok_or(ContractError::NotInitialized)
    }

    /// Register a new user
    pub fn register_user(
        env: Env,
        caller: Address,
        user: Address,
        role: Role,
        name: String,
    ) -> Result<(), ContractError> {
        circuit_breaker::require_not_paused(&env, &circuit_breaker::PauseScope::Function(symbol_short!("REG_USR")))?;
        caller.require_auth();

        if !rbac::has_permission(&env, &caller, &Permission::ManageUsers) {
            return Self::unauthorized(&env, &caller, "register_user", "permission:ManageUsers");
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

        events::publish_user_registered(&env, user, role, name);
        Ok(())
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
        let _guard = teye_common::ReentrancyGuard::new(&env);
        circuit_breaker::require_not_paused(&env, &circuit_breaker::PauseScope::Function(symbol_short!("ADD_REC")))?;
        caller.require_auth();

        Self::enforce_rate_limit(&env, &caller)?;
        validation::validate_data_hash(&data_hash)?;

        let has_perm = if caller == provider {
            rbac::has_permission(&env, &caller, &Permission::WriteRecord)
        } else {
            rbac::has_delegated_permission(&env, &provider, &caller, &Permission::WriteRecord)
        };

        if !has_perm && !rbac::has_permission(&env, &caller, &Permission::SystemAdmin) {
             return Self::unauthorized(&env, &caller, "add_record", "permission:WriteRecord");
        }

        // --- FIX: Logic to handle record ID and storage ---
        let counter_key = symbol_short!("REC_CTR");
        let record_id: u64 = env.storage().instance().get(&counter_key).unwrap_or(0) + 1;
        env.storage().instance().set(&counter_key, &record_id);

        let current_version: Option<String> = env.storage().instance().get(&ENC_CUR);

        let record = VisionRecord {
            id: record_id,
            patient: patient.clone(),
            provider: provider.clone(),
            record_type,
            data_hash,
            key_version: current_version,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
        };

        let key = (symbol_short!("RECORD"), record_id);
        env.storage().persistent().set(&key, &record);

        events::publish_record_added(&env, record_id, patient, provider);
        Ok(record_id)
    }

    pub fn get_user(env: Env, user: Address) -> Result<User, ContractError> {
        let key = (symbol_short!("USER"), user);
        env.storage().persistent().get(&key).ok_or(ContractError::UserNotFound)
    }
}