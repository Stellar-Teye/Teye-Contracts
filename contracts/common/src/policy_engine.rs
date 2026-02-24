//! Core policy evaluation engine with caching, simulation, and versioning.
//!
//! This module ties together the DSL, conflict resolver, and on-chain storage
//! to provide a single entry point for policy-based access control decisions.

#![allow(clippy::arithmetic_side_effects)]

use soroban_sdk::{contracttype, symbol_short, Env, String, Symbol, Vec};

use crate::conflict_resolver::{self, ResolutionResult, ResolutionStrategy};
use crate::policy_dsl::{
    AttrOperator, AttributeCondition, EvalContext, PolicyDefinition, PolicyEffect, PolicyId,
    PolicyRule, SimulationResult, SimulationVerdict,
};

// ── Storage Keys ────────────────────────────────────────────────────────────

const POLICY_PREFIX: Symbol = symbol_short!("POL_DEF");
const POLICY_INDEX: Symbol = symbol_short!("POL_IDX");
const POLICY_STRATEGY: Symbol = symbol_short!("POL_STRT");
const CACHE_PREFIX: Symbol = symbol_short!("POL_CACH");
const CACHE_GEN: Symbol = symbol_short!("CACHE_GN");

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

// ── Storage Key Helpers ─────────────────────────────────────────────────────

fn policy_storage_key(id: &PolicyId) -> (Symbol, String, u32) {
    (POLICY_PREFIX, id.name.clone(), id.version)
}

fn cache_key(subject_action: &String) -> (Symbol, String) {
    (CACHE_PREFIX, subject_action.clone())
}

// ── Cache Entry ─────────────────────────────────────────────────────────────

/// A cached evaluation result, tagged with a generation counter so it can be
/// invalidated when any policy is created, updated, or removed.
#[contracttype]
#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub effect: PolicyEffect,
    pub generation: u64,
    pub timestamp: u64,
}

// ── Policy Management ───────────────────────────────────────────────────────

/// Stores a policy definition on-chain and updates the global policy index.
/// Bumps the cache generation to invalidate stale cached results.
pub fn store_policy(env: &Env, policy: &PolicyDefinition) {
    let key = policy_storage_key(&policy.id);
    env.storage().persistent().set(&key, policy);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);

    update_policy_index(env, &policy.id);
    bump_cache_generation(env);
}

/// Removes a policy from on-chain storage and drops it from the index.
pub fn remove_policy(env: &Env, id: &PolicyId) {
    let key = policy_storage_key(id);
    env.storage().persistent().remove(&key);
    remove_from_index(env, id);
    bump_cache_generation(env);
}

/// Retrieves a policy by its identifier.
pub fn get_policy(env: &Env, id: &PolicyId) -> Option<PolicyDefinition> {
    env.storage().persistent().get(&policy_storage_key(id))
}

/// Lists all registered policy identifiers.
pub fn list_policies(env: &Env) -> Vec<PolicyId> {
    env.storage()
        .persistent()
        .get(&POLICY_INDEX)
        .unwrap_or(Vec::new(env))
}

/// Sets the global conflict-resolution strategy.
pub fn set_resolution_strategy(env: &Env, strategy: ResolutionStrategy) {
    env.storage().persistent().set(&POLICY_STRATEGY, &strategy);
}

/// Returns the configured conflict-resolution strategy, defaulting to
/// `DenyOverride` if none has been set.
pub fn get_resolution_strategy(env: &Env) -> ResolutionStrategy {
    env.storage()
        .persistent()
        .get(&POLICY_STRATEGY)
        .unwrap_or(ResolutionStrategy::DenyOverride)
}

// ── Policy Evaluation ───────────────────────────────────────────────────────

