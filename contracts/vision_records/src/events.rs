use crate::errors::ErrorContext;
use crate::{AccessLevel, RecordType, Role, VerificationStatus};
use soroban_sdk::{symbol_short, Address, Env, String};

/// Event published when the contract is initialized.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializedEvent {
    pub admin: Address,
    pub timestamp: u64,
}

/// Event published when a new user is registered.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserRegisteredEvent {
    pub user: Address,
    pub role: Role,
    pub name: String,
    pub timestamp: u64,
}

/// Event published when a new vision record is added.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordAddedEvent {
    pub record_id: u64,
    pub patient: Address,
    pub provider: Address,
    pub record_type: RecordType,
    pub timestamp: u64,
}

/// Event published when access is granted to a record.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessGrantedEvent {
    pub patient: Address,
    pub grantee: Address,
    pub level: AccessLevel,
    pub duration_seconds: u64,
    pub expires_at: u64,
    pub timestamp: u64,
}

/// Event published when access is revoked.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRevokedEvent {
    pub patient: Address,
    pub grantee: Address,
    pub timestamp: u64,
}

/// Event published when a batch of records is added.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchRecordsAddedEvent {
    pub provider: Address,
    pub count: u32,
    pub timestamp: u64,
}

/// Event published when a batch of access grants is made.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchAccessGrantedEvent {
    pub patient: Address,
    pub count: u32,
    pub timestamp: u64,
}

pub fn publish_initialized(env: &Env, admin: Address) {
    let topics = (symbol_short!("INIT"),);
    let data = InitializedEvent {
        admin,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when a new user is registered.
/// This event includes the user address, role, name, and registration timestamp.
pub fn publish_user_registered(env: &Env, user: Address, role: Role, name: String) {
    let topics = (symbol_short!("USR_REG"), user.clone());
    let data = UserRegisteredEvent {
        user,
        role,
        name,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when a new vision record is added.
/// This event includes the record ID, patient, provider, record type, and timestamp.
pub fn publish_record_added(
    env: &Env,
    record_id: u64,
    patient: Address,
    provider: Address,
    record_type: RecordType,
) {
    let topics = (symbol_short!("REC_ADD"), patient.clone(), provider.clone());
    let data = RecordAddedEvent {
        record_id,
        patient,
        provider,
        record_type,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when access is granted to a record.
/// This event includes patient, grantee, access level, duration, expiration, and timestamp.
pub fn publish_access_granted(
    env: &Env,
    patient: Address,
    grantee: Address,
    level: AccessLevel,
    duration_seconds: u64,
    expires_at: u64,
) {
    let topics = (symbol_short!("ACC_GRT"), patient.clone(), grantee.clone());
    let data = AccessGrantedEvent {
        patient,
        grantee,
        level,
        duration_seconds,
        expires_at,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when access to a record is revoked.
/// This event includes the patient, grantee, and revocation timestamp.
pub fn publish_access_revoked(env: &Env, patient: Address, grantee: Address) {
    let topics = (symbol_short!("ACC_REV"), patient.clone(), grantee.clone());
    let data = AccessRevokedEvent {
        patient,
        grantee,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

pub fn publish_access_expired(
    env: &Env,
    patient: Address,
    grantee: Address,
    expired_at: u64,
) {
    let topics = (symbol_short!("ACC_EXP"), patient.clone(), grantee.clone());
    let data = AccessExpiredEvent {
        patient,
        grantee,
        expired_at,
        purged_at: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}
/// Event published when a new provider is registered.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRegisteredEvent {
    pub provider: Address,
    pub name: String,
    pub provider_id: u64,
    pub timestamp: u64,
}

/// Event published when an eye examination is added to a record.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExaminationAddedEvent {
    pub record_id: u64,
    pub timestamp: u64,
}

/// Event published when a provider's verification status is updated.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderVerifiedEvent {
    pub provider: Address,
    pub verifier: Address,
    pub status: VerificationStatus,
    pub timestamp: u64,
}

/// Event published when provider information is updated.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderUpdatedEvent {
    pub provider: Address,
    pub timestamp: u64,
}

/// Publishes an event when a new provider is registered.
/// This event includes the provider address, name, provider ID, and registration timestamp.
pub fn publish_provider_registered(env: &Env, provider: Address, name: String, provider_id: u64) {
    let topics = (symbol_short!("PROV_REG"), provider.clone());
    let data = ProviderRegisteredEvent {
        provider,
        name,
        provider_id,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when a provider's verification status is updated.
/// This event includes the provider, verifier, new status, and verification timestamp.
pub fn publish_provider_verified(
    env: &Env,
    provider: Address,
    verifier: Address,
    status: VerificationStatus,
) {
    let topics = (
        symbol_short!("PROV_VER"),
        provider.clone(),
        verifier.clone(),
    );
    let data = ProviderVerifiedEvent {
        provider,
        count,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

pub fn publish_batch_access_granted(env: &Env, patient: Address, count: u32) {
    let topics = (symbol_short!("BATCH_A"), patient.clone());
    let data = BatchAccessGrantedEvent {
        patient,
        count,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when an examination is added.
/// This event includes the record ID.
pub fn publish_examination_added(env: &Env, record_id: u64) {
    let topics = (symbol_short!("EXAM_ADD"), record_id);
    let data = ExaminationAddedEvent {
        record_id,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Event published when access is granted via meta-transaction.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetaAccessGrantedEvent {
    pub patient: Address,
    pub grantee: Address,
    pub level: AccessLevel,
    pub relayer: Address,
    pub expires_at: u64,
    pub nonce: u64,
    pub timestamp: u64,
}

/// Publishes an event when access is granted via meta-transaction.
pub fn publish_meta_access_granted(
    env: &Env,
    patient: Address,
    grantee: Address,
    level: AccessLevel,
    relayer: Address,
    expires_at: u64,
    nonce: u64,
) {
    let topics = (symbol_short!("META_GRT"), patient.clone(), grantee.clone());
    let data = MetaAccessGrantedEvent {
        patient,
        grantee,
        level,
        relayer,
        expires_at,
        nonce,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an error event for monitoring and indexing.
/// This event includes error code, category, severity, message, user, resource ID, retryable flag, and timestamp.
pub fn publish_error(env: &Env, error_code: u32, context: ErrorContext) {
    let topics = (
        symbol_short!("ERROR"),
        context.category.clone(),
        context.severity.clone(),
    );
    let data = (
        error_code,
        context.category,
        context.severity,
        context.message,
        context.user,
        context.resource_id,
        context.retryable,
        context.timestamp,
    );
    env.events().publish(topics, data);
}
