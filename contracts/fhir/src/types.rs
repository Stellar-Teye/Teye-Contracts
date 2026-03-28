use soroban_sdk::{contracttype, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Gender {
    Male,
    Female,
    Other,
    Unknown,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Patient {
    pub id: String,
    pub identifier: String, // e.g. MRN
    pub name: String,
    pub active: bool,
    pub gender: Gender,
    pub birth_date: u64, // Unix timestamp
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservationStatus {
    Registered,
    Preliminary,
    Final,
    Amended,
    Corrected,
    Cancelled,
    EnteredInError,
    Unknown,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub id: String,
    pub status: ObservationStatus,
    pub code_system: String, // e.g., "LOINC", "SNOMED"
    pub code_value: String,
    pub subject_id: String, // Reference to Patient.id
    pub value: String,      // As string for flexibility, could be parsed as needed
    pub effective_datetime: u64,
}

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FhirError {
    AlreadyInitialized = 1,
    RecordNotFound = 2,
    ExternalCallFailed = 3,
    InvalidRecordData = 4,
    RecordAlreadyExists = 5,
}
