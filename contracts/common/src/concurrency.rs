//! Optimistic concurrency control (OCC) primitives for the Teye contract suite.
//!
//! Provides compare-and-swap semantics for record updates, conflict detection
//! with configurable resolution strategies, a conflict queue for manual review,
//! and field-level conflict tracking to minimise false positives.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, String, Symbol, Vec};

use crate::vector_clock::{ClockOrdering, VectorClock};

// ── Storage key prefixes ────────────────────────────────────────────────────

const VER_KEY: Symbol = symbol_short!("OCC_VER");
const CLOCK_KEY: Symbol = symbol_short!("OCC_CLK");
const CONFLICT_Q: Symbol = symbol_short!("OCC_CFQ");
const CONFLICT_CTR: Symbol = symbol_short!("OCC_CCTR");
const STRATEGY_KEY: Symbol = symbol_short!("OCC_STRT");

const TTL_THRESHOLD: u32 = 5_184_000;
const TTL_EXTEND_TO: u32 = 10_368_000;

/// Maximum number of conflicts retained in the queue before the oldest are
/// evicted. Prevents unbounded storage growth.
pub const MAX_CONFLICT_QUEUE_SIZE: u32 = 256;

// ── Types ───────────────────────────────────────────────────────────────────

/// Resolution strategy for concurrent conflicts.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolutionStrategy {
    /// The most recent write (by ledger timestamp) wins automatically.
    LastWriterWins,
    /// Non-conflicting fields are merged; conflicting fields are queued.
    Merge,
    /// All conflicts are queued for manual review.
    ManualReview,
}

/// Status of a queued conflict entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConflictStatus {
    /// Awaiting manual resolution.
    Pending,
    /// Resolved by an authorised reviewer.
    Resolved,
    /// Automatically resolved via configured strategy.
    AutoResolved,
}

/// Identifies which fields of a record were modified by a given update.
///
/// Field names are stored as Soroban `String`s so they can be compared
/// across two concurrent modifications for field-level conflict detection.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldChange {
    pub field_name: String,
    pub old_hash: String,
    pub new_hash: String,
}

/// A conflict entry stored in the on-chain conflict queue.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ConflictEntry {
    pub conflict_id: u64,
    pub record_id: u64,
    pub provider_a: Address,
    pub provider_b: Address,
    pub clock_a: VectorClock,
    pub clock_b: VectorClock,
    pub conflicting_fields: Vec<String>,
    pub status: ConflictStatus,
    pub strategy: ResolutionStrategy,
    pub created_at: u64,
    pub resolved_at: Option<u64>,
    pub resolved_by: Option<Address>,
}

/// Snapshot returned after a successful compare-and-swap so the caller knows
/// the new version and clock state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionStamp {
    pub version: u64,
    pub clock: VectorClock,
}

/// Result of an attempted update under OCC.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateOutcome {
    /// The update was applied cleanly.
    Applied(VersionStamp),
    /// A non-conflicting merge was performed; the stamp reflects the merged state.
    Merged(VersionStamp),
    /// A conflict was detected and queued; returns the conflict ID.
    Conflicted(u64),
}

// ── Version helpers ─────────────────────────────────────────────────────────

fn version_key(record_id: u64) -> (Symbol, u64) {
    (VER_KEY, record_id)
}

fn clock_key(record_id: u64) -> (Symbol, u64) {
    (CLOCK_KEY, record_id)
}

fn strategy_key_for(record_id: u64) -> (Symbol, u64) {
    (STRATEGY_KEY, record_id)
}

/// Reads the current version of a record, or `0` if not yet tracked.
pub fn get_record_version(env: &Env, record_id: u64) -> u64 {
    env.storage()
        .persistent()
        .get(&version_key(record_id))
        .unwrap_or(0)
}

/// Reads the current vector clock for a record.
pub fn get_record_clock(env: &Env, record_id: u64) -> VectorClock {
    env.storage()
        .persistent()
        .get(&clock_key(record_id))
        .unwrap_or_else(|| VectorClock::new(env))
}

