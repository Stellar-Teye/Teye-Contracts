use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Credential {
    pub id: String,
    pub issuer: String,
    pub subject: String,
    pub claims: HashMap<String, String>,
    pub signature: Option<Vec<u8>>,
}

impl Credential {
    pub fn new(id: &str, issuer: &str, subject: &str) -> Self {
        Self {
            id: id.to_string(),
            issuer: issuer.to_string(),
            subject: subject.to_string(),
            claims: HashMap::new(),
            signature: None,
        }
    }

    pub fn add_claim(&mut self, key: &str, value: &str) {
        self.claims.insert(key.to_string(), value.to_string());
    }

    pub fn sign(&mut self, _private_key: &[u8]) {
        // Simplified: in a real implementation, do proper crypto signing.
        self.signature = Some(vec![1, 2, 3]);
    }

    pub fn verify(&self, _public_key: &[u8]) -> bool {
        // Simplified verification placeholder
        self.signature.is_some()
    }
}
