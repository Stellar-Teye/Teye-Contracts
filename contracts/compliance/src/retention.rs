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
    pub fn new(now: u64) -> Self {
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

    pub fn should_purge(&self, created: u64, policy_id: &str, now: u64) -> bool {
        if let Some(p) = self.policies.iter().find(|p| p.id == policy_id) {
            return created.saturating_add(p.retention_seconds) <= now;
        }
        false
    }
}