/// Sets the resolution strategy for a specific record.
pub fn set_resolution_strategy(env: &Env, record_id: u64, strategy: &ResolutionStrategy) {
    env.storage()
        .persistent()
        .set(&strategy_key_for(record_id), strategy);
}

/// Gets the resolution strategy for a record, defaulting to `ManualReview`.
pub fn get_resolution_strategy(env: &Env, record_id: u64) -> ResolutionStrategy {
    env.storage()
        .persistent()
        .get(&strategy_key_for(record_id))
        .unwrap_or(ResolutionStrategy::ManualReview)
}

/// Initialises version tracking for a newly created record.
pub fn init_record_version(env: &Env, record_id: u64, node_id: u32) -> VersionStamp {
    let vk = version_key(record_id);
    env.storage().persistent().set(&vk, &1u64);
    env.storage()
        .persistent()
        .extend_ttl(&vk, TTL_THRESHOLD, TTL_EXTEND_TO);

    let mut clock = VectorClock::new(env);
    clock.increment(env, node_id);
    let ck = clock_key(record_id);
    env.storage().persistent().set(&ck, &clock);
    env.storage()
        .persistent()
        .extend_ttl(&ck, TTL_THRESHOLD, TTL_EXTEND_TO);

    VersionStamp { version: 1, clock }
}

// ── Compare-and-swap ────────────────────────────────────────────────────────

/// Attempts an optimistic compare-and-swap update on `record_id`.
///
/// # Parameters
/// - `expected_version` – the version the caller read before modifying.
/// - `node_id` – compact provider identifier for the vector clock.
/// - `changed_fields` – list of field-level changes the caller wants to apply.
///
/// # Returns
/// An [`UpdateOutcome`] describing whether the update was applied, merged, or
/// queued as a conflict.
#[allow(clippy::too_many_arguments)]
pub fn compare_and_swap(
    env: &Env,
    record_id: u64,
    expected_version: u64,
    node_id: u32,
    provider: &Address,
    changed_fields: &Vec<FieldChange>,
) -> UpdateOutcome {
    let current_version = get_record_version(env, record_id);
    let current_clock = get_record_clock(env, record_id);

    // Fast path: versions match — no contention.
    if current_version == expected_version {
        return apply_update(env, record_id, node_id, &current_clock);
    }

    // Versions diverge — build the caller's tentative clock to check ordering.
    let mut caller_clock = current_clock.clone();
    caller_clock.increment(env, node_id);

    let ordering = caller_clock.compare(&current_clock);

    let strategy = get_resolution_strategy(env, record_id);

    match strategy {
        ResolutionStrategy::LastWriterWins => {
            // Simply overwrite regardless of conflict.
            apply_update(env, record_id, node_id, &current_clock)
        }
        ResolutionStrategy::Merge => {
            // If the fields don't actually overlap, merge automatically.
            let overlapping = detect_field_conflicts(env, record_id, changed_fields);
            if overlapping.is_empty() {
                let stamp = bump_version(env, record_id, node_id, &current_clock);
                UpdateOutcome::Merged(stamp)
            } else {
                let cid = enqueue_conflict(
                    env,
                    record_id,
                    provider,
                    &current_clock,
                    &caller_clock,
                    &overlapping,
                    &strategy,
                );
                UpdateOutcome::Conflicted(cid)
            }
        }
        ResolutionStrategy::ManualReview => {
            if current_version != expected_version || ordering == ClockOrdering::Concurrent {
                let overlapping = detect_field_conflicts(env, record_id, changed_fields);
                let fields = if overlapping.is_empty() {
                    // Even without field overlap we queue for review under this strategy.
                    let mut v = Vec::new(env);
                    v.push_back(String::from_str(env, "*"));
                    v
                } else {
                    overlapping
                };
                let cid = enqueue_conflict(
                    env,
                    record_id,
                    provider,
                    &current_clock,
                    &caller_clock,
                    &fields,
                    &strategy,
                );
                UpdateOutcome::Conflicted(cid)
            } else {
                // Causally ordered — safe to apply.
                apply_update(env, record_id, node_id, &current_clock)
            }
        }
    }
}

