use soroban_sdk::{contracttype, Address, Bytes};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    KeyManager,
    UserRole(Address),
    PatientHistory(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessLevel {
    Full,     // Fixes Error #22
    Partial,
    ReadOnly,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Prescription {
    pub patient: Address,
    pub data: Bytes,       // Fixes Error #24
    pub created_at: u64,   // Fixes Error #25
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Admin,
    Doctor,
    Patient,
    User,
}