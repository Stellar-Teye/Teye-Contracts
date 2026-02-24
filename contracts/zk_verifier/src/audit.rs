use soroban_sdk::{contracttype, Address, BytesN, Env};

/// Record of a successful ZK verification event.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRecord {
    /// The user who performed the verification.
    pub user: Address,
    /// The resource that was accessed.
    pub resource_id: BytesN<32>,
    /// The hash of the public inputs used in the proof.
    pub proof_hash: BytesN<32>,
    /// The ledger timestamp of the verification event.
    pub timestamp: u64,
}

/// Utility for logging and retrieving ZK verification audits.
pub struct AuditTrail;

impl AuditTrail {
    /// Logs a successful access verification event to persistent storage and emits an event.
    pub fn log_access(env: &Env, user: Address, resource_id: BytesN<32>, proof_hash: BytesN<32>) {
        let record = AuditRecord {
            user: user.clone(),
            resource_id: resource_id.clone(),
            proof_hash,
            timestamp: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&(&user, &resource_id), &record);
        env.events().publish((user, resource_id), record);
    }

    /// Fetches an audit record for a given user and resource from persistent storage.
    pub fn get_record(env: &Env, user: Address, resource_id: BytesN<32>) -> Option<AuditRecord> {
        env.storage().persistent().get(&(&user, &resource_id))
    }
}
