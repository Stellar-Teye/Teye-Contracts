use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
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
