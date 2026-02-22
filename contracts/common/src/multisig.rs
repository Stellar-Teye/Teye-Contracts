use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone)]
pub struct MultisigPolicy {
    pub m: usize,
    pub keys: Vec<String>,
}

#[derive(Debug, Default)]
pub struct PendingTx {
    pub id: String,
    pub signer_set: BTreeSet<String>,
    pub required: usize,
}

#[derive(Default)]
pub struct MultisigManager {
    pub policies: HashMap<String, MultisigPolicy>,
    pub pending: HashMap<String, PendingTx>,
}

impl MultisigManager {
    pub fn add_policy(&mut self, name: &str, m: usize, keys: Vec<String>) {
        self.policies
            .insert(name.to_string(), MultisigPolicy { m, keys });
    }

    pub fn create_pending(&mut self, tx_id: &str, required: usize) {
        self.pending.insert(
            tx_id.to_string(),
            PendingTx {
                id: tx_id.to_string(),
                signer_set: BTreeSet::new(),
                required,
            },
        );
    }

    pub fn sign(&mut self, tx_id: &str, signer: &str) -> bool {
        if let Some(p) = self.pending.get_mut(tx_id) {
            p.signer_set.insert(signer.to_string());
            return p.signer_set.len() >= p.required;
        }
        false
    }

    pub fn is_executable(&self, tx_id: &str) -> bool {
        self.pending
            .get(tx_id)
            .map(|p| p.signer_set.len() >= p.required)
            .unwrap_or(false)
    }
}