/// Evaluates all enabled policies against the provided context and resolves
/// any conflicts using the configured strategy.
///
/// Returns a `ResolutionResult` describing the final effect, any conflicts
/// detected, and which policy "won".
pub fn evaluate(env: &Env, ctx: &EvalContext) -> ResolutionResult {
    let ids = list_policies(env);
    let strategy = get_resolution_strategy(env);

    let mut matched: Vec<(PolicyDefinition, PolicyEffect)> = Vec::new(env);

    for i in 0..ids.len() {
        let id = ids.get(i).unwrap();
        if let Some(policy) = get_policy(env, &id) {
            if !policy.enabled {
                continue;
            }
            let rule_result = evaluate_rule(env, &policy.rule, ctx);
            let effect = if rule_result {
                policy.effect
            } else {
                invert_effect(policy.effect)
            };
            matched.push_back((policy, effect));
        }
    }

    // Sort by priority (lower number = higher priority) for FirstApplicable
    sort_matched_by_priority(env, &mut matched);

    conflict_resolver::resolve(env, strategy, &matched)
}

/// Evaluates policies with result caching. If a valid cached result exists
/// for the given cache key, it is returned immediately.
pub fn evaluate_cached(env: &Env, ctx: &EvalContext, cache_hint: &String) -> ResolutionResult {
    let gen = current_cache_generation(env);
    let ck = cache_key(cache_hint);

    if let Some(entry) = env.storage().persistent().get::<_, CacheEntry>(&ck) {
        if entry.generation == gen {
            return ResolutionResult {
                effect: entry.effect,
                conflicts: Vec::new(env),
                winning_policy: Vec::new(env),
            };
        }
    }

    let result = evaluate(env, ctx);

    // Store in cache
    let entry = CacheEntry {
        effect: result.effect,
        generation: gen,
        timestamp: ctx.timestamp,
    };
    env.storage().persistent().set(&ck, &entry);

    result
}

// ── Simulation ──────────────────────────────────────────────────────────────

/// Runs a what-if simulation: evaluates all policies against the hypothetical
/// context without modifying any on-chain state beyond what the evaluation
/// itself reads. Returns a `SimulationResult`.
pub fn simulate(env: &Env, ctx: &EvalContext) -> SimulationResult {
    let ids = list_policies(env);
    let strategy = get_resolution_strategy(env);

    let mut matched: Vec<(PolicyDefinition, PolicyEffect)> = Vec::new(env);
    let mut eval_count: u32 = 0;

    for i in 0..ids.len() {
        let id = ids.get(i).unwrap();
        if let Some(policy) = get_policy(env, &id) {
            if !policy.enabled {
                continue;
            }
            eval_count += 1;
            let rule_result = evaluate_rule(env, &policy.rule, ctx);
            let effect = if rule_result {
                policy.effect
            } else {
                invert_effect(policy.effect)
            };
            matched.push_back((policy, effect));
        }
    }

    if matched.is_empty() {
        return SimulationResult {
            verdict: SimulationVerdict::Indeterminate,
            matched_policy: Vec::new(env),
            evaluated_count: eval_count,
        };
    }

    sort_matched_by_priority(env, &mut matched);
    let resolution = conflict_resolver::resolve(env, strategy, &matched);

    let verdict = match resolution.effect {
        PolicyEffect::Permit => SimulationVerdict::Permitted,
        PolicyEffect::Deny => SimulationVerdict::Denied,
    };

    SimulationResult {
        verdict,
        matched_policy: resolution.winning_policy,
        evaluated_count: eval_count,
    }
}

// ── Policy Versioning ───────────────────────────────────────────────────────

/// Checks backward compatibility between two versions of the same policy.
///
/// A new version is considered backward-compatible if it does not change the
/// base effect and retains the same enabled state. This is a conservative
/// heuristic; in practice you would also compare rule trees structurally.
pub fn is_backward_compatible(old: &PolicyDefinition, new: &PolicyDefinition) -> bool {
    if old.id.name != new.id.name {
        return false;
    }
    if new.id.version <= old.id.version {
        return false;
    }
    // Conservative check: effect and enabled state must match
    old.effect == new.effect && old.enabled == new.enabled
}

