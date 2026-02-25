use soroban_sdk::{contracttype, Address, String, Vec};

/// Supported EMR/EHR system types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmrSystem {
    EpicFhir,
    CernerMillennium,
    Allscripts,
    Athenahealth,
    Custom,
}

/// Status of an EMR provider registration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderStatus {
    Pending,
    Active,
    Suspended,
    Revoked,
}

/// Data exchange protocol format
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataFormat {
    FhirR4,
    Hl7V2,
    CcdA,
    Custom,
}

/// Sync operation status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyncStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    PartialSuccess,
}

/// Registered EMR provider
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmrProvider {
    pub provider_id: String,
    pub name: String,
    pub emr_system: EmrSystem,
    pub endpoint_url: String,
    pub data_format: DataFormat,
    pub status: ProviderStatus,
    pub registered_by: Address,
    pub registered_at: u64,
}

/// Field mapping between EMR system fields and Teye internal fields
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldMapping {
    pub mapping_id: String,
    pub provider_id: String,
    pub source_field: String,
    pub target_field: String,
    pub transform_rule: String,
}

/// A data exchange record tracking data sent/received
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataExchangeRecord {
    pub exchange_id: String,
    pub provider_id: String,
    pub patient_id: String,
    pub direction: ExchangeDirection,
    pub data_format: DataFormat,
    pub resource_type: String,
    pub record_hash: String,
    pub timestamp: u64,
    pub status: SyncStatus,
}

/// Direction of data exchange
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExchangeDirection {
    Import,
    Export,
}

/// Sync verification result
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncVerification {
    pub verification_id: String,
    pub exchange_id: String,
    pub source_hash: String,
    pub target_hash: String,
    pub is_consistent: bool,
    pub verified_at: u64,
    pub discrepancies: Vec<String>,
}