// ── Internal helpers ────────────────────────────────────────────────────────

fn apply_update(
    env: &Env,
    record_id: u64,
    node_id: u32,
    current_clock: &VectorClock,
) -> UpdateOutcome {
    let stamp = bump_version(env, record_id, node_id, current_clock);
    UpdateOutcome::Applied(stamp)
}

fn bump_version(
    env: &Env,
    record_id: u64,
    node_id: u32,
    current_clock: &VectorClock,
) -> VersionStamp {
    let new_version = get_record_version(env, record_id).saturating_add(1);
    let vk = version_key(record_id);
    env.storage().persistent().set(&vk, &new_version);
    env.storage()
        .persistent()
        .extend_ttl(&vk, TTL_THRESHOLD, TTL_EXTEND_TO);

    let mut new_clock = current_clock.clone();
    new_clock.increment(env, node_id);
    let ck = clock_key(record_id);
    env.storage().persistent().set(&ck, &new_clock);
    env.storage()
        .persistent()
        .extend_ttl(&ck, TTL_THRESHOLD, TTL_EXTEND_TO);

    VersionStamp {
        version: new_version,
        clock: new_clock,
    }
}

// ── Field-level conflict detection ──────────────────────────────────────────

/// Storage key for the latest field-hash snapshot of a record.
fn field_snapshot_key(record_id: u64) -> (Symbol, u64) {
    (symbol_short!("OCC_FSNP"), record_id)
}

/// Persists the current field-hash snapshot for a record so that future
/// updates can detect which fields actually collide.
pub fn save_field_snapshot(env: &Env, record_id: u64, fields: &Vec<FieldChange>) {
    let mut snapshot: Map<String, String> = env
        .storage()
        .persistent()
        .get(&field_snapshot_key(record_id))
        .unwrap_or_else(|| Map::new(env));

    for fc in fields.iter() {
        snapshot.set(fc.field_name.clone(), fc.new_hash.clone());
    }
    env.storage()
        .persistent()
        .set(&field_snapshot_key(record_id), &snapshot);
}

/// Compares the caller's changed fields against the stored snapshot to find
/// true conflicts (same field modified by both sides).
fn detect_field_conflicts(
    env: &Env,
    record_id: u64,
    changed_fields: &Vec<FieldChange>,
) -> Vec<String> {
    let snapshot: Map<String, String> = env
        .storage()
        .persistent()
        .get(&field_snapshot_key(record_id))
        .unwrap_or_else(|| Map::new(env));

    let mut conflicts = Vec::new(env);

    for fc in changed_fields.iter() {
        if let Some(stored_hash) = snapshot.get(fc.field_name.clone()) {
            // The field exists in the snapshot. If the caller's *old* hash
            // no longer matches what is stored, the field was concurrently
            // changed by someone else.
            if stored_hash != fc.old_hash {
                conflicts.push_back(fc.field_name.clone());
            }
        }
    }

    conflicts
}

// ── Conflict queue ──────────────────────────────────────────────────────────

fn conflict_queue_key() -> Symbol {
    CONFLICT_Q
}

fn next_conflict_id(env: &Env) -> u64 {
    let current: u64 = env.storage().persistent().get(&CONFLICT_CTR).unwrap_or(0);
    let next = current.saturating_add(1);
    env.storage().persistent().set(&CONFLICT_CTR, &next);
    next
}

