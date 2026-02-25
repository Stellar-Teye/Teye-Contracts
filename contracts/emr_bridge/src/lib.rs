#![no_std]
#![allow(clippy::too_many_arguments)]

pub mod events;
pub mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, String, Symbol, Vec};
use types::{
    DataExchangeRecord, DataFormat, EmrProvider, EmrSystem, ExchangeDirection, FieldMapping,
    ProviderStatus, SyncStatus, SyncVerification,
};

/// Storage keys
const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");

/// TTL constants for persistent storage (in ledgers)
const TTL_THRESHOLD: u32 = 17_280; // ~1 day
const TTL_EXTEND_TO: u32 = 518_400; // ~30 days

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EmrBridgeError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    ProviderNotFound = 4,
    ProviderAlreadyExists = 5,
    ProviderNotActive = 6,
    InvalidMapping = 7,
    ExchangeNotFound = 8,
    ExchangeAlreadyExists = 9,
    SyncFailed = 10,
    InvalidDataFormat = 11,
    MappingAlreadyExists = 12,
    VerificationNotFound = 13,
    VerificationAlreadyExists = 14,
}

#[contract]
pub struct EmrBridgeContract;

#[contractimpl]
impl EmrBridgeContract {
    // ── Initialization ───────────────────────────────────────────────────────

    /// Initialize the EMR bridge contract with an administrator
    pub fn initialize(env: Env, admin: Address) -> Result<(), EmrBridgeError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(EmrBridgeError::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        events::publish_initialized(&env, admin);

        Ok(())
    }

    /// Get the admin address
    pub fn get_admin(env: Env) -> Result<Address, EmrBridgeError> {
        env.storage()
            .instance()
            .get(&ADMIN)
            .ok_or(EmrBridgeError::NotInitialized)
    }

    // ── Provider Onboarding ──────────────────────────────────────────────────

    /// Register a new EMR provider
    pub fn register_provider(
        env: Env,
        caller: Address,
        provider_id: String,
        name: String,
        emr_system: EmrSystem,
        endpoint_url: String,
        data_format: DataFormat,
    ) -> Result<EmrProvider, EmrBridgeError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        let provider_key = (symbol_short!("PROVIDER"), provider_id.clone());
        if env.storage().persistent().has(&provider_key) {
            return Err(EmrBridgeError::ProviderAlreadyExists);
        }

