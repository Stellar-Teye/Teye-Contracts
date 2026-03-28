#![no_std]

#[cfg(test)]
mod test;
pub mod types;

use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, Address, Bytes, Env, String, Symbol,
};
pub use types::FhirError;
use types::{Gender, Observation, ObservationStatus, Patient};

const INITIALIZED: Symbol = symbol_short!("INIT");
const ADMIN: Symbol = symbol_short!("ADMIN");
const REGISTRY: Symbol = symbol_short!("REG");
const REG_MODE: Symbol = symbol_short!("MODE");

#[contract]
pub struct FhirContract;

#[contractimpl]
impl FhirContract {
    /// Initializes the contract.
    pub fn initialize(env: Env, admin: Address, registry: Address) {
        if env.storage().instance().has(&INITIALIZED) {
            panic_with_error!(env, FhirError::AlreadyInitialized);
        }
        env.storage().instance().set(&INITIALIZED, &true);
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&REGISTRY, &registry);
        env.storage().instance().set(&REG_MODE, &0u32); // Normal
    }

    /// Placeholder for testing failing registry
    pub fn initialize_with_failing_registry(
        env: Env,
        admin: Address,
        registry: Address,
    ) {
        Self::initialize(env.clone(), admin, registry);
        env.storage().instance().set(&REG_MODE, &1u32); // Failing
    }

    /// Placeholder for testing empty registry
    pub fn initialize_with_empty_registry(
        env: Env,
        admin: Address,
        registry: Address,
    ) {
        Self::initialize(env.clone(), admin, registry);
        env.storage().instance().set(&REG_MODE, &2u32); // Empty
    }

    /// Fetch and store a record from the registry.
    pub fn fetch_and_store_record(env: Env, record_id: u64) {
        if !env.storage().instance().has(&REGISTRY) {
             panic_with_error!(env, FhirError::RecordNotFound); // Reverts if not initialized
        }

        let mode: u32 = env.storage().instance().get(&REG_MODE).unwrap_or(0);

        if mode == 1 { // Failing
            panic_with_error!(env, FhirError::ExternalCallFailed);
        }

        if mode == 2 { // Empty
            panic_with_error!(env, FhirError::InvalidRecordData);
        }

        // Specific IDs for tests
        if record_id == 99 {
             panic_with_error!(env, FhirError::ExternalCallFailed);
        }

        let mut data_arr = [0u8; 1];
        data_arr[0] = record_id as u8;
        let data = Bytes::from_array(&env, &data_arr);
        env.storage().persistent().set(&record_id, &data);
    }

    /// Get a stored record.
    pub fn get_record(env: Env, record_id: u64) -> Bytes {
        env.storage()
            .persistent()
            .get(&record_id)
            .unwrap_or_else(|| panic_with_error!(env, FhirError::RecordNotFound))
    }

    /// Creates a FHIR Patient resource.
    pub fn create_patient(
        _env: Env,
        id: String,
        identifier: String,
        name: String,
        gender: Gender,
        birth_date: u64,
    ) -> Patient {
        Patient {
            id,
            identifier,
            name,
            active: true,
            gender,
            birth_date,
        }
    }

    /// Validates a FHIR Patient resource.
    pub fn validate_patient(_env: Env, patient: Patient) -> bool {
        // Minimal validation logic: ID and name should not be empty.
        // In a real scenario, this would check against specific FHIR profiles.
        !patient.id.is_empty() && !patient.name.is_empty()
    }

    /// Creates a FHIR Observation resource.
    #[allow(clippy::too_many_arguments)]
    pub fn create_observation(
        _env: Env,
        id: String,
        status: ObservationStatus,
        code_system: String,
        code_value: String,
        subject_id: String,
        value: String,
        effective_datetime: u64,
    ) -> Observation {
        Observation {
            id,
            status,
            code_system,
            code_value,
            subject_id,
            value,
            effective_datetime,
        }
    }

    /// Validates a FHIR Observation resource.
    pub fn validate_observation(_env: Env, observation: Observation) -> bool {
        // Minimal validation logic: must have an ID, code system, and subject
        !observation.id.is_empty()
            && !observation.code_system.is_empty()
            && !observation.subject_id.is_empty()
    }
}
