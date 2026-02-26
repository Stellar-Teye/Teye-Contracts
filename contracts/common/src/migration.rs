use soroban_sdk::xdr::ToXdr;
extern crate alloc;
use soroban_sdk::{
    contracterror,
    contracttype,
    symbol_short,
    Address,
    Bytes,
    Env,
    Map,
    String,
    Symbol,
    Vec,
};

// ─────────────────────────────────────────────────────────────
// Error types
// ─────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MigrationError {
    VersionTooNew = 1,
    NoMigrationPath = 2,
    ValidationFailed = 3,
    AlreadyMigrated = 4,
    RollbackUnavailable = 5,
    BulkNotAllowedInLazyMode = 6,
    TransformFailed = 7,
    InvalidCanaryPercentage = 8,
}

// ─────────────────────────────────────────────────────────────
// Core version types
// ─────────────────────────────────────────────────────────────

pub type SchemaVersion = u32;
pub const CURRENT_VERSION: SchemaVersion = 3;
pub const MINIMUM_SUPPORTED_VERSION: SchemaVersion = 1;

// ─────────────────────────────────────────────────────────────
// Migration DSL — declarative field transformations
// ─────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub enum FieldTransform {
    RenameField(Symbol, Symbol), // old_key, new_key
    AddField(Symbol, Bytes), // key, default_value
    RemoveField(Symbol), // key
    ChangeType(Symbol, Symbol), // key, transform_name
    CopyField(Symbol, Symbol), // source_key, dest_key
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Migration {
    pub from_version: SchemaVersion,
    pub to_version: SchemaVersion,
    pub forward: Vec<FieldTransform>,
    pub reverse: Vec<FieldTransform>,
    pub description: String,
}

// ─────────────────────────────────────────────────────────────
// Migration Registry
// ─────────────────────────────────────────────────────────────

const REGISTRY_KEY: Symbol = symbol_short!("MREGISTRY");