#[allow(clippy::too_many_arguments)]
fn enqueue_conflict(
    env: &Env,
    record_id: u64,
    provider: &Address,
    clock_a: &VectorClock,
    clock_b: &VectorClock,
    conflicting_fields: &Vec<String>,
    strategy: &ResolutionStrategy,
) -> u64 {
    let conflict_id = next_conflict_id(env);

    let entry = ConflictEntry {
        conflict_id,
        record_id,
        provider_a: provider.clone(),
        provider_b: provider.clone(),
        clock_a: clock_a.clone(),
        clock_b: clock_b.clone(),
        conflicting_fields: conflicting_fields.clone(),
        status: ConflictStatus::Pending,
        strategy: strategy.clone(),
        created_at: env.ledger().timestamp(),
        resolved_at: None,
        resolved_by: None,
    };

    let mut queue: Vec<ConflictEntry> = env
        .storage()
        .persistent()
        .get(&conflict_queue_key())
        .unwrap_or_else(|| Vec::new(env));

    queue.push_back(entry);

    // Evict oldest entries if the queue exceeds the cap.
    while queue.len() > MAX_CONFLICT_QUEUE_SIZE {
        let mut trimmed = Vec::new(env);
        for i in 1..queue.len() {
            if let Some(e) = queue.get(i) {
                trimmed.push_back(e);
            }
        }
        queue = trimmed;
    }

    env.storage()
        .persistent()
        .set(&conflict_queue_key(), &queue);

    conflict_id
}

/// Returns all pending (unresolved) conflict entries.
pub fn get_pending_conflicts(env: &Env) -> Vec<ConflictEntry> {
    let queue: Vec<ConflictEntry> = env
        .storage()
        .persistent()
        .get(&conflict_queue_key())
        .unwrap_or_else(|| Vec::new(env));

    let mut pending = Vec::new(env);
    for entry in queue.iter() {
        if entry.status == ConflictStatus::Pending {
            pending.push_back(entry);
        }
    }
    pending
}

/// Returns all conflict entries for a specific record.
pub fn get_record_conflicts(env: &Env, record_id: u64) -> Vec<ConflictEntry> {
    let queue: Vec<ConflictEntry> = env
        .storage()
        .persistent()
        .get(&conflict_queue_key())
        .unwrap_or_else(|| Vec::new(env));

    let mut result = Vec::new(env);
    for entry in queue.iter() {
        if entry.record_id == record_id {
            result.push_back(entry);
        }
    }
    result
}

/// Resolves a conflict by ID. The resolver must be an authorised address
/// (enforcement is the caller's responsibility).
pub fn resolve_conflict(env: &Env, conflict_id: u64, resolver: &Address) -> bool {
    let queue: Vec<ConflictEntry> = env
        .storage()
        .persistent()
        .get(&conflict_queue_key())
        .unwrap_or_else(|| Vec::new(env));

    let mut found = false;
    let mut updated = Vec::new(env);

    for entry in queue.iter() {
        if entry.conflict_id == conflict_id && entry.status == ConflictStatus::Pending {
            let mut resolved_entry = entry.clone();
            resolved_entry.status = ConflictStatus::Resolved;
            resolved_entry.resolved_at = Some(env.ledger().timestamp());
            resolved_entry.resolved_by = Some(resolver.clone());
            updated.push_back(resolved_entry);
            found = true;
        } else {
            updated.push_back(entry);
        }
    }

    if found {
        env.storage()
            .persistent()
            .set(&conflict_queue_key(), &updated);
    }

    found
}

