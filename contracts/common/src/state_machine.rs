#![allow(clippy::arithmetic_side_effects)]

use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Bytes, BytesN, Env, String, Symbol, Vec,
};

const SM_STATE: Symbol = symbol_short!("SM_STATE");
const SM_LOG: Symbol = symbol_short!("SM_LOG");
const SM_LAST: Symbol = symbol_short!("SM_LAST");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityKind {
    VisionRecord,
    Prescription,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VisionRecordState {
    Draft,
    PendingReview,
    Approved,
    Archived,
    Purged,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrescriptionState {
    Created,
    Dispensed,
    PartiallyFilled,
    Completed,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleState {
    Vision(VisionRecordState),
    Prescription(PrescriptionState),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionContext {
    pub actor: Address,
    pub actor_role: Symbol,
    pub now: u64,
    pub retention_until: u64,
    pub expires_at: u64,
    pub prerequisites_met: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionRecord {
    pub entity_kind: EntityKind,
    pub entity_id: u64,
    pub from_state: LifecycleState,
    pub to_state: LifecycleState,
    pub actor: Address,
    pub timestamp: u64,
    pub prev_hash: BytesN<32>,
    pub transition_hash: BytesN<32>,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum StateMachineError {
    InvalidTransition = 1,
    UnauthorizedRole = 2,
    TimeConstraintFailed = 3,
    PrerequisiteFailed = 4,
}

fn state_key(machine_id: u32, kind: &EntityKind, entity_id: u64) -> (Symbol, u32, EntityKind, u64) {
    (SM_STATE, machine_id, kind.clone(), entity_id)
}

fn log_key(machine_id: u32, kind: &EntityKind, entity_id: u64) -> (Symbol, u32, EntityKind, u64) {
    (SM_LOG, machine_id, kind.clone(), entity_id)
}

fn last_hash_key(
    machine_id: u32,
    kind: &EntityKind,
    entity_id: u64,
) -> (Symbol, u32, EntityKind, u64) {
    (SM_LAST, machine_id, kind.clone(), entity_id)
}

pub fn default_state(kind: &EntityKind) -> LifecycleState {
    match kind {
        EntityKind::VisionRecord => LifecycleState::Vision(VisionRecordState::Draft),
        EntityKind::Prescription => LifecycleState::Prescription(PrescriptionState::Created),
    }
}

pub fn get_state(env: &Env, machine_id: u32, kind: &EntityKind, entity_id: u64) -> LifecycleState {
    let key = state_key(machine_id, kind, entity_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(default_state(kind))
}

pub fn get_transition_log(
    env: &Env,
    machine_id: u32,
    kind: &EntityKind,
    entity_id: u64,
) -> Vec<TransitionRecord> {
    let key = log_key(machine_id, kind, entity_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env))
}

fn can_transition(
    from: &LifecycleState,
    to: &LifecycleState,
    ctx: &TransitionContext,
) -> Result<(), StateMachineError> {
    if !ctx.prerequisites_met {
        return Err(StateMachineError::PrerequisiteFailed);
    }

    match (from, to) {
        (
            LifecycleState::Vision(VisionRecordState::Draft),
            LifecycleState::Vision(VisionRecordState::PendingReview),
        ) => {
            if ctx.actor_role != symbol_short!("PROV") && ctx.actor_role != symbol_short!("ADMIN") {
                return Err(StateMachineError::UnauthorizedRole);
            }
            Ok(())
        }
        (
            LifecycleState::Vision(VisionRecordState::PendingReview),
            LifecycleState::Vision(VisionRecordState::Approved),
        ) => {
            if ctx.actor_role != symbol_short!("OPHT") && ctx.actor_role != symbol_short!("ADMIN") {
                return Err(StateMachineError::UnauthorizedRole);
            }
            Ok(())
        }
        (
            LifecycleState::Vision(VisionRecordState::Approved),
            LifecycleState::Vision(VisionRecordState::Archived),
        ) => {
            if ctx.actor_role != symbol_short!("ADMIN") {
                return Err(StateMachineError::UnauthorizedRole);
            }
            Ok(())
        }
        (
            LifecycleState::Vision(VisionRecordState::Archived),
            LifecycleState::Vision(VisionRecordState::Purged),
        ) => {
            if ctx.actor_role != symbol_short!("ADMIN") {
                return Err(StateMachineError::UnauthorizedRole);
            }
            if ctx.now < ctx.retention_until {
                return Err(StateMachineError::TimeConstraintFailed);
            }
            Ok(())
        }
        (
            LifecycleState::Prescription(PrescriptionState::Created),
            LifecycleState::Prescription(PrescriptionState::Dispensed),
        ) => {
            if ctx.actor_role != symbol_short!("PROV")
                && ctx.actor_role != symbol_short!("PHARM")
                && ctx.actor_role != symbol_short!("ADMIN")
            {
                return Err(StateMachineError::UnauthorizedRole);
            }
            Ok(())
        }
        (
            LifecycleState::Prescription(PrescriptionState::Dispensed),
            LifecycleState::Prescription(PrescriptionState::PartiallyFilled),
        ) => Ok(()),
        (
            LifecycleState::Prescription(PrescriptionState::Dispensed),
            LifecycleState::Prescription(PrescriptionState::Completed),
        ) => Ok(()),
        (
            LifecycleState::Prescription(PrescriptionState::PartiallyFilled),
            LifecycleState::Prescription(PrescriptionState::Completed),
        ) => Ok(()),
        (
            LifecycleState::Prescription(_),
            LifecycleState::Prescription(PrescriptionState::Expired),
        ) => {
            if ctx.now < ctx.expires_at {
                return Err(StateMachineError::TimeConstraintFailed);
            }
            Ok(())
        }
        _ => Err(StateMachineError::InvalidTransition),
    }
}

#[allow(clippy::too_many_arguments)]
fn hash_transition(
    env: &Env,
    kind: &EntityKind,
    entity_id: u64,
    from: &LifecycleState,
    to: &LifecycleState,
    actor: &Address,
    timestamp: u64,
    prev_hash: &BytesN<32>,
) -> BytesN<32> {
    let mut payload = Bytes::new(env);
    payload.append(&Bytes::from_slice(env, &entity_id.to_be_bytes()));
    payload.append(&Bytes::from_slice(env, &timestamp.to_be_bytes()));
    payload.append(&actor.to_string().to_bytes());
    payload.append(&Bytes::from_slice(
        env,
        &serialize_state_tag(from).to_be_bytes(),
    ));
    payload.append(&Bytes::from_slice(
        env,
        &serialize_state_tag(to).to_be_bytes(),
    ));
    payload.append(&Bytes::from_slice(
        env,
        &serialize_kind_tag(kind).to_be_bytes(),
    ));
    payload.append(&Bytes::from_slice(env, &prev_hash.to_array()));
    env.crypto().sha256(&payload).into()
}

fn serialize_kind_tag(kind: &EntityKind) -> u32 {
    match kind {
        EntityKind::VisionRecord => 1,
        EntityKind::Prescription => 2,
    }
}

fn serialize_state_tag(state: &LifecycleState) -> u32 {
    match state {
        LifecycleState::Vision(VisionRecordState::Draft) => 100,
        LifecycleState::Vision(VisionRecordState::PendingReview) => 101,
        LifecycleState::Vision(VisionRecordState::Approved) => 102,
        LifecycleState::Vision(VisionRecordState::Archived) => 103,
        LifecycleState::Vision(VisionRecordState::Purged) => 104,
        LifecycleState::Prescription(PrescriptionState::Created) => 200,
        LifecycleState::Prescription(PrescriptionState::Dispensed) => 201,
        LifecycleState::Prescription(PrescriptionState::PartiallyFilled) => 202,
        LifecycleState::Prescription(PrescriptionState::Completed) => 203,
        LifecycleState::Prescription(PrescriptionState::Expired) => 204,
    }
}

pub fn apply_transition(
    env: &Env,
    machine_id: u32,
    kind: &EntityKind,
    entity_id: u64,
    to: LifecycleState,
    ctx: TransitionContext,
) -> Result<TransitionRecord, StateMachineError> {
    let from = get_state(env, machine_id, kind, entity_id);
    can_transition(&from, &to, &ctx)?;

    let prev_key = last_hash_key(machine_id, kind, entity_id);
    let prev_hash: BytesN<32> = env
        .storage()
        .persistent()
        .get(&prev_key)
        .unwrap_or(BytesN::from_array(env, &[0u8; 32]));

    let transition_hash = hash_transition(
        env, kind, entity_id, &from, &to, &ctx.actor, ctx.now, &prev_hash,
    );

    let record = TransitionRecord {
        entity_kind: kind.clone(),
        entity_id,
        from_state: from,
        to_state: to.clone(),
        actor: ctx.actor,
        timestamp: ctx.now,
        prev_hash,
        transition_hash: transition_hash.clone(),
    };

    let key = state_key(machine_id, kind, entity_id);
    env.storage().persistent().set(&key, &to);

    let lkey = log_key(machine_id, kind, entity_id);
    let mut logs: Vec<TransitionRecord> = env
        .storage()
        .persistent()
        .get(&lkey)
        .unwrap_or(Vec::new(env));
    logs.push_back(record.clone());
    env.storage().persistent().set(&lkey, &logs);

    env.storage().persistent().set(&prev_key, &transition_hash);

    Ok(record)
}

pub fn export_dot(env: &Env, kind: &EntityKind) -> String {
    match kind {
        EntityKind::VisionRecord => String::from_str(
            env,
            "digraph vision_record_lifecycle { Draft -> PendingReview; PendingReview -> Approved; Approved -> Archived; Archived -> Purged; }",
        ),
        EntityKind::Prescription => String::from_str(
            env,
            "digraph prescription_lifecycle { Created -> Dispensed; Dispensed -> PartiallyFilled; Dispensed -> Completed; PartiallyFilled -> Completed; Created -> Expired; Dispensed -> Expired; PartiallyFilled -> Expired; }",
        ),
    }
}
