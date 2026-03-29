#![no_std]

use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, Address, Bytes, Env, IntoVal, Map,
    String, Symbol, Vec,
};

pub mod types;
pub use types::FhirError;
use types::{Gender, Observation, ObservationStatus, Patient};

use common::migration::{self, FieldTransform, Migration, SchemaVersion, CURRENT_VERSION};

// Storage keys
const INITIALIZED: Symbol = symbol_short!("INIT");
const ADMIN: Symbol = symbol_short!("ADMIN");
const RESOURCES: Symbol = symbol_short!("RES");
const VERSIONS: Symbol = symbol_short!("VER");

#[contract]
pub struct FhirContract;

#[contractimpl]
impl FhirContract {
    /// Initializes the contract with an admin address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&INITIALIZED) {
            panic_with_error!(env, FhirError::AlreadyInitialized);
        }
        env.storage().instance().set(&INITIALIZED, &true);
        env.storage().instance().set(&ADMIN, &admin);

        // Setup FHIR specific migrations
        setup_fhir_migrations(&env);
    }

    /// Registers a new FHIR resource.
    pub fn register_resource(env: Env, admin: Address, id: String, payload: Bytes) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        if admin != stored_admin {
            panic_with_error!(env, FhirError::Unauthorized);
        }

        if id.is_empty() || payload.is_empty() {
            panic_with_error!(env, FhirError::InvalidPayload);
        }

        let key = (RESOURCES, id.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(env, FhirError::RecordAlreadyExists);
        }

        let mut data = Map::new(&env);
        data.set(symbol_short!("payload"), payload.into_val(&env));
        data.set(
            symbol_short!("updated"),
            env.ledger().timestamp().into_val(&env),
        );

        env.storage().persistent().set(&key, &data);
        env.storage().persistent().set(&(VERSIONS, id), &1u32);
    }

    /// Get a stored resource, automatically applying lazy migrations.
    pub fn get_resource(env: Env, id: String) -> Bytes {
        let key = (RESOURCES, id.clone());
        let mut data: Map<Symbol, Bytes> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(env, FhirError::RecordNotFound));

        let version: u32 = env
            .storage()
            .persistent()
            .get(&(VERSIONS, id.clone()))
            .unwrap_or(1);

        // Lazy migration (forward to CURRENT_VERSION defined in common if not specified otherwise)
        let new_version = migration::lazy_read(&env, &mut data, version)
            .unwrap_or_else(|_| panic_with_error!(env, FhirError::MigrationFailed));

        if new_version != version {
            env.storage().persistent().set(&key, &data);
            env.storage()
                .persistent()
                .set(&(VERSIONS, id), &new_version);
        }

        data.get(symbol_short!("payload")).unwrap()
    }

    /// Manual migration for a specific resource.
    pub fn migrate_resource(env: Env, admin: Address, id: String, target_version: u32) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        if admin != stored_admin {
            panic_with_error!(env, FhirError::Unauthorized);
        }

        let key = (RESOURCES, id.clone());
        let mut data: Map<Symbol, Bytes> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(env, FhirError::RecordNotFound));

        let version: u32 = env
            .storage()
            .persistent()
            .get(&(VERSIONS, id.clone()))
            .unwrap_or(1);

        let reached = migration::migrate_forward(&env, &mut data, version, target_version)
            .unwrap_or_else(|_| panic_with_error!(env, FhirError::MigrationFailed));

        env.storage().persistent().set(&key, &data);
        env.storage().persistent().set(&(VERSIONS, id), &reached);
    }

    /// Update an existing FHIR resource.
    pub fn update_resource(env: Env, admin: Address, id: String, payload: Bytes) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        if admin != stored_admin {
            panic_with_error!(env, FhirError::Unauthorized);
        }

        let key = (RESOURCES, id.clone());
        let mut data: Map<Symbol, Bytes> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(env, FhirError::RecordNotFound));

        data.set(symbol_short!("payload"), payload);
        data.set(
            symbol_short!("updated"),
            env.ledger().timestamp().into_val(&env),
        );

        env.storage().persistent().set(&key, &data);
    }

    /// Deletes a FHIR resource.
    pub fn delete_resource(env: Env, admin: Address, id: String) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        if admin != stored_admin {
            panic_with_error!(env, FhirError::Unauthorized);
        }

        let key = (RESOURCES, id.clone());
        if !env.storage().persistent().has(&key) {
            panic_with_error!(env, FhirError::RecordNotFound);
        }
        env.storage().persistent().remove(&key);
        env.storage().persistent().remove(&(VERSIONS, id));
    }

    /// Creates a FHIR Patient resource (metadata only).
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
        !patient.id.is_empty() && !patient.name.is_empty()
    }

    /// Creates a FHIR Observation resource (metadata only).
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
        !observation.id.is_empty()
            && !observation.code_system.is_empty()
            && !observation.subject_id.is_empty()
    }
}

fn setup_fhir_migrations(env: &Env) {
    let m1 = Migration {
        from_version: 1,
        to_version: 2,
        description: String::from_str(env, "Add meta field to resource"),
        forward: {
            let mut v = Vec::new(env);
            v.push_back(FieldTransform::AddField(
                symbol_short!("meta"),
                Bytes::new(env),
            ));
            v
        },
        reverse: {
            let mut v = Vec::new(env);
            v.push_back(FieldTransform::RemoveField(symbol_short!("meta")));
            v
        },
    };
    let _ = migration::register_migration(env, m1);
}

#[cfg(test)]
mod test;