        let provider = EmrProvider {
            provider_id: provider_id.clone(),
            name,
            emr_system,
            endpoint_url,
            data_format,
            status: ProviderStatus::Pending,
            registered_by: caller.clone(),
            registered_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&provider_key, &provider);
        env.storage()
            .persistent()
            .extend_ttl(&provider_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        // Track provider in the list
        let list_key = symbol_short!("PRV_LIST");
        let mut providers: Vec<String> = env
            .storage()
            .persistent()
            .get(&list_key)
            .unwrap_or(Vec::new(&env));
        providers.push_back(provider_id.clone());
        env.storage().persistent().set(&list_key, &providers);
        env.storage()
            .persistent()
            .extend_ttl(&list_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        events::publish_provider_registered(&env, provider_id, caller);

        Ok(provider)
    }

    /// Activate a pending EMR provider
    pub fn activate_provider(
        env: Env,
        caller: Address,
        provider_id: String,
    ) -> Result<(), EmrBridgeError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        let provider_key = (symbol_short!("PROVIDER"), provider_id.clone());
        let mut provider: EmrProvider = env
            .storage()
            .persistent()
            .get(&provider_key)
            .ok_or(EmrBridgeError::ProviderNotFound)?;

        if provider.status == ProviderStatus::Active {
            return Ok(());
        }

        provider.status = ProviderStatus::Active;
        env.storage().persistent().set(&provider_key, &provider);
        env.storage()
            .persistent()
            .extend_ttl(&provider_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        events::publish_provider_status_changed(&env, provider_id, 1); // Active

        Ok(())
    }

    /// Suspend an EMR provider
    pub fn suspend_provider(
        env: Env,
        caller: Address,
        provider_id: String,
    ) -> Result<(), EmrBridgeError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        let provider_key = (symbol_short!("PROVIDER"), provider_id.clone());
        let mut provider: EmrProvider = env
            .storage()
            .persistent()
            .get(&provider_key)
            .ok_or(EmrBridgeError::ProviderNotFound)?;

        if provider.status == ProviderStatus::Suspended {
            return Ok(());
        }

        provider.status = ProviderStatus::Suspended;
        env.storage().persistent().set(&provider_key, &provider);
        env.storage()
            .persistent()
            .extend_ttl(&provider_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        events::publish_provider_status_changed(&env, provider_id, 2); // Suspended

        Ok(())
    }

    /// Get provider details
    pub fn get_provider(env: Env, provider_id: String) -> Result<EmrProvider, EmrBridgeError> {
        let provider_key = (symbol_short!("PROVIDER"), provider_id);
        let provider: EmrProvider = env
            .storage()
            .persistent()
            .get(&provider_key)
            .ok_or(EmrBridgeError::ProviderNotFound)?;
        env.storage()
            .persistent()
            .extend_ttl(&provider_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(provider)
    }

    /// List all registered provider IDs
    pub fn list_providers(env: Env) -> Vec<String> {
        let list_key = symbol_short!("PRV_LIST");
        let providers: Vec<String> = env
            .storage()
            .persistent()
            .get(&list_key)
            .unwrap_or(Vec::new(&env));
        if !providers.is_empty() {
            env.storage()
                .persistent()
                .extend_ttl(&list_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        }
        providers
    }

    // ── Data Exchange Protocols ──────────────────────────────────────────────

    /// Record a data exchange (import or export) with an EMR system
    pub fn record_data_exchange(
        env: Env,
        caller: Address,
        exchange_id: String,
        provider_id: String,
        patient_id: String,
        direction: ExchangeDirection,
        data_format: DataFormat,
        resource_type: String,
        record_hash: String,
    ) -> Result<DataExchangeRecord, EmrBridgeError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        // Verify provider exists and is active
        Self::require_active_provider(&env, &provider_id)?;

        let exchange_key = (symbol_short!("EXCHANGE"), exchange_id.clone());
        if env.storage().persistent().has(&exchange_key) {
            return Err(EmrBridgeError::ExchangeAlreadyExists);
        }

        let record = DataExchangeRecord {
            exchange_id: exchange_id.clone(),
            provider_id: provider_id.clone(),
            patient_id: patient_id.clone(),
            direction,
            data_format,
            resource_type,
            record_hash,
            timestamp: env.ledger().timestamp(),
            status: SyncStatus::Pending,
        };

        env.storage().persistent().set(&exchange_key, &record);
        env.storage()
            .persistent()
            .extend_ttl(&exchange_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        // Track exchanges per patient
        let patient_ex_key = (symbol_short!("PAT_EX"), patient_id.clone());
        let mut patient_exchanges: Vec<String> = env
            .storage()
            .persistent()
            .get(&patient_ex_key)
            .unwrap_or(Vec::new(&env));
        patient_exchanges.push_back(exchange_id.clone());
        env.storage()
            .persistent()
            .set(&patient_ex_key, &patient_exchanges);
        env.storage()
            .persistent()
            .extend_ttl(&patient_ex_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        events::publish_data_exchanged(&env, exchange_id, provider_id);

        Ok(record)
    }

    /// Update the status of a data exchange
    pub fn update_exchange_status(
        env: Env,
        caller: Address,
        exchange_id: String,
        new_status: SyncStatus,
    ) -> Result<(), EmrBridgeError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        let exchange_key = (symbol_short!("EXCHANGE"), exchange_id);
        let mut record: DataExchangeRecord = env
            .storage()
            .persistent()
            .get(&exchange_key)
            .ok_or(EmrBridgeError::ExchangeNotFound)?;

        record.status = new_status;
        env.storage().persistent().set(&exchange_key, &record);
        env.storage()
            .persistent()
            .extend_ttl(&exchange_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        Ok(())
    }

    /// Get a data exchange record by ID
    pub fn get_exchange(
        env: Env,
        exchange_id: String,
    ) -> Result<DataExchangeRecord, EmrBridgeError> {
        let exchange_key = (symbol_short!("EXCHANGE"), exchange_id);
        let record: DataExchangeRecord = env
            .storage()
            .persistent()
            .get(&exchange_key)
            .ok_or(EmrBridgeError::ExchangeNotFound)?;
        env.storage()
            .persistent()
            .extend_ttl(&exchange_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(record)
    }

    /// Get all exchange IDs for a patient
    pub fn get_patient_exchanges(env: Env, patient_id: String) -> Vec<String> {
        let patient_ex_key = (symbol_short!("PAT_EX"), patient_id);
        let exchanges: Vec<String> = env
            .storage()
            .persistent()
            .get(&patient_ex_key)
            .unwrap_or(Vec::new(&env));
        if !exchanges.is_empty() {
            env.storage()
                .persistent()
                .extend_ttl(&patient_ex_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        }
        exchanges
    }

    // ── Data Mapping Utilities ───────────────────────────────────────────────

    /// Create a field mapping between EMR system fields and Teye internal fields
    pub fn create_field_mapping(
        env: Env,
        caller: Address,
        mapping_id: String,
        provider_id: String,
        source_field: String,
        target_field: String,
        transform_rule: String,
    ) -> Result<FieldMapping, EmrBridgeError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        // Verify provider exists
        let provider_key = (symbol_short!("PROVIDER"), provider_id.clone());
        if !env.storage().persistent().has(&provider_key) {
            return Err(EmrBridgeError::ProviderNotFound);
        }

        if source_field.is_empty() || target_field.is_empty() {
            return Err(EmrBridgeError::InvalidMapping);
        }

        let mapping_key = (symbol_short!("MAPPING"), mapping_id.clone());
        if env.storage().persistent().has(&mapping_key) {
            return Err(EmrBridgeError::MappingAlreadyExists);
        }

        let mapping = FieldMapping {
            mapping_id: mapping_id.clone(),
            provider_id: provider_id.clone(),
            source_field,
            target_field,
            transform_rule,
        };

        env.storage().persistent().set(&mapping_key, &mapping);
        env.storage()
            .persistent()
            .extend_ttl(&mapping_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        // Track mappings per provider
        let prv_map_key = (symbol_short!("PRV_MAP"), provider_id.clone());
        let mut provider_mappings: Vec<String> = env
            .storage()
            .persistent()
            .get(&prv_map_key)
            .unwrap_or(Vec::new(&env));
        provider_mappings.push_back(mapping_id.clone());
        env.storage()
            .persistent()
            .set(&prv_map_key, &provider_mappings);
        env.storage()
            .persistent()
            .extend_ttl(&prv_map_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        events::publish_mapping_created(&env, mapping_id, provider_id);

        Ok(mapping)
    }

    /// Get a field mapping by ID
    pub fn get_field_mapping(env: Env, mapping_id: String) -> Result<FieldMapping, EmrBridgeError> {
        let mapping_key = (symbol_short!("MAPPING"), mapping_id);
        let mapping: FieldMapping = env
            .storage()
            .persistent()
            .get(&mapping_key)
            .ok_or(EmrBridgeError::InvalidMapping)?;
        env.storage()
            .persistent()
            .extend_ttl(&mapping_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(mapping)
    }

    /// Get all mapping IDs for a provider
    pub fn get_provider_mappings(env: Env, provider_id: String) -> Vec<String> {
        let prv_map_key = (symbol_short!("PRV_MAP"), provider_id);
        let mappings: Vec<String> = env
            .storage()
            .persistent()
            .get(&prv_map_key)
            .unwrap_or(Vec::new(&env));
        if !mappings.is_empty() {
            env.storage()
                .persistent()
                .extend_ttl(&prv_map_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        }
        mappings
    }

    // ── Sync Verification ────────────────────────────────────────────────────

    /// Verify data consistency between source and target after a sync operation
    pub fn verify_sync(
        env: Env,
        caller: Address,
        verification_id: String,
        exchange_id: String,
        source_hash: String,
        target_hash: String,
        discrepancies: Vec<String>,
    ) -> Result<SyncVerification, EmrBridgeError> {
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        // Verify the exchange exists
        let exchange_key = (symbol_short!("EXCHANGE"), exchange_id.clone());
        if !env.storage().persistent().has(&exchange_key) {
            return Err(EmrBridgeError::ExchangeNotFound);
        }

        // Prevent duplicate verifications
        let verify_key = (symbol_short!("VERIFY"), verification_id.clone());
        if env.storage().persistent().has(&verify_key) {
            return Err(EmrBridgeError::VerificationAlreadyExists);
        }

        let is_consistent = source_hash == target_hash && discrepancies.is_empty();

        let verification = SyncVerification {
            verification_id: verification_id.clone(),
            exchange_id: exchange_id.clone(),
            source_hash,
            target_hash,
            is_consistent,
            verified_at: env.ledger().timestamp(),
            discrepancies,
        };

        env.storage().persistent().set(&verify_key, &verification);
        env.storage()
            .persistent()
            .extend_ttl(&verify_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        // Update exchange status based on verification
        let mut record: DataExchangeRecord = env
            .storage()
            .persistent()
            .get(&exchange_key)
            .ok_or(EmrBridgeError::ExchangeNotFound)?;
        record.status = if is_consistent {
            SyncStatus::Completed
        } else {
            SyncStatus::PartialSuccess
        };
        env.storage().persistent().set(&exchange_key, &record);
        env.storage()
            .persistent()
            .extend_ttl(&exchange_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        events::publish_sync_verified(&env, verification_id, is_consistent);

        Ok(verification)
    }

    /// Get a sync verification record
    pub fn get_verification(
        env: Env,
        verification_id: String,
    ) -> Result<SyncVerification, EmrBridgeError> {
        let verify_key = (symbol_short!("VERIFY"), verification_id);
        let verification: SyncVerification = env
            .storage()
            .persistent()
            .get(&verify_key)
            .ok_or(EmrBridgeError::VerificationNotFound)?;
        env.storage()
            .persistent()
            .extend_ttl(&verify_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(verification)
    }

    // ── Internal Helpers ─────────────────────────────────────────────────────

    fn require_admin(env: &Env, caller: &Address) -> Result<(), EmrBridgeError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(EmrBridgeError::NotInitialized)?;
        if *caller != admin {
            return Err(EmrBridgeError::Unauthorized);
        }
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    fn require_active_provider(env: &Env, provider_id: &String) -> Result<(), EmrBridgeError> {
        let provider_key = (symbol_short!("PROVIDER"), provider_id.clone());
        let provider: EmrProvider = env
            .storage()
            .persistent()
            .get(&provider_key)
            .ok_or(EmrBridgeError::ProviderNotFound)?;
        if provider.status != ProviderStatus::Active {
            return Err(EmrBridgeError::ProviderNotActive);
        }
        Ok(())
    }
}
