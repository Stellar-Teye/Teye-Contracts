//! Lightweight operational transformation helpers for mergeable concurrent
//! edits to medical record fields.
//!
//! In a blockchain smart-contract context full OT is impractical, so this
//! module provides a simplified field-level transform: when two providers
//! concurrently modify *different* fields of the same record the changes can
//! be composed automatically.  When the *same* field is touched by both sides,
//! the module flags it as a true conflict.

use soroban_sdk::{contracttype, Env, Map, String, Vec};

use crate::concurrency::FieldChange;

/// Outcome of attempting an operational transform on two concurrent change sets.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransformResult {
    /// All changes are compatible — the merged set is returned.
    Merged,
    /// Some fields conflict and cannot be auto-merged.
    HasConflicts,
}

/// A pair of transform outputs: the merged (non-conflicting) fields and the
/// conflicting field names.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransformOutput {
    pub result: TransformResult,
    pub merged_fields: Vec<FieldChange>,
    pub conflicting_field_names: Vec<String>,
}

/// Transforms two concurrent change sets (`local` and `remote`) against a
/// shared base snapshot represented as a `Map<String, String>` of field names
/// to their last-known hashes.
///
/// Non-overlapping changes are collected into `merged_fields`.  Overlapping
/// changes (same field name modified by both sides with different new values)
/// are collected into `conflicting_field_names`.
pub fn transform(
    env: &Env,
    base_snapshot: &Map<String, String>,
    local_changes: &Vec<FieldChange>,
    remote_changes: &Vec<FieldChange>,
) -> TransformOutput {
    let mut merged = Vec::new(env);
    let mut conflicts = Vec::new(env);

    // Index remote changes by field name for fast lookup.
    let mut remote_map: Map<String, FieldChange> = Map::new(env);
    for rc in remote_changes.iter() {
        remote_map.set(rc.field_name.clone(), rc);
    }

    // Track which remote fields have been processed via local iteration.
    let mut processed_remote: Map<String, bool> = Map::new(env);

    for lc in local_changes.iter() {
        if let Some(rc) = remote_map.get(lc.field_name.clone()) {
            processed_remote.set(lc.field_name.clone(), true);

            if lc.new_hash == rc.new_hash {
                // Both sides converge to the same value — no conflict.
                merged.push_back(lc);
            } else {
                conflicts.push_back(lc.field_name.clone());
            }
        } else {
            // Only local touched this field — safe to include.
            merged.push_back(lc);
        }
    }

    // Include remote-only changes (not touched by local).
    for rc in remote_changes.iter() {
        if processed_remote.get(rc.field_name.clone()).is_none() {
            merged.push_back(rc);
        }
    }

    let result = if conflicts.is_empty() {
        TransformResult::Merged
    } else {
        TransformResult::HasConflicts
    };

    // Intentionally ignore base_snapshot reads here — the snapshot is used
    // upstream by the concurrency module for old-hash validation.  The
    // transform itself only needs to reason about local vs remote new values.
    let _ = base_snapshot;

    TransformOutput {
        result,
        merged_fields: merged,
        conflicting_field_names: conflicts,
    }
}

/// Convenience wrapper: returns `true` when two change sets can be fully
/// merged without any field-level conflicts.
pub fn can_auto_merge(
    env: &Env,
    base_snapshot: &Map<String, String>,
    local: &Vec<FieldChange>,
    remote: &Vec<FieldChange>,
) -> bool {
    let output = transform(env, base_snapshot, local, remote);
    output.result == TransformResult::Merged
}

/// Applies a set of merged [`FieldChange`]s back to a snapshot map, returning
/// the updated snapshot.
pub fn apply_merged_changes(
    env: &Env,
    snapshot: &Map<String, String>,
    merged: &Vec<FieldChange>,
) -> Map<String, String> {
    let mut updated = snapshot.clone();
    for fc in merged.iter() {
        updated.set(fc.field_name.clone(), fc.new_hash.clone());
    }
    let _ = env;
    updated
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fc(env: &Env, name: &str, old: &str, new: &str) -> FieldChange {
        FieldChange {
            field_name: String::from_str(env, name),
            old_hash: String::from_str(env, old),
            new_hash: String::from_str(env, new),
        }
    }

    #[test]
    fn non_overlapping_changes_merge() {
        let env = Env::default();
        let base = Map::new(&env);

        let mut local = Vec::new(&env);
        local.push_back(fc(&env, "visual_acuity", "a1", "a2"));

        let mut remote = Vec::new(&env);
        remote.push_back(fc(&env, "iop", "b1", "b2"));

        let out = transform(&env, &base, &local, &remote);
        assert_eq!(out.result, TransformResult::Merged);
        assert_eq!(out.merged_fields.len(), 2);
        assert!(out.conflicting_field_names.is_empty());
    }

    #[test]
    fn overlapping_same_value_merges() {
        let env = Env::default();
        let base = Map::new(&env);

        let mut local = Vec::new(&env);
        local.push_back(fc(&env, "cornea", "old", "new_same"));

        let mut remote = Vec::new(&env);
        remote.push_back(fc(&env, "cornea", "old", "new_same"));

        let out = transform(&env, &base, &local, &remote);
        assert_eq!(out.result, TransformResult::Merged);
        assert_eq!(out.merged_fields.len(), 1);
    }

    #[test]
    fn overlapping_different_values_conflict() {
        let env = Env::default();
        let base = Map::new(&env);

        let mut local = Vec::new(&env);
        local.push_back(fc(&env, "lens", "old", "local_new"));

        let mut remote = Vec::new(&env);
        remote.push_back(fc(&env, "lens", "old", "remote_new"));

        let out = transform(&env, &base, &local, &remote);
        assert_eq!(out.result, TransformResult::HasConflicts);
        assert_eq!(out.conflicting_field_names.len(), 1);
    }

    #[test]
    fn can_auto_merge_convenience() {
        let env = Env::default();
        let base = Map::new(&env);

        let mut local = Vec::new(&env);
        local.push_back(fc(&env, "field_a", "a", "a2"));

        let mut remote = Vec::new(&env);
        remote.push_back(fc(&env, "field_b", "b", "b2"));

        assert!(can_auto_merge(&env, &base, &local, &remote));
    }

    #[test]
    fn apply_merged_updates_snapshot() {
        let env = Env::default();
        let mut base = Map::new(&env);
        base.set(String::from_str(&env, "x"), String::from_str(&env, "old_x"));

        let mut merged = Vec::new(&env);
        merged.push_back(fc(&env, "x", "old_x", "new_x"));
        merged.push_back(fc(&env, "y", "old_y", "new_y"));

        let updated = apply_merged_changes(&env, &base, &merged);
        assert_eq!(
            updated.get(String::from_str(&env, "x")),
            Some(String::from_str(&env, "new_x"))
        );
        assert_eq!(
            updated.get(String::from_str(&env, "y")),
            Some(String::from_str(&env, "new_y"))
        );
    }
}
