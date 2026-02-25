extern crate alloc;
use alloc::vec::Vec;
use soroban_sdk::{contracttype, symbol_short, Address, BytesN, Env, String, Symbol};
use audit::types::LogSegmentId;
use audit::merkle_log::hash_leaf;

const AUDIT_LATEST_HASH: Symbol = symbol_short!("AUD_HASH");
const AUDIT_SEQUENCE: Symbol = symbol_short!("AUD_SEQ");

pub struct AuditManager;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditLogEvent {
    pub sequence: u64,
    pub timestamp: u64,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub result: String,
    pub prev_hash: BytesN<32>,
    pub entry_hash: BytesN<32>,
}

impl AuditManager {
    /// Logs a tamper-evident audit event for the vision records contract.
    pub fn log_event(
        env: &Env,
        actor: Address,
        action: &str,
        target: String,
        result: &str,
    ) {
        let mut sequence: u64 = env.storage().persistent().get(&AUDIT_SEQUENCE).unwrap_or(0);
        sequence += 1;

        let prev_hash_bytes: [u8; 32] = env.storage().persistent().get(&AUDIT_LATEST_HASH).unwrap_or([0u8; 32]);
        let timestamp = env.ledger().timestamp();

        // Use the segment "vision_records"
        let segment = LogSegmentId::new("vision_records").unwrap();

        let mut buf = Vec::new();
        buf.extend_from_slice(&sequence.to_le_bytes());
        buf.extend_from_slice(&timestamp.to_le_bytes());
        
        // Copy actor string to buffer
        let actor_str = actor.to_string();
        let mut actor_bytes = alloc::vec![0u8; actor_str.len() as usize];
        actor_str.copy_into_slice(&mut actor_bytes);
        buf.extend_from_slice(&actor_bytes);
        buf.push(0);

        buf.extend_from_slice(action.as_bytes());
        buf.push(0);

        // Copy target string to buffer
        let mut target_bytes = alloc::vec![0u8; target.len() as usize];
        target.copy_into_slice(&mut target_bytes);
        buf.extend_from_slice(&target_bytes);
        buf.push(0);

        buf.extend_from_slice(result.as_bytes());
        buf.push(0);
        buf.extend_from_slice(&prev_hash_bytes);
        buf.extend_from_slice(segment.as_bytes());

        let entry_hash = hash_leaf(&buf);

        // Update state
        env.storage().persistent().set(&AUDIT_LATEST_HASH, &entry_hash);
        env.storage().persistent().set(&AUDIT_SEQUENCE, &sequence);

        // Emit event
        let event_data = AuditLogEvent {
            sequence,
            timestamp,
            actor: actor_str,
            action: String::from_str(env, action),
            target,
            result: String::from_str(env, result),
            prev_hash: BytesN::from_array(env, &prev_hash_bytes),
            entry_hash: BytesN::from_array(env, &entry_hash),
        };

        env.events().publish((symbol_short!("AUDIT"), actor), event_data);

use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol, Vec};

// ── Storage keys ──────────────────────────────────────────────
pub const AUDIT_CTR: Symbol = symbol_short!("AUD_CTR");
const AUDIT_ENTRY: Symbol = symbol_short!("AUD_ENT");
const AUDIT_RECORD: Symbol = symbol_short!("AUD_REC");
const AUDIT_USER: Symbol = symbol_short!("AUD_USR");
const AUDIT_PATIENT: Symbol = symbol_short!("AUD_PAT");

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

/// Extends the time-to-live (TTL) for audit entry storage keys.
fn extend_ttl_audit_key(env: &Env, key: &(Symbol, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Extends the time-to-live (TTL) for audit by record keys.
fn extend_ttl_audit_record_key(env: &Env, key: &(Symbol, u64, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Extends the time-to-live (TTL) for audit by user keys.
fn extend_ttl_audit_user_key(env: &Env, key: &(Symbol, Address, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Extends the time-to-live (TTL) for audit by patient keys.
fn extend_ttl_audit_patient_key(env: &Env, key: &(Symbol, Address, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Types ─────────────────────────────────────────────────────

/// Type of access action
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AccessAction {
    Read = 1,
    Write = 2,
    Delete = 3,
    GrantAccess = 4,
    RevokeAccess = 5,
    EmergencyAccess = 6,
    Query = 7,
}

/// Result of an access attempt
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AccessResult {
    Success = 1,
    Failure = 2,
    Denied = 3,
    NotFound = 4,
    Expired = 5,
}

/// An audit log entry for access events
#[contracttype]
#[derive(Clone, Debug)]
pub struct AuditEntry {
    pub id: u64,
    pub timestamp: u64,
    pub actor: Address,         // Who performed the action
    pub patient: Address,       // Patient whose record was accessed
    pub record_id: Option<u64>, // Record ID if applicable
    pub action: AccessAction,
    pub result: AccessResult,
    pub reason: Option<String>,     // Failure reason or additional context
    pub ip_address: Option<String>, // Optional IP address (for off-chain tracking)
    pub user_agent: Option<String>, // Optional user agent (for off-chain tracking)
}

// ── Storage Functions ────────────────────────────────────────

/// Increments and returns the next audit entry ID
pub fn increment_audit_counter(env: &Env) -> u64 {
    let current: u64 = env.storage().instance().get(&AUDIT_CTR).unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&AUDIT_CTR, &next);
    next
}

/// Stores an audit entry
pub fn add_audit_entry(env: &Env, entry: &AuditEntry) {
    // Store by entry ID
    let key = (AUDIT_ENTRY, entry.id);
    env.storage().persistent().set(&key, entry);
    extend_ttl_audit_key(env, &key);

    // Index by record ID for quick lookup
    if let Some(record_id) = entry.record_id {
        let record_key = (AUDIT_RECORD, record_id, entry.id);
        env.storage().persistent().set(&record_key, &true);
        extend_ttl_audit_record_key(env, &record_key);
    }

    // Index by actor (user) for quick lookup
    let user_key = (AUDIT_USER, entry.actor.clone(), entry.id);
    env.storage().persistent().set(&user_key, &true);
    extend_ttl_audit_user_key(env, &user_key);

    // Index by patient for quick lookup
    let patient_key = (AUDIT_PATIENT, entry.patient.clone(), entry.id);
    env.storage().persistent().set(&patient_key, &true);
    extend_ttl_audit_patient_key(env, &patient_key);
}

/// Retrieves an audit entry by ID
pub fn get_audit_entry(env: &Env, entry_id: u64) -> Option<AuditEntry> {
    let key = (AUDIT_ENTRY, entry_id);
    env.storage().persistent().get(&key)
}

/// Gets all audit entries for a specific record
pub fn get_record_audit_log(env: &Env, record_id: u64) -> Vec<AuditEntry> {
    let mut entries = Vec::new(env);
    let counter: u64 = env.storage().instance().get(&AUDIT_CTR).unwrap_or(0);
    if counter == 0 {
        return entries;
    }
    let start_id = if counter > 1000 { counter - 1000 } else { 1 };

    for id in start_id..=counter {
        let record_key = (AUDIT_RECORD, record_id, id);
        if env
            .storage()
            .persistent()
            .get::<_, bool>(&record_key)
            .is_some()
        {
            if let Some(entry) = get_audit_entry(env, id) {
                entries.push_back(entry);
            }
        }
    }
    entries
}

/// Gets all audit entries for a specific user (actor)
pub fn get_user_audit_log(env: &Env, user: &Address) -> Vec<AuditEntry> {
    let mut entries = Vec::new(env);
    let counter: u64 = env.storage().instance().get(&AUDIT_CTR).unwrap_or(0);
    let start_id = if counter > 1000 { counter - 1000 } else { 1 };

    for id in start_id..=counter {
        let user_key = (AUDIT_USER, user.clone(), id);
        if env
            .storage()
            .persistent()
            .get::<_, bool>(&user_key)
            .is_some()
        {
            if let Some(entry) = get_audit_entry(env, id) {
                entries.push_back(entry);
            }
        }
    }
    entries
}

/// Gets all audit entries for a specific patient
pub fn get_patient_audit_log(env: &Env, patient: &Address) -> Vec<AuditEntry> {
    let mut entries = Vec::new(env);
    let counter: u64 = env.storage().instance().get(&AUDIT_CTR).unwrap_or(0);
    let start_id = if counter > 1000 { counter - 1000 } else { 1 };

    for id in start_id..=counter {
        let patient_key = (AUDIT_PATIENT, patient.clone(), id);
        if env
            .storage()
            .persistent()
            .get::<_, bool>(&patient_key)
            .is_some()
        {
            if let Some(entry) = get_audit_entry(env, id) {
                entries.push_back(entry);
            }
        }
    }
    entries
}

/// Gets audit entries filtered by action type
pub fn get_audit_log_by_action(env: &Env, action: AccessAction) -> Vec<AuditEntry> {
    let mut entries = Vec::new(env);
    let counter: u64 = env.storage().instance().get(&AUDIT_CTR).unwrap_or(0);
    let start_id = if counter > 1000 { counter - 1000 } else { 1 };

    for id in start_id..=counter {
        if let Some(entry) = get_audit_entry(env, id) {
            if entry.action == action {
                entries.push_back(entry);
            }
        }
    }
    entries
}

/// Gets audit entries filtered by result
pub fn get_audit_log_by_result(env: &Env, result: AccessResult) -> Vec<AuditEntry> {
    let mut entries = Vec::new(env);
    let counter: u64 = env.storage().instance().get(&AUDIT_CTR).unwrap_or(0);
    let start_id = if counter > 1000 { counter - 1000 } else { 1 };

    for id in start_id..=counter {
        if let Some(entry) = get_audit_entry(env, id) {
            if entry.result == result {
                entries.push_back(entry);
            }
        }
    }
    entries
}

/// Gets audit entries within a time range
pub fn get_audit_log_by_time_range(env: &Env, start_time: u64, end_time: u64) -> Vec<AuditEntry> {
    let mut entries = Vec::new(env);
    let counter: u64 = env.storage().instance().get(&AUDIT_CTR).unwrap_or(0);
    let start_id = if counter > 1000 { counter - 1000 } else { 1 };

    for id in start_id..=counter {
        if let Some(entry) = get_audit_entry(env, id) {
            if entry.timestamp >= start_time && entry.timestamp <= end_time {
                entries.push_back(entry);
            }
        }
    }
    entries
}

/// Gets recent audit entries (last N entries)
pub fn get_recent_audit_log(env: &Env, limit: u64) -> Vec<AuditEntry> {
    let mut entries = Vec::new(env);
    let counter: u64 = env.storage().instance().get(&AUDIT_CTR).unwrap_or(0);
    let start_id = if counter > limit { counter - limit } else { 1 };

    for id in start_id..=counter {
        if let Some(entry) = get_audit_entry(env, id) {
            entries.push_back(entry);
        }
    }
    entries
}

/// Helper function to create an audit entry
pub fn create_audit_entry(
    env: &Env,
    actor: Address,
    patient: Address,
    record_id: Option<u64>,
    action: AccessAction,
    result: AccessResult,
    reason: Option<String>,
) -> AuditEntry {
    let id = increment_audit_counter(env);
    AuditEntry {
        id,
        timestamp: env.ledger().timestamp(),
        actor,
        patient,
        record_id,
        action,
        result,
        reason,
        ip_address: None,
        user_agent: None,
    }
}
