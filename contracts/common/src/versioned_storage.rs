
use soroban_sdk::{Bytes, Env, Map, Symbol};

use crate::migration::{
    lazy_read, lazy_write, set_stored_version, stored_version,
    MigrationError, SchemaVersion, CURRENT_VERSION,
};

// ─────────────────────────────────────────────────────────────
// Storage key conventions
// ─────────────────────────────────────────────────────────────

fn record_key(env: &Env, record_id: u64) -> (Symbol, u64) {
    (Symbol::new(env, "RECORD"), record_id)
}

fn version_key(env: &Env, record_id: u64) -> (Symbol, u64) {
    (Symbol::new(env, "VERSION"), record_id)
}

// ─────────────────────────────────────────────────────────────
// Versioned record envelope
// ─────────────────────────────────────────────────────────────

pub struct VersionedRecord {
    pub data:    Map<Symbol, Bytes>,
    pub version: SchemaVersion,
}

// ─────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────

pub fn write_record(
    env:       &Env,
    record_id: u64,
    mut data:  Map<Symbol, Bytes>,
    from_ver:  SchemaVersion,
) -> Result<(), MigrationError> {
    let migrated_ver = lazy_write(env, &mut data, from_ver)?;

    let rk = record_key(env, record_id);
    let vk = version_key(env, record_id);

    env.storage().persistent().set(&rk, &data);
    env.storage().persistent().set(&vk, &migrated_ver);

    Ok(())
}

pub fn read_record(
    env:       &Env,
    record_id: u64,
) -> Result<Option<VersionedRecord>, MigrationError> {
    let rk = record_key(env, record_id);
    let vk = version_key(env, record_id);

    let maybe_data: Option<Map<Symbol, Bytes>> =
        env.storage().persistent().get(&rk);

    let data = match maybe_data {
        None        => return Ok(None),
        Some(d)     => d,
    };

    let record_ver: SchemaVersion = env
        .storage()
        .persistent()
        .get(&vk)
        .unwrap_or(1u32);

    let mut migrated = data;
    let new_ver = lazy_read(env, &mut migrated, record_ver)?;

    Ok(Some(VersionedRecord {
        data:    migrated,
        version: new_ver,
    }))
}

pub fn delete_record(env: &Env, record_id: u64) {
    let rk = record_key(env, record_id);
    let vk = version_key(env, record_id);
    env.storage().persistent().remove(&rk);
    env.storage().persistent().remove(&vk);
}

pub fn bump_global_version(env: &Env) {
    let next = stored_version(env) + 1;
    if next <= CURRENT_VERSION {
        set_stored_version(env, next);
    }
}

pub fn global_version(env: &Env) -> SchemaVersion {
    stored_version(env)
}