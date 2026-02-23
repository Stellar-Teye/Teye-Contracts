#![allow(clippy::arithmetic_side_effects)]
use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Vec};

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

fn extend_ttl(env: &Env, key: &(soroban_sdk::Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_ttl_string_key(env: &Env, key: &(soroban_sdk::Symbol, String)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_ttl_u64_key(env: &Env, key: &(soroban_sdk::Symbol, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_ttl_status_key(env: &Env, key: &(soroban_sdk::Symbol, VerificationStatus)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VerificationStatus {
    Pending = 1,
    Verified = 2,
    Rejected = 3,
    Suspended = 4,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct License {
    pub number: String,
    pub issuing_authority: String,
    pub issued_date: u64,
    pub expiry_date: u64,
    pub license_type: String,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Certification {
    pub name: String,
    pub issuer: String,
    pub issued_date: u64,
    pub expiry_date: u64,
    pub credential_id: String,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Location {
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub zip: String,
    pub country: String,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Provider {
    pub address: Address,
    pub name: String,
    pub licenses: Vec<License>,
    pub specialties: Vec<String>,
    pub certifications: Vec<Certification>,
    pub locations: Vec<Location>,
    pub verification_status: VerificationStatus,
    pub registered_at: u64,
    pub verified_at: Option<u64>,
    pub verified_by: Option<Address>,
    pub is_active: bool,
}

pub fn provider_key(provider: &Address) -> (soroban_sdk::Symbol, Address) {
    (symbol_short!("PROV"), provider.clone())
}

pub fn specialty_index_key(specialty: &String) -> (soroban_sdk::Symbol, String) {
    (symbol_short!("SPEC_IDX"), specialty.clone())
}

pub fn status_index_key(status: &VerificationStatus) -> (soroban_sdk::Symbol, VerificationStatus) {
    (symbol_short!("STAT_IDX"), status.clone())
}

pub fn get_provider(env: &Env, provider: &Address) -> Option<Provider> {
    let key = provider_key(provider);
    env.storage().persistent().get(&key)
}

pub fn set_provider(env: &Env, provider: &Provider) {
    let key = provider_key(&provider.address);

    // Get old provider to update status index if status changed
    let old_provider = get_provider(env, &provider.address);

    env.storage().persistent().set(&key, provider);
    extend_ttl(env, &key);

    // Update status index
    if let Some(old) = old_provider {
        // Remove from old status index if status or active state changed
        if old.verification_status != provider.verification_status
            || old.is_active != provider.is_active
        {
            remove_provider_from_status_index(env, &old.verification_status, &provider.address);
        }
    }

    // Add to new status index if active
    if provider.is_active {
        add_provider_to_status_index(env, &provider.verification_status, &provider.address);
    } else {
        // Remove from status index if provider is inactive
        remove_provider_from_status_index(env, &provider.verification_status, &provider.address);
    }
}

pub fn add_provider_to_specialty_index(env: &Env, specialty: &String, provider: &Address) {
    let key = specialty_index_key(specialty);
    let mut providers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));
    if !providers.contains(provider) {
        providers.push_back(provider.clone());
    }
    env.storage().persistent().set(&key, &providers);
    extend_ttl_string_key(env, &key);
}

pub fn remove_provider_from_specialty_index(env: &Env, specialty: &String, provider: &Address) {
    let key = specialty_index_key(specialty);
    if let Some(providers) = env.storage().persistent().get::<_, Vec<Address>>(&key) {
        let mut new_providers = Vec::new(env);
        for i in 0..providers.len() {
            if let Some(p) = providers.get(i) {
                if p != *provider {
                    new_providers.push_back(p);
                }
            }
        }
        if !new_providers.is_empty() {
            env.storage().persistent().set(&key, &new_providers);
            extend_ttl_string_key(env, &key);
        } else {
            env.storage().persistent().remove(&key);
        }
    }
}

pub fn get_providers_by_specialty(env: &Env, specialty: &String) -> Vec<Address> {
    let key = specialty_index_key(specialty);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env))
}

pub fn add_provider_to_status_index(env: &Env, status: &VerificationStatus, provider: &Address) {
    let key = status_index_key(status);
    let mut providers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));
    if !providers.contains(provider) {
        providers.push_back(provider.clone());
    }
    env.storage().persistent().set(&key, &providers);
    extend_ttl_status_key(env, &key);
}

pub fn remove_provider_from_status_index(
    env: &Env,
    status: &VerificationStatus,
    provider: &Address,
) {
    let key = status_index_key(status);
    if let Some(providers) = env.storage().persistent().get::<_, Vec<Address>>(&key) {
        let mut new_providers = Vec::new(env);
        for i in 0..providers.len() {
            if let Some(p) = providers.get(i) {
                if p != *provider {
                    new_providers.push_back(p);
                }
            }
        }
        if !new_providers.is_empty() {
            env.storage().persistent().set(&key, &new_providers);
            extend_ttl_status_key(env, &key);
        } else {
            env.storage().persistent().remove(&key);
        }
    }
}

pub fn get_providers_by_status(env: &Env, status: &VerificationStatus) -> Vec<Address> {
    let key = status_index_key(status);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env))
}

pub fn get_provider_counter(env: &Env) -> u64 {
    let counter_key = symbol_short!("PROV_CTR");
    env.storage().instance().get(&counter_key).unwrap_or(0)
}

pub fn increment_provider_counter(env: &Env) -> u64 {
    let counter_key = symbol_short!("PROV_CTR");
    let count = get_provider_counter(env) + 1;
    env.storage().instance().set(&counter_key, &count);
    count
}

pub fn get_all_provider_ids(env: &Env) -> Vec<u64> {
    let ids_key = symbol_short!("PROV_IDS");
    env.storage()
        .instance()
        .get(&ids_key)
        .unwrap_or(Vec::new(env))
}

pub fn add_provider_id(env: &Env, provider_id: u64, provider: &Address) {
    let ids_key = symbol_short!("PROV_IDS");
    let mut ids: Vec<u64> = get_all_provider_ids(env);
    if !ids.contains(provider_id) {
        ids.push_back(provider_id);
        env.storage().instance().set(&ids_key, &ids);
    }
    let id_key = (symbol_short!("PROV_ID"), provider_id);
    env.storage().persistent().set(&id_key, provider);
    extend_ttl_u64_key(env, &id_key);
}