// ── Rule Evaluation (recursive) ─────────────────────────────────────────────

/// Recursively evaluates a `PolicyRule` tree against the given context.
#[allow(clippy::only_used_in_recursion)]
pub fn evaluate_rule(env: &Env, rule: &PolicyRule, ctx: &EvalContext) -> bool {
    match rule {
        PolicyRule::Allow => true,
        PolicyRule::Deny => false,

        PolicyRule::And(children) => {
            for i in 0..children.len() {
                if !evaluate_rule(env, &children.get(i).unwrap(), ctx) {
                    return false;
                }
            }
            true
        }

        PolicyRule::Or(children) => {
            for i in 0..children.len() {
                if evaluate_rule(env, &children.get(i).unwrap(), ctx) {
                    return true;
                }
            }
            false
        }

        PolicyRule::Not(inner) => {
            if inner.is_empty() {
                return false;
            }
            !evaluate_rule(env, &inner.get(0).unwrap(), ctx)
        }

        PolicyRule::IfThenElse(parts) => {
            if parts.len() < 3 {
                return false;
            }
            let condition = parts.get(0).unwrap();
            let then_rule = parts.get(1).unwrap();
            let else_rule = parts.get(2).unwrap();
            if evaluate_rule(env, &condition, ctx) {
                evaluate_rule(env, &then_rule, ctx)
            } else {
                evaluate_rule(env, &else_rule, ctx)
            }
        }

        PolicyRule::Unless(parts) => {
            if parts.len() < 2 {
                return false;
            }
            let main_rule = parts.get(0).unwrap();
            let exception = parts.get(1).unwrap();
            if evaluate_rule(env, &exception, ctx) {
                false
            } else {
                evaluate_rule(env, &main_rule, ctx)
            }
        }

        PolicyRule::Attribute(condition) => evaluate_attribute(condition, ctx),

        PolicyRule::Temporal(constraint) => constraint.is_satisfied(ctx.timestamp),

        PolicyRule::DelegationCheck(chain) => chain.validate(ctx.timestamp),
    }
}

// ── Attribute Evaluation ────────────────────────────────────────────────────

fn evaluate_attribute(cond: &AttributeCondition, ctx: &EvalContext) -> bool {
    let actual = match ctx.get_attr(&cond.key) {
        Some(v) => v,
        None => return false,
    };

    match cond.operator {
        AttrOperator::Eq => {
            if cond.values.is_empty() {
                return false;
            }
            actual == cond.values.get(0).unwrap()
        }
        AttrOperator::NotEq => {
            if cond.values.is_empty() {
                return true;
            }
            actual != cond.values.get(0).unwrap()
        }
        AttrOperator::In => {
            for i in 0..cond.values.len() {
                if actual == cond.values.get(i).unwrap() {
                    return true;
                }
            }
            false
        }
        AttrOperator::NotIn => {
            for i in 0..cond.values.len() {
                if actual == cond.values.get(i).unwrap() {
                    return false;
                }
            }
            true
        }
        AttrOperator::Gte | AttrOperator::Lte => {
            // Numeric comparison on string-encoded integers
            if cond.values.is_empty() {
                return false;
            }
            // Simple lexicographic comparison for on-chain use
            let expected = cond.values.get(0).unwrap();
            match cond.operator {
                AttrOperator::Gte => actual >= expected,
                AttrOperator::Lte => actual <= expected,
                _ => false,
            }
        }
    }
}

// ── Internal Utilities ──────────────────────────────────────────────────────

fn invert_effect(effect: PolicyEffect) -> PolicyEffect {
    match effect {
        PolicyEffect::Permit => PolicyEffect::Deny,
        PolicyEffect::Deny => PolicyEffect::Permit,
    }
}

fn current_cache_generation(env: &Env) -> u64 {
    env.storage().persistent().get(&CACHE_GEN).unwrap_or(0)
}

