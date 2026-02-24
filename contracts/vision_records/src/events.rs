use crate::appointment::AppointmentType;
use crate::audit::{AccessAction, AccessResult, AuditEntry};
use crate::circuit_breaker::PauseScope;
use crate::emergency::EmergencyCondition;
use crate::errors::{ErrorCategory, ErrorContext, ErrorSeverity};
use crate::{AccessLevel, RecordType, Role, VerificationStatus};
use soroban_sdk::{symbol_short, Address, Env, String};

/// Event published when the contract is initialized.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializedEvent {
    pub admin: Address,
    pub timestamp: u64,
}

/// Event published when an admin transfer is proposed.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferProposedEvent {
    pub current_admin: Address,
    pub proposed_admin: Address,
    pub timestamp: u64,
}

/// Event published when an admin transfer is accepted.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferAcceptedEvent {
    pub old_admin: Address,
    pub new_admin: Address,
    pub timestamp: u64,
}

/// Event published when a pending admin transfer is cancelled.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferCancelledEvent {
    pub admin: Address,
    pub cancelled_proposed: Address,
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

/// Event published when access is granted to a specific record.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordAccessGrantedEvent {
    pub patient: Address,
    pub grantee: Address,
    pub record_id: u64,
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

/// Event published when an expired access grant is purged.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessExpiredEvent {
    pub patient: Address,
    pub grantee: Address,
    pub expired_at: u64,
    pub purged_at: u64,
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

/// Event published when circuit breaker is enabled.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractPausedEvent {
    pub caller: Address,
    pub scope: PauseScope,
    pub timestamp: u64,
}

/// Event published when circuit breaker is disabled.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractResumedEvent {
    pub caller: Address,
    pub scope: PauseScope,
    pub timestamp: u64,
}

