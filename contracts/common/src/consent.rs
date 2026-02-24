use std::collections::HashMap;

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
    /// Grant consent using an externally supplied timestamp.
    ///
    /// Callers must provide the current time (`now`) rather than relying
    /// on `SystemTime`. In a Soroban contract context this is the ledger
    /// timestamp; in off-chain tooling it is `SystemTime::now()` converted
    /// to seconds since the UNIX epoch.
    pub fn grant(
        &mut self,
        id: &str,
        subject: &str,
        grantee: &str,
        ctype: ConsentType,
        now: u64,
        ttl_secs: Option<u64>,
    ) {
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

    /// Check if consent is active at the given timestamp.
    ///
    /// Returns `false` when the record is missing, revoked, or expired
    /// relative to `now`.
    pub fn is_active(&self, id: &str, now: u64) -> bool {
        if let Some(r) = self.records.get(id) {
            if r.revoked {
                return false;
            }
            if let Some(exp) = r.expires_at {
                return now < exp;
            }
            return true;
        }
        false
    }
}