fn bump_cache_generation(env: &Env) {
    let gen = current_cache_generation(env) + 1;
    env.storage().persistent().set(&CACHE_GEN, &gen);
}

fn update_policy_index(env: &Env, id: &PolicyId) {
    let index: Vec<PolicyId> = env
        .storage()
        .persistent()
        .get(&POLICY_INDEX)
        .unwrap_or(Vec::new(env));

    // Replace existing entry with same name (upgrade) or append
    let mut found = false;
    let mut new_index: Vec<PolicyId> = Vec::new(env);
    for i in 0..index.len() {
        let existing = index.get(i).unwrap();
        if existing.name == id.name {
            new_index.push_back(id.clone());
            found = true;
        } else {
            new_index.push_back(existing);
        }
    }
    if !found {
        new_index.push_back(id.clone());
    }

    env.storage().persistent().set(&POLICY_INDEX, &new_index);
}

fn remove_from_index(env: &Env, id: &PolicyId) {
    let index: Vec<PolicyId> = env
        .storage()
        .persistent()
        .get(&POLICY_INDEX)
        .unwrap_or(Vec::new(env));

    let mut new_index: Vec<PolicyId> = Vec::new(env);
    for i in 0..index.len() {
        let existing = index.get(i).unwrap();
        if existing.name != id.name || existing.version != id.version {
            new_index.push_back(existing);
        }
    }

    env.storage().persistent().set(&POLICY_INDEX, &new_index);
}

