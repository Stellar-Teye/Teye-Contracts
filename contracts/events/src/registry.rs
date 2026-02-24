//! Event registry: schema versioning and source contract management.
//!
//! Each topic can have multiple schema versions. Schemas are identified by a
//! hash string (e.g. a content-addressable hash of the schema definition).
//! Versions must be registered in ascending order to enforce forward evolution.

use crate::EventError;
use soroban_sdk::{symbol_short, Address, Env, String};

// ── Storage key helpers ──────────────────────────────────────────────────────

fn schema_key(topic: &String, version: u32) -> (soroban_sdk::Symbol, String, u32) {
    (symbol_short!("SCHEMA"), topic.clone(), version)
}

fn latest_version_key(topic: &String) -> (soroban_sdk::Symbol, String) {
    (symbol_short!("SCH_LAST"), topic.clone())
}

fn source_key(source: &Address) -> (soroban_sdk::Symbol, Address) {
    (symbol_short!("SRC"), source.clone())
}

// ── Schema registration ──────────────────────────────────────────────────────

/// Register a new schema version for a given topic.
///
/// Enforces that versions are registered in strictly ascending order so that
/// consumers can rely on monotonically increasing version numbers.
pub fn register_schema(
    env: &Env,
    topic: &String,
    version: u32,
    schema_hash: &String,
) -> Result<(), EventError> {
    if topic.len() == 0 || schema_hash.len() == 0 {
        return Err(EventError::InvalidInput);
    }

    if version == 0 {
        return Err(EventError::InvalidSchema);
    }

    let latest_key = latest_version_key(topic);
    let current_latest: u32 = env.storage().persistent().get(&latest_key).unwrap_or(0);

    if version <= current_latest {
        return Err(EventError::InvalidSchema);
    }

    let key = schema_key(topic, version);
    env.storage().persistent().set(&key, schema_hash);
    env.storage().persistent().set(&latest_key, &version);

    env.events().publish(
        (symbol_short!("SCH_REG"), topic.clone(), version),
        schema_hash.clone(),
    );

    Ok(())
}

/// Retrieve the schema hash for a specific topic and version.
pub fn get_schema(env: &Env, topic: &String, version: u32) -> Result<String, EventError> {
    let key = schema_key(topic, version);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(EventError::SchemaNotFound)
}

/// Return the latest registered schema version for a topic.
pub fn get_latest_version(env: &Env, topic: &String) -> Result<u32, EventError> {
    let key = latest_version_key(topic);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(EventError::TopicNotFound)
}

/// Validate that a schema exists for the given topic and version.
/// Used before publishing events to guarantee schema compliance.
pub fn require_schema_exists(
    env: &Env,
    topic: &String,
    version: u32,
) -> Result<(), EventError> {
    let key = schema_key(topic, version);
    if env.storage().persistent().has(&key) {
        Ok(())
    } else {
        Err(EventError::SchemaNotFound)
    }
}

// ── Source contract management ───────────────────────────────────────────────

/// Register an address as an authorized event publisher.
pub fn register_source(env: &Env, source: &Address) {
    let key = source_key(source);
    env.storage().persistent().set(&key, &true);

    env.events().publish(
        (symbol_short!("SRC_REG"), source.clone()),
        true,
    );
}

/// Check if an address is an authorized event publisher.
pub fn is_registered_source(env: &Env, source: &Address) -> bool {
    let key = source_key(source);
    env.storage().persistent().get(&key).unwrap_or(false)
}
