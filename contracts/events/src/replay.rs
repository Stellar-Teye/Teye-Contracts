//! Event replay, checkpointing, compaction, and dead letter queue.
//!
//! Replay allows consumers to catch up from any checkpoint or arbitrary event ID.
//! Compaction merges sequential update events for the same topic to reduce the
//! number of entries a consumer must process during replay.
//!
//! The dead letter queue captures events that failed delivery so they can be
//! retried or inspected later.

use crate::subscription::dispatch_to_subscribers;
use crate::{EventEnvelope, EventError};
use soroban_sdk::{contracttype, symbol_short, Env, Address, String, Vec};

// ── Storage key constants ────────────────────────────────────────────────────

const CHKPT_CTR: soroban_sdk::Symbol = symbol_short!("CHKP_CTR");
const DLQ_KEY: soroban_sdk::Symbol = symbol_short!("DLQ");
const MAX_DLQ_SIZE: u32 = 100;

// ── Types ────────────────────────────────────────────────────────────────────

/// A checkpoint that records an event log position at a point in time.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub id: u64,
    pub event_id: u64,
    pub ledger_ts: u64,
}

/// An entry in the dead letter queue representing a failed event delivery.
#[contracttype]
#[derive(Clone, Debug)]
pub struct DeadLetterEntry {
    pub event_id: u64,
    pub subscriber: Address,
    pub reason: String,
    pub failed_at: u64,
}

// ── Storage key helpers ──────────────────────────────────────────────────────

fn checkpoint_key(checkpoint_id: u64) -> (soroban_sdk::Symbol, u64) {
    (symbol_short!("CHKPT"), checkpoint_id)
}

fn log_key(event_id: u64) -> (soroban_sdk::Symbol, u64) {
    (symbol_short!("LOG"), event_id)
}

fn compacted_key(topic: &String) -> (soroban_sdk::Symbol, String) {
    (symbol_short!("T_CMPCT"), topic.clone())
}

// ── Replay ───────────────────────────────────────────────────────────────────

/// Replay events starting from `from_event_id` up to `limit` entries.
///
/// Walks the global ordered index and returns events whose ID is greater than
/// or equal to `from_event_id`. This guarantees the same ordering that was
/// established by the Lamport timestamps during publishing.
pub fn replay_from(
    env: &Env,
    from_event_id: u64,
    limit: u32,
) -> Result<Vec<EventEnvelope>, EventError> {
    if limit == 0 {
        return Err(EventError::InvalidInput);
    }

    let idx_key = symbol_short!("LOG_IDX");
    let index: Vec<u64> = env
        .storage()
        .persistent()
        .get(&idx_key)
        .unwrap_or(Vec::new(env));

    let mut result = Vec::new(env);
    let mut collected = 0u32;

    for eid in index.iter() {
        if eid >= from_event_id && collected < limit {
            if let Some(envelope) = env
                .storage()
                .persistent()
                .get::<_, EventEnvelope>(&log_key(eid))
            {
                result.push_back(envelope);
                collected += 1;
            }
        }
        if collected >= limit {
            break;
        }
    }

    Ok(result)
}

/// Replay events for a specific topic starting from a given event ID.
///
/// Filters the global log to return only events matching the requested topic.
pub fn replay_topic(
    env: &Env,
    topic: &String,
    from_event_id: u64,
    limit: u32,
) -> Result<Vec<EventEnvelope>, EventError> {
    if limit == 0 || topic.len() == 0 {
        return Err(EventError::InvalidInput);
    }

    let idx_key = symbol_short!("LOG_IDX");
    let index: Vec<u64> = env
        .storage()
        .persistent()
        .get(&idx_key)
        .unwrap_or(Vec::new(env));

    let mut result = Vec::new(env);
    let mut collected = 0u32;

    for eid in index.iter() {
        if eid >= from_event_id && collected < limit {
            if let Some(envelope) = env
                .storage()
                .persistent()
                .get::<_, EventEnvelope>(&log_key(eid))
            {
                if envelope.topic == *topic {
                    result.push_back(envelope);
                    collected += 1;
                }
            }
        }
        if collected >= limit {
            break;
        }
    }

    Ok(result)
}

// ── Checkpoints ──────────────────────────────────────────────────────────────

/// Create a checkpoint at the current event log head.
/// Returns the checkpoint ID.
#[allow(clippy::arithmetic_side_effects)]
pub fn create_checkpoint(env: &Env) -> Result<u64, EventError> {
    let evt_ctr_key = symbol_short!("EVT_CTR");
    let current_event_id: u64 = env.storage().instance().get(&evt_ctr_key).unwrap_or(0);

    let chkpt_id = next_checkpoint_id(env);

    let checkpoint = Checkpoint {
        id: chkpt_id,
        event_id: current_event_id,
        ledger_ts: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&checkpoint_key(chkpt_id), &checkpoint);

    env.events().publish(
        (symbol_short!("CHKPT"), chkpt_id),
        checkpoint.clone(),
    );

    Ok(chkpt_id)
}