pub fn load_registry(env: &Env) -> Vec<Migration> {
    env.storage()
        .instance()
        .get(&REGISTRY_KEY)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn save_registry(env: &Env, registry: &Vec<Migration>) {
    env.storage().instance().set(&REGISTRY_KEY, registry);
}

pub fn register_migration(env: &Env, migration: Migration) -> Result<(), MigrationError> {
    let mut registry = load_registry(env);
    for existing in registry.iter() {
        if existing.from_version == migration.from_version {
            return Err(MigrationError::AlreadyMigrated);
        }
    }
    registry.push_back(migration);
    save_registry(env, &registry);
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// Schema version tracking
// ─────────────────────────────────────────────────────────────

const VERSION_KEY: Symbol = symbol_short!("SCHEMA_V");

pub fn stored_version(env: &Env) -> SchemaVersion {
    env.storage().instance().get(&VERSION_KEY).unwrap_or(0u32)
}

pub fn set_stored_version(env: &Env, version: SchemaVersion) {
    env.storage().instance().set(&VERSION_KEY, &version);
}

// ─────────────────────────────────────────────────────────────
// Lazy Migration Logic (FIXED: Missing functions added)
// ─────────────────────────────────────────────────────────────

/// Migrates a record forward from its current disk version to the latest logic version.
pub fn lazy_read(
    env: &Env,
    record: &mut Map<Symbol, Bytes>,
    record_ver: SchemaVersion,
) -> Result<SchemaVersion, MigrationError> {
    if record_ver >= CURRENT_VERSION {
        return Ok(record_ver);
    }
    // Automatically bring the old data up to the current schema
    migrate_forward(env, record, record_ver, CURRENT_VERSION)
}

/// Ensures data is at the target version before being committed to persistent storage.
pub fn lazy_write(
    env: &Env,
    record: &mut Map<Symbol, Bytes>,
    from_ver: SchemaVersion,
) -> Result<SchemaVersion, MigrationError> {
    // If we are writing, we typically want the data to be in the latest schema
    migrate_forward(env, record, from_ver, CURRENT_VERSION)
}

// ─────────────────────────────────────────────────────────────
// Forward migration
// ─────────────────────────────────────────────────────────────

pub fn migrate_forward(
    env: &Env,
    record: &mut Map<Symbol, Bytes>,
    current_ver: SchemaVersion,
    target_ver: SchemaVersion
) -> Result<SchemaVersion, MigrationError> {
    if current_ver >= target_ver {
        return Ok(current_ver);
    }
    if target_ver > CURRENT_VERSION {
        return Err(MigrationError::VersionTooNew);
    }

    let registry = load_registry(env);
    let mut ver = current_ver;

    while ver < target_ver {
        let step = find_migration(&registry, ver, ver + 1).ok_or(MigrationError::NoMigrationPath)?;
        apply_transforms(env, record, &step.forward)?;
        ver += 1;
    }

    Ok(ver)
}

// ─────────────────────────────────────────────────────────────
// Rollback
// ─────────────────────────────────────────────────────────────

pub fn migrate_rollback(
    env: &Env,
    record: &mut Map<Symbol, Bytes>,
    current_ver: SchemaVersion,
    target_ver: SchemaVersion
) -> Result<SchemaVersion, MigrationError> {
    if current_ver <= target_ver {
        return Ok(current_ver);
    }
    if target_ver < MINIMUM_SUPPORTED_VERSION {
        return Err(MigrationError::RollbackUnavailable);
    }

    let registry = load_registry(env);
    let mut steps: Vec<Migration> = Vec::new(env);
    let mut ver = current_ver;

    while ver > target_ver {
        let step = find_migration(&registry, ver - 1, ver).ok_or(
            MigrationError::RollbackUnavailable
        )?;
        steps.push_back(step);
        ver -= 1;
    }

    // Validation step
    {
        let mut snapshot = record.clone();
        for step in steps.iter() {
            apply_transforms(env, &mut snapshot, &step.reverse)?;
        }
        validate_record(env, &snapshot, target_ver)?;
    }

    for step in steps.iter() {
        apply_transforms(env, record, &step.reverse)?;
    }

    Ok(target_ver)
}

// ─────────────────────────────────────────────────────────────
// Canary deployments
// ─────────────────────────────────────────────────────────────

const CANARY_PCT_KEY: Symbol = symbol_short!("CANARY_P");
const CANARY_VER_KEY: Symbol = symbol_short!("CANARY_V");

pub fn set_canary(
    env: &Env,
    percentage: u32,
    new_version: SchemaVersion
) -> Result<(), MigrationError> {
    if percentage > 100 {
        return Err(MigrationError::InvalidCanaryPercentage);
    }
    env.storage().instance().set(&CANARY_PCT_KEY, &percentage);
    env.storage().instance().set(&CANARY_VER_KEY, &new_version);
    Ok(())
}

pub fn resolve_version_for_caller(env: &Env, caller: &Address) -> SchemaVersion {
    let pct: u32 = env.storage().instance().get(&CANARY_PCT_KEY).unwrap_or(0u32);

    if pct == 0 {
        return stored_version(env);
    }

    let canary_ver: SchemaVersion = env
        .storage()
        .instance()
        .get(&CANARY_VER_KEY)
        .unwrap_or(stored_version(env));

    // FIX: Using hash of Address bytes instead of to_string()
    // Using .clone() because to_xdr takes ownership
    let caller_bytes = caller.clone().to_xdr(env);
    let hash = env.crypto().sha256(&caller_bytes);

    let hash_bytes = soroban_sdk::Bytes::from_array(env, &hash.into());
    let first_byte = hash_bytes.get(0).unwrap_or(0);
    let bucket = (first_byte as u32) % 100;

    if bucket < pct {
        canary_ver
    } else {
        stored_version(env)
    }
}

// ─────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────

fn find_migration(
    registry: &Vec<Migration>,
    from: SchemaVersion,
    to: SchemaVersion
) -> Option<Migration> {
    registry.iter().find(|m| m.from_version == from && m.to_version == to)
}

fn apply_transforms(
    _env: &Env,
    record: &mut Map<Symbol, Bytes>,
    transforms: &Vec<FieldTransform>
) -> Result<(), MigrationError> {
    for transform in transforms.iter() {
        match transform {
            FieldTransform::RenameField(old_key, new_key) => {
                if let Some(value) = record.get(old_key.clone()) {
                    record.remove(old_key);
                    record.set(new_key, value);
                }
            }
            FieldTransform::AddField(key, default_value) => {
                if !record.contains_key(key.clone()) {
                    record.set(key, default_value);
                }
            }
            FieldTransform::RemoveField(key) => {
                record.remove(key);
            }
            FieldTransform::ChangeType(key, _) => {
                if !record.contains_key(key.clone()) {
                    return Err(MigrationError::TransformFailed);
                }
            }
            FieldTransform::CopyField(source_key, dest_key) => {
                if let Some(value) = record.get(source_key.clone()) {
                    record.set(dest_key, value);
                }
            }
        }
    }
    Ok(())
}

fn validate_record(
    env: &Env,
    record: &Map<Symbol, Bytes>,
    version: SchemaVersion
) -> Result<(), MigrationError> {
    let required: &[&str] = match version {
        1 => &["patient_id", "exam_date"],
        2 => &["patient_id", "exam_date", "iop_value"],
        3 => &["patient_id", "exam_date", "iop_value", "ai_flag"],
        _ => &[],
    };

    for &field in required {
        let key = Symbol::new(env, field);
        if !record.contains_key(key) {
            return Err(MigrationError::ValidationFailed);
        }
    }
    Ok(())
}

pub fn initialize_default_migrations(env: &Env) {
    let m1 = Migration {
        from_version: 1,
        to_version: 2,
        description: String::from_str(env, "Add iop_value field"),
        forward: {
            let mut v = Vec::new(env);
            v.push_back(
                FieldTransform::AddField(
                    Symbol::new(env, "iop_value"),
                    Bytes::from_slice(env, b"0")
                )
            );
            v
        },
        reverse: {
            let mut v = Vec::new(env);
            v.push_back(FieldTransform::RemoveField(Symbol::new(env, "iop_value")));
            v
        },
    };

    let m2 = Migration {
        from_version: 2,
        to_version: 3,
        description: String::from_str(env, "AI flag migration"),
        forward: {
            let mut v = Vec::new(env);
            v.push_back(
                FieldTransform::RenameField(
                    Symbol::new(env, "raw_notes"),
                    Symbol::new(env, "clinical_notes")
                )
            );
            v.push_back(
                FieldTransform::AddField(
                    Symbol::new(env, "ai_flag"),
                    Bytes::from_slice(env, b"false")
                )
            );
            v
        },
        reverse: {
            let mut v = Vec::new(env);
            v.push_back(
                FieldTransform::RenameField(
                    Symbol::new(env, "clinical_notes"),
                    Symbol::new(env, "raw_notes")
                )
            );
            v.push_back(FieldTransform::RemoveField(Symbol::new(env, "ai_flag")));
            v
        },
    };

    let mut registry = load_registry(env);
    registry.push_back(m1);
    registry.push_back(m2);
    save_registry(env, &registry);
    set_stored_version(env, 1);
}