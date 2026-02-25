use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Vec};

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
    /// Hash of the previous audit record in the chain (zero for the first record).
    pub prev_hash: BytesN<32>,
}

/// Compute a keccak256 hash of an audit record's contents.
fn hash_record(env: &Env, record: &AuditRecord) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.extend_from_array(&record.proof_hash.to_array());
    buf.extend_from_array(&record.resource_id.to_array());
    buf.extend_from_array(&record.prev_hash.to_array());
    buf.extend_from_array(&record.timestamp.to_be_bytes());
    env.crypto().keccak256(&buf).into()
}

/// Utility for logging and retrieving ZK verification audits.
pub struct AuditTrail;

impl AuditTrail {
    /// Logs a successful access verification event to persistent storage and emits an event.
    /// Each new record is chained to the previous one via `prev_hash`.
    pub fn log_access(env: &Env, user: Address, resource_id: BytesN<32>, proof_hash: BytesN<32>) {
        let key = (&user, &resource_id);
        let mut chain: Vec<AuditRecord> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));

        let prev_hash = if chain.is_empty() {
            BytesN::from_array(env, &[0u8; 32])
        } else {
            match chain.last() {
                Some(last) => hash_record(env, &last),
                None => BytesN::from_array(env, &[0u8; 32]),
            }
        };

        let record = AuditRecord {
            user: user.clone(),
            resource_id: resource_id.clone(),
            proof_hash,
            timestamp: env.ledger().timestamp(),
            prev_hash,
        };

        chain.push_back(record.clone());
        env.storage().persistent().set(&key, &chain);
        #[allow(deprecated)]
        env.events().publish((user, resource_id), record);
    }

    /// Fetches the most recent audit record for a given user and resource.
    pub fn get_record(env: &Env, user: Address, resource_id: BytesN<32>) -> Option<AuditRecord> {
        let chain: Option<Vec<AuditRecord>> =
            env.storage().persistent().get(&(&user, &resource_id));
        chain.and_then(|c| {
            if c.is_empty() {
                None
            } else {
                Some(c.get(c.len() - 1).unwrap())
            }
        })
    }

    /// Fetches the full audit chain for a given user and resource.
    pub fn get_chain(env: &Env, user: Address, resource_id: BytesN<32>) -> Vec<AuditRecord> {
        env.storage()
            .persistent()
            .get(&(&user, &resource_id))
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Verifies that the audit chain is intact.
    /// Returns `true` if every record's `prev_hash` matches the hash of its predecessor
    /// and the first record has a zero `prev_hash`.
    pub fn verify_chain(env: &Env, user: Address, resource_id: BytesN<32>) -> bool {
        let chain = Self::get_chain(env, user, resource_id);
        if chain.is_empty() {
            return true;
        }

        let zero = BytesN::from_array(env, &[0u8; 32]);
        let first = match chain.first() {
            Some(item) => item,
            None => return true,
        };
        if first.prev_hash != zero {
            return false;
        }

        let mut i: u32 = 1;
        while i < chain.len() {
            let prev = match chain.get(i - 1) {
                Some(item) => item,
                None => return false,
            };
            let current = match chain.get(i) {
                Some(item) => item,
                None => return false,
            };
            if current.prev_hash != hash_record(env, &prev) {
                return false;
            }
            i += 1;
        }

        true
    }
}
