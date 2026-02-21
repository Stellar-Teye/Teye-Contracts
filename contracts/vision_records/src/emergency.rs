use soroban_sdk::{contracttype, symbol_short, Address, String, Symbol, Vec};

// ── Storage keys ──────────────────────────────────────────────
pub const EMRG_CTR: Symbol = symbol_short!("EMRG_CTR");

// ── Types ─────────────────────────────────────────────────────

/// Conditions that justify emergency access
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmergencyCondition {
    LifeThreatening,
    Unconscious,
    SurgicalEmergency,
    Masscasualties,
}

/// Status of an emergency access request
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmergencyStatus {
    Active,
    Expired,
    Revoked,
}

/// An emergency access grant — always time-limited
#[contracttype]
#[derive(Clone, Debug)]
pub struct EmergencyAccess {
    pub id: u64,
    pub patient: Address,
    pub requester: Address,
    pub condition: EmergencyCondition,
    /// Free-text attestation signed off by the requester
    pub attestation: String,
    pub granted_at: u64,
    pub expires_at: u64,
    pub status: EmergencyStatus,
    pub notified_contacts: Vec<Address>,
}

/// Immutable audit entry — written once, never deleted
#[contracttype]
#[derive(Clone, Debug)]
pub struct EmergencyAuditEntry {
    pub access_id: u64,
    pub actor: Address,
    pub action: String, // e.g. "GRANTED", "REVOKED", "ACCESSED"
    pub timestamp: u64,
}
