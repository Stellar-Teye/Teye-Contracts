use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

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
}

impl KeyManager {
    pub fn new(master: Vec<u8>) -> Self {
        Self {
            master,
            data_keys: HashMap::new(),
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

    pub fn get_key(&self, id: &str) -> Option<&DataKey> {
        self.data_keys.get(id)
    }
}
