use crate::appointment::AppointmentType;
use crate::audit::{AccessAction, AccessResult, AuditEntry};
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

/// Publishes an event when the contract is initialized.
/// This event includes the admin address and initialization timestamp.
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

/// Event published when a new provider is registered.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRegisteredEvent {
    pub provider: Address,
    pub name: String,
    pub provider_id: u64,
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

/// Publishes an event when provider information is updated.
/// This event includes the provider address and update timestamp.
pub fn publish_provider_updated(env: &Env, provider: Address) {
    let topics = (symbol_short!("PROV_UPD"), provider.clone());
    let data = ProviderUpdatedEvent {
        provider,
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