pub fn publish_admin_transfer_proposed(env: &Env, current_admin: Address, proposed_admin: Address) {
    let topics = (symbol_short!("ADM_PROP"), current_admin.clone());
    let data = AdminTransferProposedEvent {
        current_admin,
        proposed_admin,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

pub fn publish_admin_transfer_accepted(env: &Env, old_admin: Address, new_admin: Address) {
    let topics = (symbol_short!("ADM_ACPT"), new_admin.clone());
    let data = AdminTransferAcceptedEvent {
        old_admin,
        new_admin,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

pub fn publish_admin_transfer_cancelled(env: &Env, admin: Address, cancelled_proposed: Address) {
    let topics = (symbol_short!("ADM_CNCL"), admin.clone());
    let data = AdminTransferCancelledEvent {
        admin,
        cancelled_proposed,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
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

pub fn publish_record_access_granted(
    env: &Env,
    patient: Address,
    grantee: Address,
    record_id: u64,
    level: AccessLevel,
    duration_seconds: u64,
    expires_at: u64,
) {
    let topics = (
        symbol_short!("REC_GRT"),
        patient.clone(),
        grantee.clone(),
        record_id,
    );
    let data = RecordAccessGrantedEvent {
        patient,
        grantee,
        record_id,
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

pub fn publish_batch_records_added(env: &Env, provider: Address, count: u32) {
    let topics = (symbol_short!("BATCH_R"), provider.clone());
    let data = BatchRecordsAddedEvent {
        provider,
        count,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

pub fn publish_contract_paused(env: &Env, caller: Address, scope: PauseScope) {
    let topics = (symbol_short!("PAUSE"),);
    let data = ContractPausedEvent {
        caller,
        scope,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

pub fn publish_contract_resumed(env: &Env, caller: Address, scope: PauseScope) {
    let topics = (symbol_short!("RESUME"),);
    let data = ContractResumedEvent {
        caller,
        scope,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

pub fn publish_access_expired(env: &Env, patient: Address, grantee: Address, expired_at: u64) {
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
        verifier,
        status,
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

/// Event published when consent is granted by a patient.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentGrantedEvent {
    pub patient: Address,
    pub grantee: Address,
    pub consent_type: crate::ConsentType,
    pub expires_at: u64,
    pub timestamp: u64,
}

/// Event published when consent is revoked by a patient.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentRevokedEvent {
    pub patient: Address,
    pub grantee: Address,
    pub timestamp: u64,
}

/// Publishes an event when consent is granted.
pub fn publish_consent_granted(
    env: &Env,
    patient: Address,
    grantee: Address,
    consent_type: crate::ConsentType,
    expires_at: u64,
) {
    let topics = (symbol_short!("CST_GRT"), patient.clone(), grantee.clone());
    let data = ConsentGrantedEvent {
        patient,
        grantee,
        consent_type,
        expires_at,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when consent is revoked.
pub fn publish_consent_revoked(env: &Env, patient: Address, grantee: Address) {
    let topics = (symbol_short!("CST_REV"), patient.clone(), grantee.clone());
    let data = ConsentRevokedEvent {
        patient,
        grantee,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Event published when a patient profile is created.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileCreatedEvent {
    pub patient: Address,
    pub timestamp: u64,
}

/// Event published when a patient profile is updated.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileUpdatedEvent {
    pub patient: Address,
    pub timestamp: u64,
}

pub fn publish_profile_created(env: &Env, patient: Address) {
    let topics = (symbol_short!("PROF_C"), patient.clone());
    let data = ProfileCreatedEvent {
        patient,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

pub fn publish_profile_updated(env: &Env, patient: Address) {
    let topics = (symbol_short!("PROF_U"), patient.clone());
    let data = ProfileUpdatedEvent {
        patient,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Event published when an error occurs.
/// This event includes error code, category, severity, message, user, resource ID, retryable flag, and timestamp.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorEvent {
    pub error_code: u32,
    pub category: ErrorCategory,
    pub severity: ErrorSeverity,
    pub message: String,
    pub user: Option<Address>,
    pub resource_id: Option<String>,
    pub retryable: bool,
    pub timestamp: u64,
}

/// Publishes an error event for monitoring and indexing.
/// This event includes error code, category, severity, message, user, resource ID, retryable flag, and timestamp.
pub fn publish_error(env: &Env, error_code: u32, context: ErrorContext) {
    let topics = (
        symbol_short!("ERROR"),
        context.category.clone(),
        context.severity.clone(),
    );
    let data = ErrorEvent {
        error_code,
        category: context.category,
        severity: context.severity,
        message: context.message,
        user: context.user,
        resource_id: context.resource_id,
        retryable: context.retryable,
        timestamp: context.timestamp,
    };
    env.events().publish(topics, data);
}

/// Event published when emergency access is granted.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyAccessGrantedEvent {
    pub access_id: u64,
    pub patient: Address,
    pub requester: Address,
    pub condition: EmergencyCondition,
    pub expires_at: u64,
    pub timestamp: u64,
}

/// Event published when emergency access is revoked.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyAccessRevokedEvent {
    pub access_id: u64,
    pub patient: Address,
    pub revoker: Address,
    pub timestamp: u64,
}

/// Event published when emergency contacts are notified.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyContactNotifiedEvent {
    pub access_id: u64,
    pub patient: Address,
    pub contact: Address,
    pub timestamp: u64,
}

/// Event published when emergency access is used to access records.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyAccessUsedEvent {
    pub access_id: u64,
    pub patient: Address,
    pub requester: Address,
    pub record_id: Option<u64>,
    pub timestamp: u64,
}

/// Publishes an event when emergency access is granted.
pub fn publish_emergency_access_granted(
    env: &Env,
    access_id: u64,
    patient: Address,
    requester: Address,
    condition: EmergencyCondition,
    expires_at: u64,
) {
    let topics = (
        symbol_short!("EMRG_GRT"),
        patient.clone(),
        requester.clone(),
    );
    let data = EmergencyAccessGrantedEvent {
        access_id,
        patient,
        requester,
        condition,
        expires_at,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when emergency access is revoked.
pub fn publish_emergency_access_revoked(
    env: &Env,
    access_id: u64,
    patient: Address,
    revoker: Address,
) {
    let topics = (symbol_short!("EMRG_REV"), patient.clone(), revoker.clone());
    let data = EmergencyAccessRevokedEvent {
        access_id,
        patient,
        revoker,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when an emergency contact is notified.
pub fn publish_emergency_contact_notified(
    env: &Env,
    access_id: u64,
    patient: Address,
    contact: Address,
) {
    let topics = (symbol_short!("EMRG_NOT"), patient.clone(), contact.clone());
    let data = EmergencyContactNotifiedEvent {
        access_id,
        patient,
        contact,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when emergency access is used to access records.
pub fn publish_emergency_access_used(
    env: &Env,
    access_id: u64,
    patient: Address,
    requester: Address,
    record_id: Option<u64>,
) {
    let topics = (
        symbol_short!("EMRG_USE"),
        patient.clone(),
        requester.clone(),
    );
    let data = EmergencyAccessUsedEvent {
        access_id,
        patient,
        requester,
        record_id,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Event published when an appointment is created/scheduled.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppointmentScheduledEvent {
    pub appointment_id: u64,
    pub patient: Address,
    pub provider: Address,
    pub appointment_type: AppointmentType,
    pub scheduled_at: u64,
    pub timestamp: u64,
}

/// Event published when an appointment is confirmed.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppointmentConfirmedEvent {
    pub appointment_id: u64,
    pub patient: Address,
    pub provider: Address,
    pub confirmed_by: Address,
    pub timestamp: u64,
}

/// Event published when an appointment is cancelled.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppointmentCancelledEvent {
    pub appointment_id: u64,
    pub patient: Address,
    pub provider: Address,
    pub cancelled_by: Address,
    pub timestamp: u64,
}

/// Event published when an appointment is rescheduled.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppointmentRescheduledEvent {
    pub appointment_id: u64,
    pub patient: Address,
    pub provider: Address,
    pub old_scheduled_at: u64,
    pub new_scheduled_at: u64,
    pub rescheduled_by: Address,
    pub timestamp: u64,
}

/// Event published when an appointment is completed.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppointmentCompletedEvent {
    pub appointment_id: u64,
    pub patient: Address,
    pub provider: Address,
    pub completed_by: Address,
    pub timestamp: u64,
}

/// Event published when an appointment reminder is sent.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppointmentReminderEvent {
    pub appointment_id: u64,
    pub patient: Address,
    pub provider: Address,
    pub scheduled_at: u64,
    pub timestamp: u64,
}

/// Event published when an appointment is verified.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppointmentVerifiedEvent {
    pub appointment_id: u64,
    pub patient: Address,
    pub provider: Address,
    pub verifier: Address,
    pub timestamp: u64,
}

/// Publishes an event when an appointment is scheduled.
pub fn publish_appointment_scheduled(
    env: &Env,
    appointment_id: u64,
    patient: Address,
    provider: Address,
    appointment_type: AppointmentType,
    scheduled_at: u64,
) {
    let topics = (symbol_short!("APPT_SCH"), patient.clone(), provider.clone());
    let data = AppointmentScheduledEvent {
        appointment_id,
        patient,
        provider,
        appointment_type,
        scheduled_at,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when an appointment is confirmed.
pub fn publish_appointment_confirmed(
    env: &Env,
    appointment_id: u64,
    patient: Address,
    provider: Address,
    confirmed_by: Address,
) {
    let topics = (symbol_short!("APPT_CFM"), patient.clone(), provider.clone());
    let data = AppointmentConfirmedEvent {
        appointment_id,
        patient,
        provider,
        confirmed_by,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when an appointment is cancelled.
pub fn publish_appointment_cancelled(
    env: &Env,
    appointment_id: u64,
    patient: Address,
    provider: Address,
    cancelled_by: Address,
) {
    let topics = (symbol_short!("APPT_CNL"), patient.clone(), provider.clone());
    let data = AppointmentCancelledEvent {
        appointment_id,
        patient,
        provider,
        cancelled_by,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when an appointment is rescheduled.
pub fn publish_appointment_rescheduled(
    env: &Env,
    appointment_id: u64,
    patient: Address,
    provider: Address,
    old_scheduled_at: u64,
    new_scheduled_at: u64,
    rescheduled_by: Address,
) {
    let topics = (
        symbol_short!("APPT_RSCH"),
        patient.clone(),
        provider.clone(),
    );
    let data = AppointmentRescheduledEvent {
        appointment_id,
        patient,
        provider,
        old_scheduled_at,
        new_scheduled_at,
        rescheduled_by,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when an appointment is completed.
pub fn publish_appointment_completed(
    env: &Env,
    appointment_id: u64,
    patient: Address,
    provider: Address,
    completed_by: Address,
) {
    let topics = (symbol_short!("APPT_CMP"), patient.clone(), provider.clone());
    let data = AppointmentCompletedEvent {
        appointment_id,
        patient,
        provider,
        completed_by,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when an appointment reminder is sent.
pub fn publish_appointment_reminder(
    env: &Env,
    appointment_id: u64,
    patient: Address,
    provider: Address,
    scheduled_at: u64,
) {
    let topics = (symbol_short!("APPT_RMD"), patient.clone(), provider.clone());
    let data = AppointmentReminderEvent {
        appointment_id,
        patient,
        provider,
        scheduled_at,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when an appointment is verified.
pub fn publish_appointment_verified(
    env: &Env,
    appointment_id: u64,
    patient: Address,
    provider: Address,
    verifier: Address,
) {
    let topics = (symbol_short!("APPT_VER"), patient.clone(), provider.clone());
    let data = AppointmentVerifiedEvent {
        appointment_id,
        patient,
        provider,
        verifier,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Event published when an audit log entry is created.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditLogEntryEvent {
    pub entry_id: u64,
    pub actor: Address,
    pub patient: Address,
    pub record_id: Option<u64>,
    pub action: AccessAction,
    pub result: AccessResult,
    pub reason: Option<String>,
    pub timestamp: u64,
}

/// Publishes an audit log entry event.
pub fn publish_audit_log_entry(env: &Env, entry: &AuditEntry) {
    let topics = (
        symbol_short!("AUDIT"),
        entry.actor.clone(),
        entry.patient.clone(),
    );
    let data = AuditLogEntryEvent {
        entry_id: entry.id,
        actor: entry.actor.clone(),
        patient: entry.patient.clone(),
        record_id: entry.record_id,
        action: entry.action.clone(),
        result: entry.result.clone(),
        reason: entry.reason.clone(),
        timestamp: entry.timestamp,
    };
    env.events().publish(topics, data);
}

/// Event published when rate limit is exceeded.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitExceededEvent {
    pub address: Address,
    pub operation: String,
    pub current_count: u32,
    pub max_requests: u32,
    pub reset_at: u64,
    pub timestamp: u64,
}

/// Event published when rate limit configuration is updated.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitConfigUpdatedEvent {
    pub operation: String,
    pub max_requests: u32,
    pub window_seconds: u64,
    pub updated_by: Address,
    pub timestamp: u64,
}

/// Event published when rate limit bypass is granted or revoked.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitBypassUpdatedEvent {
    pub address: Address,
    pub bypass_enabled: bool,
    pub updated_by: Address,
    pub timestamp: u64,
}

/// Publishes a rate limit exceeded event.
pub fn publish_rate_limit_exceeded(
    env: &Env,
    address: Address,
    operation: String,
    current_count: u32,
    max_requests: u32,
    reset_at: u64,
) {
    let topics = (symbol_short!("RL_EXCD"), address.clone(), operation.clone());
    let data = RateLimitExceededEvent {
        address: address.clone(),
        operation: operation.clone(),
        current_count,
        max_requests,
        reset_at,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes a rate limit configuration updated event.
pub fn publish_rate_limit_config_updated(
    env: &Env,
    operation: String,
    max_requests: u32,
    window_seconds: u64,
    updated_by: Address,
) {
    let topics = (symbol_short!("RL_CONFIG"), operation.clone());
    let data = RateLimitConfigUpdatedEvent {
        operation: operation.clone(),
        max_requests,
        window_seconds,
        updated_by: updated_by.clone(),
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes a rate limit bypass updated event.
pub fn publish_rate_limit_bypass_updated(
    env: &Env,
    address: Address,
    bypass_enabled: bool,
    updated_by: Address,
) {
    let topics = (symbol_short!("RL_BYPASS"), address.clone());
    let data = RateLimitBypassUpdatedEvent {
        address: address.clone(),
        bypass_enabled,
        updated_by: updated_by.clone(),
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Event published when an access policy is created.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyCreatedEvent {
    pub policy_id: String,
    pub created_by: Address,
    pub timestamp: u64,
}

/// Event published when a user credential is set.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialSetEvent {
    pub user: Address,
    pub credential: crate::CredentialType,
    pub set_by: Address,
    pub timestamp: u64,
}

/// Event published when a record sensitivity level is set.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SensitivitySetEvent {
    pub record_id: u64,
    pub sensitivity: crate::SensitivityLevel,
    pub set_by: Address,
    pub timestamp: u64,
}

/// Publishes an event when an access policy is created.
pub fn publish_policy_created(env: &Env, policy_id: String, created_by: Address) {
    let topics = (symbol_short!("POL_CRT"),);
    let data = PolicyCreatedEvent {
        policy_id,
        created_by,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when a user credential is set.
pub fn publish_credential_set(
    env: &Env,
    user: Address,
    credential: crate::CredentialType,
    set_by: Address,
) {
    let topics = (symbol_short!("CRED_SET"), user.clone());
    let data = CredentialSetEvent {
        user,
        credential,
        set_by,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an event when a record sensitivity level is set.
pub fn publish_sensitivity_set(
    env: &Env,
    record_id: u64,
    sensitivity: crate::SensitivityLevel,
    set_by: Address,
) {
    let topics = (symbol_short!("SENS_SET"), record_id);
    let data = SensitivitySetEvent {
        record_id,
        sensitivity,
        set_by,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}
