use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    pub id: String,
    pub retention_seconds: u64,
}

#[derive(Default)]
pub struct RetentionManager {
    pub policies: Vec<RetentionPolicy>,
    pub created_at: u64,
}

impl RetentionManager {
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            policies: vec![],
            created_at: now,
        }
    }

    pub fn add_policy(&mut self, id: &str, seconds: u64) {
        self.policies.push(RetentionPolicy {
            id: id.to_string(),
            retention_seconds: seconds,
        });
    }

    pub fn should_purge(&self, created: u64, policy_id: &str) -> bool {
        if let Some(p) = self.policies.iter().find(|p| p.id == policy_id) {
            return created.saturating_add(p.retention_seconds)
                <= SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
        }
        false
    }
}
