use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol, Vec};

// ── Storage keys ──────────────────────────────────────────────
pub const APPT_CTR: Symbol = symbol_short!("APPT_CTR");
const APPT_RECORD: Symbol = symbol_short!("APPT_REC");
const APPT_PATIENT: Symbol = symbol_short!("APPT_PAT");
const APPT_PROVIDER: Symbol = symbol_short!("APPT_PROV");
const APPT_HISTORY: Symbol = symbol_short!("APPT_HIST");

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

/// Extends the time-to-live (TTL) for appointment storage keys.
fn extend_ttl_appointment_key(env: &Env, key: &(Symbol, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Extends the time-to-live (TTL) for appointment by patient keys.
fn extend_ttl_appointment_patient_key(env: &Env, key: &(Symbol, Address, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Extends the time-to-live (TTL) for appointment by provider keys.
fn extend_ttl_appointment_provider_key(env: &Env, key: &(Symbol, Address, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Types ─────────────────────────────────────────────────────

/// Status of an appointment
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AppointmentStatus {
    None = 0, // Used for history entries when there's no previous status
    Scheduled = 1,
    Confirmed = 2,
    Completed = 3,
    Cancelled = 4,
    NoShow = 5,
    Rescheduled = 6,
}

/// Type of appointment
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AppointmentType {
    Examination = 1,
    Consultation = 2,
    FollowUp = 3,
    Surgery = 4,
    Emergency = 5,
    Routine = 6,
}

/// An appointment record
#[contracttype]
#[derive(Clone, Debug)]
pub struct Appointment {
    pub id: u64,
    pub patient: Address,
    pub provider: Address,
    pub appointment_type: AppointmentType,
    pub scheduled_at: u64,
    pub duration_minutes: u32,
    pub status: AppointmentStatus,
    pub notes: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub verified_at: Option<u64>,
    pub verified_by: Option<Address>,
    pub reminder_sent: bool,
}

/// Appointment history entry for tracking changes
#[contracttype]
#[derive(Clone, Debug)]
pub struct AppointmentHistoryEntry {
    pub appointment_id: u64,
    pub action: String, // e.g. "CREATED", "CONFIRMED", "CANCELLED", "RESCHEDULED", "COMPLETED"
    pub actor: Address,
    pub timestamp: u64,
    pub previous_status: AppointmentStatus, // Use AppointmentStatus::None when there's no previous status
    pub new_status: AppointmentStatus,
    pub notes: Option<String>,
}

// ── Storage Functions ────────────────────────────────────────

/// Increments and returns the next appointment ID
pub fn increment_appointment_counter(env: &Env) -> u64 {
    let current: u64 = env.storage().instance().get(&APPT_CTR).unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&APPT_CTR, &next);
    next
}

/// Stores an appointment record
pub fn set_appointment(env: &Env, appointment: &Appointment) {
    let key = (APPT_RECORD, appointment.id);
    env.storage().persistent().set(&key, appointment);
    extend_ttl_appointment_key(env, &key);

    // Index by patient for quick lookup
    let patient_key = (APPT_PATIENT, appointment.patient.clone(), appointment.id);
    env.storage().persistent().set(&patient_key, &true);
    extend_ttl_appointment_patient_key(env, &patient_key);

    // Index by provider for quick lookup
    let provider_key = (APPT_PROVIDER, appointment.provider.clone(), appointment.id);
    env.storage().persistent().set(&provider_key, &true);
    extend_ttl_appointment_provider_key(env, &provider_key);
}

/// Retrieves an appointment by ID
pub fn get_appointment(env: &Env, appointment_id: u64) -> Option<Appointment> {
    let key = (APPT_RECORD, appointment_id);
    env.storage().persistent().get(&key)
}

/// Gets all appointments for a patient
pub fn get_patient_appointments(env: &Env, patient: &Address) -> Vec<Appointment> {
    let mut appointments = Vec::new(env);
    let counter: u64 = env.storage().instance().get(&APPT_CTR).unwrap_or(0);
    let start_id = if counter > 100 { counter - 100 } else { 1 };

    for id in start_id..=counter {
        let key = (APPT_RECORD, id);
        if let Some(appointment) = env.storage().persistent().get::<_, Appointment>(&key) {
            if appointment.patient == *patient {
                appointments.push_back(appointment);
            }
        }
    }
    appointments
}

/// Gets all appointments for a provider
pub fn get_provider_appointments(env: &Env, provider: &Address) -> Vec<Appointment> {
    let mut appointments = Vec::new(env);
    let counter: u64 = env.storage().instance().get(&APPT_CTR).unwrap_or(0);
    let start_id = if counter > 100 { counter - 100 } else { 1 };

    for id in start_id..=counter {
        let key = (APPT_RECORD, id);
        if let Some(appointment) = env.storage().persistent().get::<_, Appointment>(&key) {
            if appointment.provider == *provider {
                appointments.push_back(appointment);
            }
        }
    }
    appointments
}

/// Gets upcoming appointments for a patient (scheduled time in the future)
pub fn get_upcoming_patient_appointments(env: &Env, patient: &Address) -> Vec<Appointment> {
    let mut appointments = Vec::new(env);
    let current_time = env.ledger().timestamp();
    let counter: u64 = env.storage().instance().get(&APPT_CTR).unwrap_or(0);
    let start_id = if counter > 100 { counter - 100 } else { 1 };

    for id in start_id..=counter {
        let key = (APPT_RECORD, id);
        if let Some(appointment) = env.storage().persistent().get::<_, Appointment>(&key) {
            if appointment.patient == *patient
                && appointment.scheduled_at > current_time
                && (appointment.status == AppointmentStatus::Scheduled
                    || appointment.status == AppointmentStatus::Confirmed)
            {
                appointments.push_back(appointment);
            }
        }
    }
    appointments
}

/// Adds a history entry for an appointment
pub fn add_history_entry(env: &Env, entry: &AppointmentHistoryEntry) {
    let key = (APPT_HISTORY, entry.appointment_id);
    let mut history: Vec<AppointmentHistoryEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));

    history.push_back(entry.clone());

    // Limit history to 1000 entries per appointment
    if history.len() > 1000 {
        let mut new_history = Vec::new(env);
        for i in 1..history.len() {
            if let Some(entry) = history.get(i) {
                new_history.push_back(entry);
            }
        }
        history = new_history;
    }

    env.storage().persistent().set(&key, &history);
    extend_ttl_appointment_key(env, &key);
}

/// Retrieves history entries for an appointment
pub fn get_appointment_history(env: &Env, appointment_id: u64) -> Vec<AppointmentHistoryEntry> {
    let key = (APPT_HISTORY, appointment_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env))
}

/// Gets appointments that need reminders (scheduled within reminder window)
pub fn get_appointments_needing_reminders(
    env: &Env,
    reminder_window_seconds: u64,
) -> Vec<Appointment> {
    let mut appointments = Vec::new(env);
    let current_time = env.ledger().timestamp();
    let reminder_threshold = current_time + reminder_window_seconds;
    let counter: u64 = env.storage().instance().get(&APPT_CTR).unwrap_or(0);
    let start_id = if counter > 100 { counter - 100 } else { 1 };

    for id in start_id..=counter {
        let key = (APPT_RECORD, id);
        if let Some(appointment) = env.storage().persistent().get::<_, Appointment>(&key) {
            if appointment.scheduled_at <= reminder_threshold
                && appointment.scheduled_at > current_time
                && !appointment.reminder_sent
                && (appointment.status == AppointmentStatus::Scheduled
                    || appointment.status == AppointmentStatus::Confirmed)
            {
                appointments.push_back(appointment);
            }
        }
    }
    appointments
}

/// Marks an appointment's reminder as sent
pub fn mark_reminder_sent(env: &Env, appointment_id: u64) -> Option<Appointment> {
    let key = (APPT_RECORD, appointment_id);
    if let Some(mut appointment) = env.storage().persistent().get::<_, Appointment>(&key) {
        appointment.reminder_sent = true;
        appointment.updated_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &appointment);
        extend_ttl_appointment_key(env, &key);
        Some(appointment)
    } else {
        None
    }
}