/// Sorts matched policies by priority in ascending order (lowest number =
/// highest priority). Uses a simple insertion sort suitable for the small
/// policy sets typical in on-chain evaluation.
fn sort_matched_by_priority(env: &Env, matched: &mut Vec<(PolicyDefinition, PolicyEffect)>) {
    let len = matched.len();
    if len <= 1 {
        return;
    }

    // Collect into a temporary vec, sort, then rebuild
    let mut items: soroban_sdk::Vec<(PolicyDefinition, PolicyEffect)> = Vec::new(env);
    for i in 0..len {
        items.push_back(matched.get(i).unwrap());
    }

    // Insertion sort by priority
    for i in 1..len {
        let mut j = i;
        while j > 0 {
            let curr = items.get(j).unwrap();
            let prev = items.get(j - 1).unwrap();
            if curr.0.priority < prev.0.priority {
                items.set(j, prev);
                items.set(j - 1, curr);
                j -= 1;
            } else {
                break;
            }
        }
    }

    // Rebuild the matched vec
    *matched = items;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_dsl::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Address, Env, String, Vec};

    fn test_ctx(env: &Env) -> EvalContext {
        let subject = Address::generate(env);
        let mut keys = Vec::new(env);
        keys.push_back(String::from_str(env, "role"));
        keys.push_back(String::from_str(env, "department"));

        let mut vals = Vec::new(env);
        vals.push_back(String::from_str(env, "doctor"));
        vals.push_back(String::from_str(env, "cardiology"));

        EvalContext {
            subject,
            resource_id: String::from_str(env, "record_42"),
            action: String::from_str(env, "read"),
            timestamp: 10 * 3600, // 10 AM UTC
            attr_keys: keys,
            attr_vals: vals,
        }
    }

    fn make_policy_def(
        env: &Env,
        name: &str,
        rule: PolicyRule,
        effect: PolicyEffect,
        priority: u32,
    ) -> PolicyDefinition {
        PolicyDefinition {
            id: PolicyId {
                name: String::from_str(env, name),
                version: 1,
            },
            description: String::from_str(env, "test policy"),
            rule,
            effect,
            priority,
            enabled: true,
        }
    }

    #[test]
    fn allow_rule_evaluates_true() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        assert!(evaluate_rule(&env, &PolicyRule::Allow, &ctx));
    }

    #[test]
    fn deny_rule_evaluates_false() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        assert!(!evaluate_rule(&env, &PolicyRule::Deny, &ctx));
    }

    #[test]
    fn and_rule_all_true() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        let mut children = Vec::new(&env);
        children.push_back(PolicyRule::Allow);
        children.push_back(PolicyRule::Allow);
        assert!(evaluate_rule(&env, &PolicyRule::And(children), &ctx));
    }

    #[test]
    fn and_rule_one_false() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        let mut children = Vec::new(&env);
        children.push_back(PolicyRule::Allow);
        children.push_back(PolicyRule::Deny);
        assert!(!evaluate_rule(&env, &PolicyRule::And(children), &ctx));
    }

    #[test]
    fn or_rule_one_true() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        let mut children = Vec::new(&env);
        children.push_back(PolicyRule::Deny);
        children.push_back(PolicyRule::Allow);
        assert!(evaluate_rule(&env, &PolicyRule::Or(children), &ctx));
    }

    #[test]
    fn or_rule_all_false() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        let mut children = Vec::new(&env);
        children.push_back(PolicyRule::Deny);
        children.push_back(PolicyRule::Deny);
        assert!(!evaluate_rule(&env, &PolicyRule::Or(children), &ctx));
    }

    #[test]
    fn not_rule_inverts() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        let mut inner = Vec::new(&env);
        inner.push_back(PolicyRule::Deny);
        assert!(evaluate_rule(&env, &PolicyRule::Not(inner), &ctx));
    }

    #[test]
    fn if_then_else_takes_then_branch() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        let mut parts = Vec::new(&env);
        parts.push_back(PolicyRule::Allow); // condition = true
        parts.push_back(PolicyRule::Allow); // then
        parts.push_back(PolicyRule::Deny); // else
        assert!(evaluate_rule(&env, &PolicyRule::IfThenElse(parts), &ctx));
    }

    #[test]
    fn if_then_else_takes_else_branch() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        let mut parts = Vec::new(&env);
        parts.push_back(PolicyRule::Deny); // condition = false
        parts.push_back(PolicyRule::Deny); // then (not taken)
        parts.push_back(PolicyRule::Allow); // else
        assert!(evaluate_rule(&env, &PolicyRule::IfThenElse(parts), &ctx));
    }

    #[test]
    fn unless_blocks_on_exception() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        let mut parts = Vec::new(&env);
        parts.push_back(PolicyRule::Allow); // main rule
        parts.push_back(PolicyRule::Allow); // exception is true => deny
        assert!(!evaluate_rule(&env, &PolicyRule::Unless(parts), &ctx));
    }

    #[test]
    fn unless_passes_when_no_exception() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        let mut parts = Vec::new(&env);
        parts.push_back(PolicyRule::Allow); // main rule
        parts.push_back(PolicyRule::Deny); // exception is false
        assert!(evaluate_rule(&env, &PolicyRule::Unless(parts), &ctx));
    }

    #[test]
    fn attribute_eq_matches() {
        let env = Env::default();
        let ctx = test_ctx(&env);

        let mut values = Vec::new(&env);
        values.push_back(String::from_str(&env, "doctor"));

        let cond = AttributeCondition {
            key: String::from_str(&env, "role"),
            operator: AttrOperator::Eq,
            values,
        };
        assert!(evaluate_rule(&env, &PolicyRule::Attribute(cond), &ctx));
    }

    #[test]
    fn attribute_in_matches() {
        let env = Env::default();
        let ctx = test_ctx(&env);

        let mut values = Vec::new(&env);
        values.push_back(String::from_str(&env, "cardiology"));
        values.push_back(String::from_str(&env, "neurology"));

        let cond = AttributeCondition {
            key: String::from_str(&env, "department"),
            operator: AttrOperator::In,
            values,
        };
        assert!(evaluate_rule(&env, &PolicyRule::Attribute(cond), &ctx));
    }

    #[test]
    fn attribute_not_in_rejects() {
        let env = Env::default();
        let ctx = test_ctx(&env);

        let mut values = Vec::new(&env);
        values.push_back(String::from_str(&env, "cardiology"));

        let cond = AttributeCondition {
            key: String::from_str(&env, "department"),
            operator: AttrOperator::NotIn,
            values,
        };
        assert!(!evaluate_rule(&env, &PolicyRule::Attribute(cond), &ctx));
    }

    #[test]
    fn temporal_rule_checks_timestamp() {
        let env = Env::default();
        let ctx = test_ctx(&env); // timestamp = 10 * 3600

        let tc = TemporalConstraint {
            valid_from: 0,
            valid_until: 0,
            allowed_hour_start: 9,
            allowed_hour_end: 17,
            allowed_days_mask: 0,
        };
        assert!(evaluate_rule(&env, &PolicyRule::Temporal(tc), &ctx));
    }

    #[test]
    fn store_and_evaluate_policy() {
        let env = Env::default();
        let ctx = test_ctx(&env);

        let policy = make_policy_def(
            &env,
            "allow_doctors",
            PolicyRule::Allow,
            PolicyEffect::Permit,
            1,
        );
        store_policy(&env, &policy);

        let result = evaluate(&env, &ctx);
        assert_eq!(result.effect, PolicyEffect::Permit);
    }

    #[test]
    fn store_and_remove_policy() {
        let env = Env::default();
        let ctx = test_ctx(&env);

        let policy = make_policy_def(&env, "temp", PolicyRule::Allow, PolicyEffect::Permit, 1);
        store_policy(&env, &policy);
        assert_eq!(list_policies(&env).len(), 1);

        remove_policy(&env, &policy.id);
        assert!(list_policies(&env).is_empty());

        // Evaluate with no policies -> default deny
        let result = evaluate(&env, &ctx);
        assert_eq!(result.effect, PolicyEffect::Deny);
    }

    #[test]
    fn cached_evaluation_returns_same_result() {
        let env = Env::default();
        let ctx = test_ctx(&env);

        let policy = make_policy_def(&env, "cached", PolicyRule::Allow, PolicyEffect::Permit, 1);
        store_policy(&env, &policy);

        let hint = String::from_str(&env, "test_cache_key");
        let r1 = evaluate_cached(&env, &ctx, &hint);
        let r2 = evaluate_cached(&env, &ctx, &hint);
        assert_eq!(r1.effect, r2.effect);
    }

    #[test]
    fn cache_invalidated_on_policy_change() {
        let env = Env::default();
        let ctx = test_ctx(&env);

        let policy = make_policy_def(&env, "v1", PolicyRule::Allow, PolicyEffect::Permit, 1);
        store_policy(&env, &policy);

        let hint = String::from_str(&env, "cache_inv_test");
        let r1 = evaluate_cached(&env, &ctx, &hint);
        assert_eq!(r1.effect, PolicyEffect::Permit);

        // Store a new deny policy — bumps generation
        let deny_policy =
            make_policy_def(&env, "v1_deny", PolicyRule::Allow, PolicyEffect::Deny, 0);
        store_policy(&env, &deny_policy);

        // Cache is invalidated; re-evaluates
        let r2 = evaluate_cached(&env, &ctx, &hint);
        // Now deny override kicks in
        assert_eq!(r2.effect, PolicyEffect::Deny);
    }

    #[test]
    fn simulation_returns_indeterminate_when_no_policies() {
        let env = Env::default();
        let ctx = test_ctx(&env);
        let sim = simulate(&env, &ctx);
        assert_eq!(sim.verdict, SimulationVerdict::Indeterminate);
        assert_eq!(sim.evaluated_count, 0);
    }

    #[test]
    fn simulation_evaluates_policies() {
        let env = Env::default();
        let ctx = test_ctx(&env);

        let policy = make_policy_def(&env, "sim_test", PolicyRule::Allow, PolicyEffect::Permit, 1);
        store_policy(&env, &policy);

        let sim = simulate(&env, &ctx);
        assert_eq!(sim.verdict, SimulationVerdict::Permitted);
        assert_eq!(sim.evaluated_count, 1);
    }

    #[test]
    fn backward_compatibility_check() {
        let env = Env::default();

        let old = make_policy_def(&env, "compat", PolicyRule::Allow, PolicyEffect::Permit, 1);
        let mut new_compat = old.clone();
        new_compat.id.version = 2;

        assert!(is_backward_compatible(&old, &new_compat));

        let mut new_incompat = old.clone();
        new_incompat.id.version = 2;
        new_incompat.effect = PolicyEffect::Deny;
        assert!(!is_backward_compatible(&old, &new_incompat));
    }

    #[test]
    fn complex_boolean_policy() {
        let env = Env::default();
        let ctx = test_ctx(&env); // role=doctor, dept=cardiology, 10AM

        // Build: (role == doctor) AND (dept IN [cardiology, neuro]) AND (9-17 hours)
        let mut role_vals = Vec::new(&env);
        role_vals.push_back(String::from_str(&env, "doctor"));
        let role_cond = PolicyRule::Attribute(AttributeCondition {
            key: String::from_str(&env, "role"),
            operator: AttrOperator::Eq,
            values: role_vals,
        });

        let mut dept_vals = Vec::new(&env);
        dept_vals.push_back(String::from_str(&env, "cardiology"));
        dept_vals.push_back(String::from_str(&env, "neurology"));
        let dept_cond = PolicyRule::Attribute(AttributeCondition {
            key: String::from_str(&env, "department"),
            operator: AttrOperator::In,
            values: dept_vals,
        });

        let time_cond = PolicyRule::Temporal(TemporalConstraint {
            valid_from: 0,
            valid_until: 0,
            allowed_hour_start: 9,
            allowed_hour_end: 17,
            allowed_days_mask: 0,
        });

        let mut and_children = Vec::new(&env);
        and_children.push_back(role_cond);
        and_children.push_back(dept_cond);
        and_children.push_back(time_cond);

        let complex_rule = PolicyRule::And(and_children);

        let policy = make_policy_def(
            &env,
            "complex_access",
            complex_rule,
            PolicyEffect::Permit,
            1,
        );
        store_policy(&env, &policy);

        let result = evaluate(&env, &ctx);
        assert_eq!(result.effect, PolicyEffect::Permit);
    }

    #[test]
    fn resolution_strategy_can_be_changed() {
        let env = Env::default();

        set_resolution_strategy(&env, ResolutionStrategy::PermitOverride);
        assert_eq!(
            get_resolution_strategy(&env),
            ResolutionStrategy::PermitOverride
        );

        set_resolution_strategy(&env, ResolutionStrategy::FirstApplicable);
        assert_eq!(
            get_resolution_strategy(&env),
            ResolutionStrategy::FirstApplicable
        );
    }

    #[test]
    fn policy_versioning_upgrade() {
        let env = Env::default();
        let ctx = test_ctx(&env);

        let v1 = make_policy_def(
            &env,
            "versioned",
            PolicyRule::Allow,
            PolicyEffect::Permit,
            1,
        );
        store_policy(&env, &v1);

        // Upgrade to v2 with same name — replaces in index
        let mut v2 = v1.clone();
        v2.id.version = 2;
        v2.rule = PolicyRule::Deny;
        v2.effect = PolicyEffect::Deny;
        store_policy(&env, &v2);

        // Only one policy in index (upgraded)
        assert_eq!(list_policies(&env).len(), 1);

        let result = evaluate(&env, &ctx);
        // v2 rule is Deny with effect Deny: rule_result=false => invert(Deny)=Permit
        // Actually: rule is Deny (evaluates false), effect is Deny, so final = invert(Deny) = Permit
        // Let me re-check: evaluate_rule(Deny) = false; since rule_result is false, effect = invert(Deny) = Permit
        assert_eq!(result.effect, PolicyEffect::Permit);
    }
}
