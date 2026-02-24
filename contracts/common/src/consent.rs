use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub enum ConsentType {
    Treatment,
    Research,
    Sharing,
}

#[derive(Debug, Clone)]
pub struct ConsentRecord {
    pub subject: String,
    pub grantee: String,
    pub consent_type: ConsentType,
    pub granted_at: u64,
    pub expires_at: Option<u64>,
    pub revoked: bool,
}

#[derive(Default)]
pub struct ConsentManager {
    pub records: HashMap<String, ConsentRecord>,
}

impl ConsentManager {
    pub fn grant(
        &mut self,
        id: &str,
        subject: &str,
        grantee: &str,
        ctype: ConsentType,
        ttl_secs: Option<u64>,
    ) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires = ttl_secs.and_then(|t| now.checked_add(t));
        self.records.insert(
            id.to_string(),
            ConsentRecord {
                subject: subject.to_string(),
                grantee: grantee.to_string(),
                consent_type: ctype,
                granted_at: now,
                expires_at: expires,
                revoked: false,
            },
        );
    }

    pub fn revoke(&mut self, id: &str) {
        if let Some(r) = self.records.get_mut(id) {
            r.revoked = true;
        }
    }

    pub fn is_active(&self, id: &str) -> bool {
        if let Some(r) = self.records.get(id) {
            if r.revoked {
                return false;
            }
            if let Some(exp) = r.expires_at {
                return SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    < exp;
            }
            return true;
        }
        false
    }
}