/// Returns the version stamp (version + clock) for a record, useful for
/// callers that need to read the current state before issuing an update.
pub fn get_version_stamp(env: &Env, record_id: u64) -> VersionStamp {
    VersionStamp {
        version: get_record_version(env, record_id),
        clock: get_record_clock(env, record_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn init_and_clean_update() {
        let env = Env::default();
        let stamp = init_record_version(&env, 1, 10);
        assert_eq!(stamp.version, 1);
        assert_eq!(stamp.clock.get(10), 1);

        // Update with matching expected version succeeds.
        let fields = Vec::new(&env);
        let provider = Address::generate(&env);
        match compare_and_swap(&env, 1, 1, 10, &provider, &fields) {
            UpdateOutcome::Applied(s) => {
                assert_eq!(s.version, 2);
                assert_eq!(s.clock.get(10), 2);
            }
            other => panic!("expected Applied, got {:?}", other),
        }
    }

    #[test]
    fn stale_version_triggers_conflict_under_manual_review() {
        let env = Env::default();
        init_record_version(&env, 1, 10);

        // Simulate another provider advancing the version.
        let fields = Vec::new(&env);
        let provider_a = Address::generate(&env);
        let _ = compare_and_swap(&env, 1, 1, 20, &provider_a, &fields);

        // Now attempt with stale version=1, different node.
        set_resolution_strategy(&env, 1, &ResolutionStrategy::ManualReview);
        let provider_b = Address::generate(&env);
        match compare_and_swap(&env, 1, 1, 30, &provider_b, &fields) {
            UpdateOutcome::Conflicted(cid) => {
                assert!(cid > 0);
                let pending = get_pending_conflicts(&env);
                assert_eq!(pending.len(), 1);
            }
            other => panic!("expected Conflicted, got {:?}", other),
        }
    }

    #[test]
    fn last_writer_wins_always_applies() {
        let env = Env::default();
        init_record_version(&env, 1, 10);
        set_resolution_strategy(&env, 1, &ResolutionStrategy::LastWriterWins);

        let fields = Vec::new(&env);
        let provider = Address::generate(&env);
        // Advance version once.
        let _ = compare_and_swap(&env, 1, 1, 20, &provider, &fields);

        // Stale version under LWW still applies.
        match compare_and_swap(&env, 1, 1, 30, &provider, &fields) {
            UpdateOutcome::Applied(_) => {}
            other => panic!("expected Applied under LWW, got {:?}", other),
        }
    }

    #[test]
    fn merge_strategy_non_overlapping_fields() {
        let env = Env::default();
        init_record_version(&env, 1, 10);
        set_resolution_strategy(&env, 1, &ResolutionStrategy::Merge);

        // Save a field snapshot for the record.
        let mut snapshot_fields = Vec::new(&env);
        snapshot_fields.push_back(FieldChange {
            field_name: String::from_str(&env, "visual_acuity"),
            old_hash: String::from_str(&env, "hash_a"),
            new_hash: String::from_str(&env, "hash_b"),
        });
        save_field_snapshot(&env, 1, &snapshot_fields);

        // Advance version.
        let provider = Address::generate(&env);
        let empty = Vec::new(&env);
        let _ = compare_and_swap(&env, 1, 1, 20, &provider, &empty);

        // New update touches a different field — should merge.
        let mut new_fields = Vec::new(&env);
        new_fields.push_back(FieldChange {
            field_name: String::from_str(&env, "iop"),
            old_hash: String::from_str(&env, "old_iop"),
            new_hash: String::from_str(&env, "new_iop"),
        });
        match compare_and_swap(&env, 1, 1, 30, &provider, &new_fields) {
            UpdateOutcome::Merged(_) => {}
            other => panic!("expected Merged, got {:?}", other),
        }
    }

    #[test]
    fn resolve_conflict_marks_as_resolved() {
        let env = Env::default();
        init_record_version(&env, 1, 10);
        set_resolution_strategy(&env, 1, &ResolutionStrategy::ManualReview);

        let provider = Address::generate(&env);
        let fields = Vec::new(&env);
        let _ = compare_and_swap(&env, 1, 1, 20, &provider, &fields);

        match compare_and_swap(&env, 1, 1, 30, &provider, &fields) {
            UpdateOutcome::Conflicted(cid) => {
                let resolver = Address::generate(&env);
                assert!(resolve_conflict(&env, cid, &resolver));
                assert!(get_pending_conflicts(&env).is_empty());
            }
            other => panic!("expected Conflicted, got {:?}", other),
        }
    }
}
