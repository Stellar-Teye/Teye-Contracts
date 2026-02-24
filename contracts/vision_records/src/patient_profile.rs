#![no_std]
use soroban_sdk::{contracttype, Address, String, Vec};

/// Emergency contact information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyContact {
    pub name: String,
    pub relationship: String,
    pub phone: String,
    pub email: String,
}

/// Insurance information (hashed values only for security)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsuranceInfo {
    pub provider_hash: String,      // Hash of insurance provider name
    pub policy_id_hash: String,     // Hash of policy ID
    pub group_id_hash: String,      // Hash of group ID (if applicable)
    pub verified_at: u64,           // Timestamp of last verification
}

/// Patient profile structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatientProfile {
    pub patient: Address,
    pub created_at: u64,
    pub updated_at: u64,
    pub is_active: bool,
    
    // Demographics (hashed for privacy)
    pub date_of_birth_hash: String,
    pub gender_hash: String,
    pub blood_type_hash: String,
    
    // Emergency contact
    pub emergency_contact: Option<EmergencyContact>,
    
    // Insurance information
    pub insurance_info: Option<InsuranceInfo>,
    
    // Medical history references (IPFS hashes or record IDs)
    pub medical_history_refs: Vec<String>,
}