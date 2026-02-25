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
    /// Logs a tamper-evident audit event for the staking contract.
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

        // Use the segment "staking"
        let segment = LogSegmentId::new("staking").unwrap();

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
    }
}