/// Retrieve the event ID stored at a given checkpoint.
pub fn get_checkpoint(env: &Env, checkpoint_id: u64) -> Result<u64, EventError> {
    let checkpoint: Checkpoint = env
        .storage()
        .persistent()
        .get(&checkpoint_key(checkpoint_id))
        .ok_or(EventError::CheckpointNotFound)?;
    Ok(checkpoint.event_id)
}

// ── Compaction ───────────────────────────────────────────────────────────────

/// Compact events for a topic by keeping only the latest event per topic.
///
/// Scans all events for the given topic and marks earlier entries as compacted,
/// preserving only the most recent event. Returns the number of events removed.
///
/// This reduces replay overhead for consumers that only need the latest state.
#[allow(clippy::arithmetic_side_effects)]
pub fn compact_topic(env: &Env, topic: &String) -> Result<u32, EventError> {
    if topic.len() == 0 {
        return Err(EventError::InvalidInput);
    }

    let idx_key = symbol_short!("LOG_IDX");
    let index: Vec<u64> = env
        .storage()
        .persistent()
        .get(&idx_key)
        .unwrap_or(Vec::new(env));

    // Collect all event IDs for this topic
    let mut topic_event_ids = Vec::new(env);
    for eid in index.iter() {
        if let Some(envelope) = env
            .storage()
            .persistent()
            .get::<_, EventEnvelope>(&log_key(eid))
        {
            if envelope.topic == *topic {
                topic_event_ids.push_back(eid);
            }
        }
    }

    let total = topic_event_ids.len();
    if total <= 1 {
        return Ok(0);
    }

    // Keep only the last event, remove the rest from the log
    let mut removed = 0u32;
    for i in 0..(total - 1) {
        if let Some(eid) = topic_event_ids.get(i) {
            env.storage().persistent().remove(&log_key(eid));
            removed += 1;
        }
    }

    // Rebuild the global index without compacted entries
    let mut new_index = Vec::new(env);
    for eid in index.iter() {
        if env.storage().persistent().has(&log_key(eid)) {
            new_index.push_back(eid);
        }
    }
    env.storage().persistent().set(&idx_key, &new_index);

    // Record compaction metadata
    env.storage().persistent().set(&compacted_key(topic), &removed);

    env.events().publish(
        (symbol_short!("COMPACT"), topic.clone()),
        removed,
    );

    Ok(removed)
}

// ── Dead letter queue ────────────────────────────────────────────────────────

/// Push a failed delivery into the dead letter queue.
pub fn push_dead_letter(
    env: &Env,
    event_id: u64,
    subscriber: &Address,
    reason: &String,
) -> Result<(), EventError> {
    let mut dlq: Vec<DeadLetterEntry> = env
        .storage()
        .persistent()
        .get(&DLQ_KEY)
        .unwrap_or(Vec::new(env));

    if dlq.len() >= MAX_DLQ_SIZE {
        return Err(EventError::DeadLetterFull);
    }

    let entry = DeadLetterEntry {
        event_id,
        subscriber: subscriber.clone(),
        reason: reason.clone(),
        failed_at: env.ledger().timestamp(),
    };

    dlq.push_back(entry.clone());
    env.storage().persistent().set(&DLQ_KEY, &dlq);

    env.events().publish(
        (symbol_short!("DLQ_PUSH"), subscriber.clone()),
        event_id,
    );

    Ok(())
}

/// Return all entries in the dead letter queue.
pub fn get_dead_letters(env: &Env) -> Vec<DeadLetterEntry> {
    env.storage()
        .persistent()
        .get(&DLQ_KEY)
        .unwrap_or(Vec::new(env))
}

/// Retry a specific dead letter entry by re-dispatching the original event.
///
/// Removes the entry from the DLQ upon successful re-dispatch.
pub fn retry_dead_letter(env: &Env, dead_letter_index: u32) -> Result<(), EventError> {
    let dlq: Vec<DeadLetterEntry> = env
        .storage()
        .persistent()
        .get(&DLQ_KEY)
        .unwrap_or(Vec::new(env));

    if dead_letter_index >= dlq.len() {
        return Err(EventError::InvalidInput);
    }

    let entry = dlq.get(dead_letter_index).ok_or(EventError::InvalidInput)?;

    // Retrieve the original event
    let envelope: EventEnvelope = env
        .storage()
        .persistent()
        .get(&log_key(entry.event_id))
        .ok_or(EventError::EventNotFound)?;

    // Re-dispatch the event to subscribers
    dispatch_to_subscribers(env, &envelope);

    // Remove the entry from the DLQ by rebuilding without it
    let mut new_dlq = Vec::new(env);
    for i in 0..dlq.len() {
        if i != dead_letter_index {
            if let Some(e) = dlq.get(i) {
                new_dlq.push_back(e);
            }
        }
    }
    env.storage().persistent().set(&DLQ_KEY, &new_dlq);

    env.events().publish(
        (symbol_short!("DLQ_RTY"), entry.subscriber.clone()),
        entry.event_id,
    );

    Ok(())
}

// ── Counter helpers ──────────────────────────────────────────────────────────

#[allow(clippy::arithmetic_side_effects)]
fn next_checkpoint_id(env: &Env) -> u64 {
    let current: u64 = env.storage().instance().get(&CHKPT_CTR).unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&CHKPT_CTR, &next);
    next
}
