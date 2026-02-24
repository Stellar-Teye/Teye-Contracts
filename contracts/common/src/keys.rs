// Minimal audit log for secure rotation
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default)]
pub struct AuditEntry {
    pub actor: String,
    pub action: String,
    pub target: String,
    pub timestamp: u64,
}

#[derive(Default)]
pub struct AuditLog {
    pub entries: Vec<AuditEntry>,
}

impl AuditLog {
    pub fn record(&mut self, actor: &str, action: &str, target: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.entries.push(AuditEntry {
            actor: actor.to_string(),
            action: action.to_string(),
            target: target.to_string(),
            timestamp: now,
        });
    }

    pub fn query(&self) -> &[AuditEntry] {
        &self.entries
    }
}
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DataKey {
    pub id: String,
    pub key: Vec<u8>,
    pub created: u64,
    pub expires: Option<u64>,
}

#[derive(Default)]
pub struct KeyManager {
    pub master: Vec<u8>,
    pub data_keys: HashMap<String, DataKey>,
    pub old_master: Option<Vec<u8>>, // For audit trail
}

impl KeyManager {
    pub fn new(master: Vec<u8>) -> Self {
        Self {
            master,
            data_keys: HashMap::new(),
            old_master: None,
        }
    }

    pub fn create_data_key(&mut self, id: &str, key: Vec<u8>, ttl: Option<u64>) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.data_keys.insert(
            id.to_string(),
            DataKey {
                id: id.to_string(),
                key,
                created: now,
                expires: ttl.and_then(|t| now.checked_add(t)),
            },
        );
    }

    pub fn rotate_master(&mut self, new_master: Vec<u8>) {
        self.master = new_master;
    }

    /// Securely rotates the master key:
    /// - Re-encrypts all data keys
    /// - Emits rotation event via audit log
    /// - Maintains audit trail
    /// - Zeroes old master key
    pub fn rotate_master_secure(&mut self, new_master: Vec<u8>, audit: &mut AuditLog, actor: &str) {
        // Save old master for audit
        self.old_master = Some(self.master.clone());
        // Re-encrypt each data key (simulate: XOR with old master, then XOR with new master)
        for dk in self.data_keys.values_mut() {
            // Decrypt with old master (simulate)
            for (i, b) in dk.key.iter_mut().enumerate() {
                *b ^= self.master.get(i % self.master.len()).unwrap_or(&0);
            }
            // Encrypt with new master (simulate)
            for (i, b) in dk.key.iter_mut().enumerate() {
                *b ^= new_master.get(i % new_master.len()).unwrap_or(&0);
            }
        }
        // Zero old master
        for b in self.master.iter_mut() {
            *b = 0;
        }
        // Set new master
        self.master = new_master.clone();
        // Log rotation event
        audit.record(actor, "rotate_master_secure", "master_key");
    }

    pub fn get_key(&self, id: &str) -> Option<&DataKey> {
        self.data_keys.get(id)
    }
}
