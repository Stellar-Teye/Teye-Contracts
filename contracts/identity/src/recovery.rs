use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RecoveryAgent {
    pub id: String,
    pub contact: String,
}

#[derive(Default)]
pub struct RecoveryManager {
    pub agents: HashMap<String, RecoveryAgent>,
    pub recovery_claims: HashMap<String, String>,
}

impl RecoveryManager {
    pub fn add_agent(&mut self, agent: RecoveryAgent) {
        self.agents.insert(agent.id.clone(), agent);
    }

    pub fn request_recovery(&mut self, did: &str, agent_id: &str) {
        // In a full implementation, validation and multi-party checks would be required.
        self.recovery_claims
            .insert(did.to_string(), agent_id.to_string());
    }

    pub fn execute_recovery(&mut self, did: &str) -> Option<String> {
        self.recovery_claims.remove(did)
    }
}
