#![no_std]
#![allow(deprecated)]
#![allow(clippy::too_many_arguments)]

mod attestation;
mod derivation;
mod hierarchy;
mod rotation;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
    Symbol, Vec,
};

use identity::IdentityContractClient;

use attestation::attest_record;
use derivation::{derive_child_key, derive_record_key};
use hierarchy::validate_child_level;
use rotation::rotation_due;

const ADMIN: Symbol = symbol_short!("ADMIN");
const IDENTITY: Symbol = symbol_short!("IDENTITY");
const AUDIT_SEQ: Symbol = symbol_short!("AUD_SEQ");
const AUDIT_TAIL: Symbol = symbol_short!("AUD_TAIL");
const KEY: Symbol = symbol_short!("KEY");
const KEY_VER: Symbol = symbol_short!("KEY_VER");
const RECOVERY: Symbol = symbol_short!("RECOV");
const AUDIT: Symbol = symbol_short!("AUDIT");

const RECOVERY_COOLDOWN: u64 = 86_400; // 24 hours

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyType {
    Signing = 1,
    Encryption = 2,
    Authentication = 3,
    Delegation = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyLevel {
    Master = 1,
    Contract = 2,
    Operation = 3,
    Session = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyStatus {
    Active = 1,
    Revoked = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyPolicy {
    pub max_uses: u32,
    pub not_before: u64,
    pub not_after: u64,
    pub allowed_ops: Vec<Symbol>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyRecord {
    pub id: BytesN<32>,
    pub owner: Address,
    pub parent: Option<BytesN<32>>,
    pub level: KeyLevel,
    pub key_type: KeyType,
    pub chain_code: BytesN<32>,
    pub current_version: u32,
    pub created_at: u64,
    pub last_rotated: u64,
    pub rotation_interval: u64,
    pub uses: u32,
    pub policy: KeyPolicy,
    pub status: KeyStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyVersion {
    pub version: u32,
    pub key_bytes: BytesN<32>,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedKey {
    pub key: BytesN<32>,
    pub version: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryRequest {
    pub key_id: BytesN<32>,
    pub new_key: BytesN<32>,
    pub approvals: Vec<Address>,
    pub initiated_at: u64,
    pub execute_after: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEntry {
    pub seq: u64,
    pub actor: Address,
    pub action: Symbol,
    pub key_id: Option<BytesN<32>>,
    pub timestamp: u64,
    pub details_hash: BytesN<32>,
    pub prev_hash: BytesN<32>,
    pub entry_hash: BytesN<32>,
}

#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ContractError {
    NotInitialized = 1,
    Unauthorized = 2,
    KeyNotFound = 3,
    InvalidHierarchy = 4,
    InvalidPolicy = 5,
    PolicyViolation = 6,
    RotationNotDue = 7,
    RecoveryAlreadyActive = 8,
    RecoveryNotActive = 9,
    NotAGuardian = 10,
    AlreadyApproved = 11,
    InsufficientApprovals = 12,
    CooldownNotExpired = 13,
    KeyRevoked = 14,
}

#[contract]
pub struct KeyManagerContract;

#[contractimpl]
#[allow(clippy::too_many_arguments)]
impl KeyManagerContract {
    pub fn initialize(env: Env, admin: Address, identity_contract: Address) {
        if env.storage().instance().has(&ADMIN) {
            return;
        }
        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&IDENTITY, &identity_contract);
    }

    pub fn set_identity_contract(
        env: Env,
        caller: Address,
        identity_contract: Address,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &caller)?;
        env.storage().instance().set(&IDENTITY, &identity_contract);
        Ok(())
    }

    pub fn create_master_key(
        env: Env,
        caller: Address,
        key_type: KeyType,
        policy: KeyPolicy,
        rotation_interval: u64,
        key_bytes: BytesN<32>,
    ) -> Result<BytesN<32>, ContractError> {
        Self::require_admin(&env, &caller)?;
        Self::validate_policy(&policy)?;

        let now = env.ledger().timestamp();
        let id = Self::new_key_id(&env, None, key_type.clone(), KeyLevel::Master, now, 0);
        let chain_code = Self::chain_code_from(&env, &key_bytes, 0, false);

        let record = KeyRecord {
            id: id.clone(),
            owner: caller.clone(),
            parent: None,
            level: KeyLevel::Master,
            key_type,
            chain_code,
            current_version: 1,
            created_at: now,
            last_rotated: now,
            rotation_interval,
            uses: 0,
            policy,
            status: KeyStatus::Active,
        };

        Self::store_key_record(&env, &record);
        Self::store_key_version(&env, &record.id, 1, key_bytes, now);

        Self::audit(&env, caller, symbol_short!("KEY_NEW"), Some(record.id.clone()), &record.id);

        Ok(record.id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn derive_key(
        env: Env,
        caller: Address,
        parent_id: BytesN<32>,
        child_level: KeyLevel,
        index: u32,
        hardened: bool,
        key_type: KeyType,
        policy: KeyPolicy,
        rotation_interval: u64,
    ) -> Result<BytesN<32>, ContractError> {
        caller.require_auth();
        let mut parent = Self::load_key_record(&env, &parent_id)?;
        Self::require_owner_or_admin(&env, &caller, &parent.owner)?;
        Self::ensure_active(&parent)?;
        validate_child_level(parent.level.clone(), child_level.clone())?;
        Self::validate_policy(&policy)?;

        let (parent_key, _) = Self::load_key_version(&env, &parent_id, parent.current_version)?;
        let (child_key, child_chain) =
            derive_child_key(&env, &parent_key, &parent.chain_code, index, hardened);

        let now = env.ledger().timestamp();
        let child_id = Self::new_key_id(
            &env,
            Some(parent_id.clone()),
            key_type.clone(),
            child_level.clone(),
            now,
            index,
        );

        let record = KeyRecord {
            id: child_id.clone(),
            owner: parent.owner.clone(),
            parent: Some(parent_id.clone()),
            level: child_level,
            key_type,
            chain_code: child_chain,
            current_version: 1,
            created_at: now,
            last_rotated: now,
            rotation_interval,
            uses: 0,
            policy,
            status: KeyStatus::Active,
        };

        Self::store_key_record(&env, &record);
        Self::store_key_version(&env, &record.id, 1, child_key, now);

        parent.uses = parent.uses.saturating_add(1);
        Self::store_key_record(&env, &parent);

        Self::audit(
            &env,
            caller,
            symbol_short!("KEY_DER"),
            Some(record.id.clone()),
            &record.id,
        );

        Ok(record.id)
    }

    pub fn use_key(
        env: Env,
        caller: Address,
        key_id: BytesN<32>,
        operation: Symbol,
    ) -> Result<BytesN<32>, ContractError> {
        caller.require_auth();
        let mut record = Self::load_key_record(&env, &key_id)?;
        Self::require_owner_or_admin(&env, &caller, &record.owner)?;
        Self::ensure_active(&record)?;
        Self::enforce_policy(&record, &operation, env.ledger().timestamp())?;

        record.uses = record.uses.saturating_add(1);
        Self::store_key_record(&env, &record);

        let (key_bytes, _) = Self::load_key_version(&env, &key_id, record.current_version)?;
        Self::audit(&env, caller, symbol_short!("KEY_USE"), Some(key_id), &key_bytes);
        Ok(key_bytes)
    }

    pub fn derive_record_key(
        env: Env,
        key_id: BytesN<32>,
        record_id: u64,
    ) -> Result<DerivedKey, ContractError> {
        let record = Self::load_key_record(&env, &key_id)?;
        Self::ensure_active(&record)?;
        let (key_bytes, _) = Self::load_key_version(&env, &key_id, record.current_version)?;
        let derived = derive_record_key(&env, &key_bytes, record_id);
        Ok(DerivedKey {
            key: derived,
            version: record.current_version,
        })
    }

    pub fn derive_record_key_with_version(
        env: Env,
        key_id: BytesN<32>,
        record_id: u64,
        version: u32,
    ) -> Result<DerivedKey, ContractError> {
        let _record = Self::load_key_record(&env, &key_id)?;
        let (key_bytes, _) = Self::load_key_version(&env, &key_id, version)?;
        let derived = derive_record_key(&env, &key_bytes, record_id);
        Ok(DerivedKey {
            key: derived,
            version,
        })
    }

    pub fn rotate_key(env: Env, caller: Address, key_id: BytesN<32>) -> Result<u32, ContractError> {
        caller.require_auth();
        let mut record = Self::load_key_record(&env, &key_id)?;
        Self::require_owner_or_admin(&env, &caller, &record.owner)?;
        Self::ensure_active(&record)?;

        let now = env.ledger().timestamp();
        if record.rotation_interval > 0 && !rotation_due(now, record.last_rotated, record.rotation_interval)
        {
            return Err(ContractError::RotationNotDue);
        }

        let (current_key, _) = Self::load_key_version(&env, &key_id, record.current_version)?;
        let new_key = Self::rotate_material(&env, &current_key, now);

        let next_version = record.current_version.saturating_add(1);
        record.current_version = next_version;
        record.last_rotated = now;
        Self::store_key_record(&env, &record);
        Self::store_key_version(&env, &key_id, next_version, new_key, now);

        Self::audit(&env, caller, symbol_short!("KEY_ROT"), Some(key_id), &record.id);

        Ok(next_version)
    }

    pub fn revoke_key(env: Env, caller: Address, key_id: BytesN<32>) -> Result<(), ContractError> {
        caller.require_auth();
        let mut record = Self::load_key_record(&env, &key_id)?;
        Self::require_owner_or_admin(&env, &caller, &record.owner)?;
        record.status = KeyStatus::Revoked;
        Self::store_key_record(&env, &record);
        Self::audit(&env, caller, symbol_short!("KEY_RVK"), Some(key_id), &record.id);
        Ok(())
    }

    pub fn attest_key(env: Env, key_id: BytesN<32>) -> Result<BytesN<32>, ContractError> {
        let record = Self::load_key_record(&env, &key_id)?;
        Ok(attest_record(&env, &record))
    }

    pub fn initiate_recovery(
        env: Env,
        guardian: Address,
        key_id: BytesN<32>,
        new_key: BytesN<32>,
    ) -> Result<(), ContractError> {
        guardian.require_auth();
        let record = Self::load_key_record(&env, &key_id)?;
        let (guardians, threshold) = Self::load_guardians(&env, &record.owner)?;
        if !guardians.contains(&guardian) {
            return Err(ContractError::NotAGuardian);
        }

        let key = (RECOVERY, key_id.clone());
        if env.storage().persistent().has(&key) {
            return Err(ContractError::RecoveryAlreadyActive);
        }

        let mut approvals = Vec::new(&env);
        approvals.push_back(guardian.clone());
        let now = env.ledger().timestamp();
        let request = RecoveryRequest {
            key_id: key_id.clone(),
            new_key,
            approvals,
            initiated_at: now,
            execute_after: now.saturating_add(RECOVERY_COOLDOWN),
        };
        env.storage().persistent().set(&key, &request);

        let details = Self::hash_recovery_details(&env, &request, threshold);
        Self::audit(&env, guardian, symbol_short!("REC_NEW"), Some(key_id), &details);

        Ok(())
    }

    pub fn approve_recovery(
        env: Env,
        guardian: Address,
        key_id: BytesN<32>,
    ) -> Result<(), ContractError> {
        guardian.require_auth();
        let record = Self::load_key_record(&env, &key_id)?;
        let (guardians, threshold) = Self::load_guardians(&env, &record.owner)?;
        if !guardians.contains(&guardian) {
            return Err(ContractError::NotAGuardian);
        }

        let key = (RECOVERY, key_id.clone());
        let mut request: RecoveryRequest = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::RecoveryNotActive)?;

        if request.approvals.contains(&guardian) {
            return Err(ContractError::AlreadyApproved);
        }
        request.approvals.push_back(guardian.clone());
        env.storage().persistent().set(&key, &request);

        let details = Self::hash_recovery_details(&env, &request, threshold);
        Self::audit(&env, guardian, symbol_short!("REC_APP"), Some(key_id), &details);

        Ok(())
    }

    pub fn execute_recovery(
        env: Env,
        caller: Address,
        key_id: BytesN<32>,
    ) -> Result<u32, ContractError> {
        caller.require_auth();
        let record = Self::load_key_record(&env, &key_id)?;
        let (_guardians, threshold) = Self::load_guardians(&env, &record.owner)?;

        let key = (RECOVERY, key_id.clone());
        let request: RecoveryRequest = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::RecoveryNotActive)?;

        if request.approvals.len() < threshold {
            return Err(ContractError::InsufficientApprovals);
        }

        let now = env.ledger().timestamp();
        if now < request.execute_after {
            return Err(ContractError::CooldownNotExpired);
        }

        let mut record = record;
        let next_version = record.current_version.saturating_add(1);
        record.current_version = next_version;
        record.last_rotated = now;
        Self::store_key_record(&env, &record);
        Self::store_key_version(&env, &key_id, next_version, request.new_key, now);

        env.storage().persistent().remove(&key);

        Self::audit(
            &env,
            caller,
            symbol_short!("REC_EXE"),
            Some(key_id),
            &record.id,
        );

        Ok(next_version)
    }

    pub fn get_key_record(env: Env, key_id: BytesN<32>) -> Option<KeyRecord> {
        env.storage().persistent().get(&(KEY, key_id))
    }

    pub fn get_key_version(
        env: Env,
        key_id: BytesN<32>,
        version: u32,
    ) -> Option<KeyVersion> {
        env.storage()
            .persistent()
            .get(&(KEY_VER, key_id, version))
    }

    pub fn get_audit_entry(env: Env, seq: u64) -> Option<AuditEntry> {
        env.storage().persistent().get(&(AUDIT, seq))
    }

    pub fn get_audit_tail(env: Env) -> Option<BytesN<32>> {
        env.storage().instance().get(&AUDIT_TAIL)
    }

    fn require_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)?;
        if caller != &admin {
            return Err(ContractError::Unauthorized);
        }
        Ok(())
    }

    fn require_owner_or_admin(
        env: &Env,
        caller: &Address,
        owner: &Address,
    ) -> Result<(), ContractError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)?;
        if caller != owner && caller != &admin {
            return Err(ContractError::Unauthorized);
        }
        Ok(())
    }

    fn ensure_active(record: &KeyRecord) -> Result<(), ContractError> {
        if record.status == KeyStatus::Revoked {
            return Err(ContractError::KeyRevoked);
        }
        Ok(())
    }

    fn validate_policy(policy: &KeyPolicy) -> Result<(), ContractError> {
        if policy.not_after > 0 && policy.not_before > 0 && policy.not_after <= policy.not_before {
            return Err(ContractError::InvalidPolicy);
        }
        Ok(())
    }

    fn enforce_policy(
        record: &KeyRecord,
        operation: &Symbol,
        now: u64,
    ) -> Result<(), ContractError> {
        if record.policy.max_uses > 0 && record.uses >= record.policy.max_uses {
            return Err(ContractError::PolicyViolation);
        }
        if record.policy.not_before > 0 && now < record.policy.not_before {
            return Err(ContractError::PolicyViolation);
        }
        if record.policy.not_after > 0 && now > record.policy.not_after {
            return Err(ContractError::PolicyViolation);
        }
        if !record.policy.allowed_ops.is_empty() && !record.policy.allowed_ops.contains(operation) {
            return Err(ContractError::PolicyViolation);
        }
        Ok(())
    }

    fn new_key_id(
        env: &Env,
        parent: Option<BytesN<32>>,
        key_type: KeyType,
        level: KeyLevel,
        now: u64,
        index: u32,
    ) -> BytesN<32> {
        let mut data = Bytes::new(env);
        match parent {
            Some(p) => data.extend_from_array(&p.to_array()),
            None => data.extend_from_array(b"root"),
        }
        data.extend_from_array(&[key_type as u8]);
        data.extend_from_array(&[level as u8]);
        data.extend_from_array(&now.to_be_bytes());
        data.extend_from_array(&index.to_be_bytes());
        env.crypto().sha256(&data).into()
    }

    fn chain_code_from(env: &Env, key_bytes: &BytesN<32>, index: u32, hardened: bool) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.extend_from_array(&key_bytes.to_array());
        data.extend_from_array(&index.to_be_bytes());
        data.extend_from_array(&[if hardened { 1 } else { 0 }]);
        data.extend_from_array(b"chain");
        env.crypto().sha256(&data).into()
    }

    fn rotate_material(env: &Env, key_bytes: &BytesN<32>, now: u64) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.extend_from_array(&key_bytes.to_array());
        data.extend_from_array(&now.to_be_bytes());
        data.extend_from_array(b"rotate");
        env.crypto().sha256(&data).into()
    }

    fn store_key_record(env: &Env, record: &KeyRecord) {
        env.storage()
            .persistent()
            .set(&(KEY, record.id.clone()), record);
    }

    fn store_key_version(
        env: &Env,
        key_id: &BytesN<32>,
        version: u32,
        key_bytes: BytesN<32>,
        created_at: u64,
    ) {
        let version_record = KeyVersion {
            version,
            key_bytes,
            created_at,
        };
        env.storage()
            .persistent()
            .set(&(KEY_VER, key_id.clone(), version), &version_record);
    }

    fn load_key_record(env: &Env, key_id: &BytesN<32>) -> Result<KeyRecord, ContractError> {
        env.storage()
            .persistent()
            .get(&(KEY, key_id.clone()))
            .ok_or(ContractError::KeyNotFound)
    }

    fn load_key_version(
        env: &Env,
        key_id: &BytesN<32>,
        version: u32,
    ) -> Result<(BytesN<32>, u64), ContractError> {
        let entry: KeyVersion = env
            .storage()
            .persistent()
            .get(&(KEY_VER, key_id.clone(), version))
            .ok_or(ContractError::KeyNotFound)?;
        Ok((entry.key_bytes, entry.created_at))
    }

    fn load_guardians(
        env: &Env,
        owner: &Address,
    ) -> Result<(Vec<Address>, u32), ContractError> {
        let identity_addr: Address = env
            .storage()
            .instance()
            .get(&IDENTITY)
            .ok_or(ContractError::NotInitialized)?;
        let client = IdentityContractClient::new(env, &identity_addr);
        let guardians = client.get_guardians(owner);
        let threshold = client.get_recovery_threshold(owner);
        Ok((guardians, threshold))
    }

    fn audit(
        env: &Env,
        actor: Address,
        action: Symbol,
        key_id: Option<BytesN<32>>,
        details: &BytesN<32>,
    ) {
        let mut seq: u64 = env.storage().instance().get(&AUDIT_SEQ).unwrap_or(0);
        seq = seq.saturating_add(1);

        let prev_hash = env
            .storage()
            .instance()
            .get(&AUDIT_TAIL)
            .unwrap_or(BytesN::from_array(env, &[0u8; 32]));

        let entry_hash = Self::hash_audit(env, &prev_hash, details);

        let entry = AuditEntry {
            seq,
            actor: actor.clone(),
            action,
            key_id,
            timestamp: env.ledger().timestamp(),
            details_hash: details.clone(),
            prev_hash,
            entry_hash: entry_hash.clone(),
        };

        env.storage().persistent().set(&(AUDIT, seq), &entry);
        env.storage().instance().set(&AUDIT_SEQ, &seq);
        env.storage().instance().set(&AUDIT_TAIL, &entry_hash);

        env.events().publish((symbol_short!("AUDIT"), seq), entry);
    }

    fn hash_audit(env: &Env, prev: &BytesN<32>, details: &BytesN<32>) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.extend_from_array(&prev.to_array());
        data.extend_from_array(&details.to_array());
        env.crypto().sha256(&data).into()
    }

    fn hash_recovery_details(
        env: &Env,
        request: &RecoveryRequest,
        threshold: u32,
    ) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.extend_from_array(&request.key_id.to_array());
        data.extend_from_array(&request.new_key.to_array());
        data.extend_from_array(&request.initiated_at.to_be_bytes());
        data.extend_from_array(&request.execute_after.to_be_bytes());
        data.extend_from_array(&threshold.to_be_bytes());
        data.extend_from_array(&request.approvals.len().to_be_bytes());
        env.crypto().sha256(&data).into()
    }
}
