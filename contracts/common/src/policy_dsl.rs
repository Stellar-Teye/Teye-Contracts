//! Policy Domain Specific Language (DSL) for composable access control.
//!
//! Provides on-chain data structures that express complex, composable access
//! control policies using boolean logic, temporal constraints, attribute
//! conditions, and delegation chains.

#![allow(clippy::arithmetic_side_effects)]

use soroban_sdk::{contracttype, Address, Env, String, Vec};

// ── Policy Identifiers ──────────────────────────────────────────────────────

/// Unique identifier for a policy, combining a human-readable name with a
/// monotonically increasing version number.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyId {
    pub name: String,
    pub version: u32,
}

// ── Attribute Conditions ────────────────────────────────────────────────────

/// Comparison operators for attribute-based conditions.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum AttrOperator {
    Eq = 1,
    NotEq = 2,
    In = 3,
    NotIn = 4,
    Gte = 5,
    Lte = 6,
}

/// An attribute condition that compares a named attribute against one or more
/// expected values.
///
/// Examples (conceptual):
/// - `role == "doctor"`        → key="role", op=Eq, values=["doctor"]
/// - `dept IN ["cardio","neuro"]` → key="dept", op=In, values=["cardio","neuro"]
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttributeCondition {
    pub key: String,
    pub operator: AttrOperator,
    pub values: Vec<String>,
}

// ── Temporal Constraints ────────────────────────────────────────────────────

/// Time-based constraints for policy evaluation.
///
/// All timestamps are UNIX epoch seconds. Hour values are 0-23 UTC.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalConstraint {
    /// Policy is not valid before this timestamp (0 = no lower bound).
    pub valid_from: u64,
    /// Policy is not valid after this timestamp (0 = no upper bound).
    pub valid_until: u64,
    /// Earliest hour of day (UTC) during which the policy applies (0-23).
    pub allowed_hour_start: u32,
    /// Latest hour of day (UTC) during which the policy applies (0-23).
    pub allowed_hour_end: u32,
    /// Bitmask for allowed days of week (bit 0 = Sunday … bit 6 = Saturday).
    /// 0 means all days are allowed.
    pub allowed_days_mask: u32,
}

impl TemporalConstraint {
    /// Returns a constraint that imposes no restrictions.
    pub fn unrestricted(env: &Env) -> Self {
        let _ = env; // keep consistent with Soroban patterns
        TemporalConstraint {
            valid_from: 0,
            valid_until: 0,
            allowed_hour_start: 0,
            allowed_hour_end: 23,
            allowed_days_mask: 0,
        }
    }

    /// Checks whether the given ledger timestamp satisfies this constraint.
    pub fn is_satisfied(&self, timestamp: u64) -> bool {
        // Date-range check
        if self.valid_from > 0 && timestamp < self.valid_from {
            return false;
        }
        if self.valid_until > 0 && timestamp > self.valid_until {
            return false;
        }

        // Hour-of-day check
        let hour = ((timestamp / 3600) % 24) as u32;
        if self.allowed_hour_start <= self.allowed_hour_end {
            if hour < self.allowed_hour_start || hour > self.allowed_hour_end {
                return false;
            }
        } else {
            // Overnight range (e.g. 22-06)
            if hour < self.allowed_hour_start && hour > self.allowed_hour_end {
                return false;
            }
        }

        // Day-of-week check (0 mask = all days allowed)
        if self.allowed_days_mask != 0 {
            let day_of_week = ((timestamp / 86400) + 4) % 7; // epoch was Thursday
            if (self.allowed_days_mask & (1 << day_of_week)) == 0 {
                return false;
            }
        }

        true
    }
}

// ── Delegation Chain ────────────────────────────────────────────────────────

/// A single link in a delegation chain, capturing who delegated what to whom
/// with an optional scope restriction and expiry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DelegationLink {
    pub delegator: Address,
    pub delegatee: Address,
    /// The set of permission names this link carries. Each subsequent link in
    /// the chain may only narrow (never widen) the scope.
    pub scoped_permissions: Vec<String>,
    pub expires_at: u64,
}

/// A full delegation chain from the original authority to the final actor.
/// Scope must narrow (or stay equal) at each successive link.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DelegationChain {
    pub links: Vec<DelegationLink>,
}

impl DelegationChain {
    /// Validates that the chain is well-formed:
    /// 1. Each link's delegatee matches the next link's delegator.
    /// 2. Permissions narrow (or stay equal) at each step.
    /// 3. No link has expired relative to the provided `now` timestamp.
    pub fn validate(&self, now: u64) -> bool {
        if self.links.is_empty() {
            return false;
        }

        let mut prev_perms_count: Option<u32> = None;

        for i in 0..self.links.len() {
            let link = self.links.get(i).unwrap();

            // Expiry check
            if link.expires_at > 0 && link.expires_at <= now {
                return false;
            }

            // Chain continuity: delegatee[i] == delegator[i+1]
            if i + 1 < self.links.len() {
                let next = self.links.get(i + 1).unwrap();
                if link.delegatee != next.delegator {
                    return false;
                }
            }

            // Scope narrowing: permission count must not increase
            let perm_count = link.scoped_permissions.len();
            if let Some(prev) = prev_perms_count {
                if perm_count > prev {
                    return false;
                }
            }
            prev_perms_count = Some(perm_count);
        }

        true
    }

    /// Returns the effective permissions at the end of the chain — i.e. the
    /// permissions carried by the last link.
    pub fn effective_permissions(&self) -> Option<Vec<String>> {
        if self.links.is_empty() {
            return None;
        }
        let last_idx = self.links.len() - 1;
        Some(self.links.get(last_idx).unwrap().scoped_permissions.clone())
    }
}

// ── Policy Rule (the DSL expression tree) ───────────────────────────────────

/// The core DSL node. Policies are composed as a tree of `PolicyRule` nodes,
/// evaluated recursively by the policy engine.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyRule {
    /// Unconditionally permits access.
    Allow,
    /// Unconditionally denies access.
    Deny,
    /// True when ALL child rules evaluate to true.
    And(Vec<PolicyRule>),
    /// True when ANY child rule evaluates to true.
    Or(Vec<PolicyRule>),
    /// Inverts the result of the inner rule.
    Not(Vec<PolicyRule>),
    /// Evaluates to `then_rule` when `condition` is true, else `else_rule`.
    /// Encoded as a 3-element Vec: [condition, then_rule, else_rule].
    IfThenElse(Vec<PolicyRule>),
    /// Evaluates to `rule` UNLESS `exception` is true. If the exception
    /// matches, the result is Deny.
    /// Encoded as a 2-element Vec: [rule, exception].
    Unless(Vec<PolicyRule>),
    /// Checks an attribute condition against the evaluation context.
    Attribute(AttributeCondition),
    /// Checks a temporal constraint against the current ledger timestamp.
    Temporal(TemporalConstraint),
    /// Validates a delegation chain.
    DelegationCheck(DelegationChain),
}

// ── Top-level Policy Definition ─────────────────────────────────────────────

/// The effect of a policy evaluation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum PolicyEffect {
    Permit = 1,
    Deny = 2,
}

/// A complete, versioned policy definition stored on-chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyDefinition {
    pub id: PolicyId,
    pub description: String,
    pub rule: PolicyRule,
    pub effect: PolicyEffect,
    pub priority: u32,
    pub enabled: bool,
}

// ── Evaluation Context ──────────────────────────────────────────────────────

/// Runtime context supplied to the policy engine during evaluation.
#[contracttype]
#[derive(Clone, Debug)]
pub struct EvalContext {
    pub subject: Address,
    pub resource_id: String,
    pub action: String,
    pub timestamp: u64,
    /// Key-value attribute pairs for the subject (e.g. role, department).
    pub attr_keys: Vec<String>,
    pub attr_vals: Vec<String>,
}

impl EvalContext {
    /// Looks up an attribute value by key. Returns `None` if not found.
    pub fn get_attr(&self, key: &String) -> Option<String> {
        for i in 0..self.attr_keys.len() {
            if self.attr_keys.get(i).unwrap() == *key {
                return Some(self.attr_vals.get(i).unwrap());
            }
        }
        None
    }
}

// ── Simulation Result ───────────────────────────────────────────────────────

/// The outcome of a policy simulation (what-if evaluation without side effects).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum SimulationVerdict {
    Permitted = 1,
    Denied = 2,
    Indeterminate = 3,
}

/// Detailed result from a policy simulation run.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SimulationResult {
    pub verdict: SimulationVerdict,
    /// Empty vec = no match; single element = the winning policy.
    pub matched_policy: Vec<PolicyId>,
    pub evaluated_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    #[test]
    fn temporal_constraint_unrestricted_always_passes() {
        let env = Env::default();
        let tc = TemporalConstraint::unrestricted(&env);
        assert!(tc.is_satisfied(0));
        assert!(tc.is_satisfied(1_700_000_000));
    }

    #[test]
    fn temporal_constraint_valid_range() {
        let env = Env::default();
        let _ = &env;
        let tc = TemporalConstraint {
            valid_from: 1000,
            valid_until: 2000,
            allowed_hour_start: 0,
            allowed_hour_end: 23,
            allowed_days_mask: 0,
        };
        assert!(!tc.is_satisfied(999));
        assert!(tc.is_satisfied(1500));
        assert!(!tc.is_satisfied(2001));
    }

    #[test]
    fn temporal_constraint_hour_range() {
        let env = Env::default();
        let _ = &env;
        let tc = TemporalConstraint {
            valid_from: 0,
            valid_until: 0,
            allowed_hour_start: 9,
            allowed_hour_end: 17,
            allowed_days_mask: 0,
        };
        // 9 AM UTC on epoch day
        let nine_am = 9 * 3600;
        assert!(tc.is_satisfied(nine_am));
        // 8 AM — outside range
        let eight_am = 8 * 3600;
        assert!(!tc.is_satisfied(eight_am));
        // 5 PM — inside range
        let five_pm = 17 * 3600;
        assert!(tc.is_satisfied(five_pm));
        // 6 PM — outside
        let six_pm = 18 * 3600;
        assert!(!tc.is_satisfied(six_pm));
    }

    #[test]
    fn temporal_constraint_overnight_hour_range() {
        let env = Env::default();
        let _ = &env;
        let tc = TemporalConstraint {
            valid_from: 0,
            valid_until: 0,
            allowed_hour_start: 22,
            allowed_hour_end: 6,
            allowed_days_mask: 0,
        };
        let ten_pm = 22 * 3600;
        assert!(tc.is_satisfied(ten_pm));
        let two_am = 2 * 3600;
        assert!(tc.is_satisfied(two_am));
        let noon = 12 * 3600;
        assert!(!tc.is_satisfied(noon));
    }

    #[test]
    fn temporal_constraint_day_mask() {
        let env = Env::default();
        let _ = &env;
        // Only Monday (bit 1)
        let tc = TemporalConstraint {
            valid_from: 0,
            valid_until: 0,
            allowed_hour_start: 0,
            allowed_hour_end: 23,
            allowed_days_mask: 0b0000010,
        };
        // Unix epoch (Thu Jan 1 1970) => day 4 (Thursday). We need a Monday.
        // Day 4 of epoch week = Thursday. Monday is day 1.
        // Seconds for Monday Jan 5 1970 00:00 UTC = 4 * 86400
        let monday = 4 * 86400;
        assert!(tc.is_satisfied(monday));
        // Tuesday Jan 6 = 5 * 86400
        let tuesday = 5 * 86400;
        assert!(!tc.is_satisfied(tuesday));
    }

    #[test]
    fn delegation_chain_validates_continuity() {
        let env = Env::default();
        let a = Address::generate(&env);
        let b = Address::generate(&env);
        let c = Address::generate(&env);

        let mut perms_ab = Vec::new(&env);
        perms_ab.push_back(String::from_str(&env, "read"));
        perms_ab.push_back(String::from_str(&env, "write"));

        let mut perms_bc = Vec::new(&env);
        perms_bc.push_back(String::from_str(&env, "read"));

        let mut links = Vec::new(&env);
        links.push_back(DelegationLink {
            delegator: a.clone(),
            delegatee: b.clone(),
            scoped_permissions: perms_ab,
            expires_at: 0,
        });
        links.push_back(DelegationLink {
            delegator: b.clone(),
            delegatee: c.clone(),
            scoped_permissions: perms_bc,
            expires_at: 0,
        });

        let chain = DelegationChain { links };
        assert!(chain.validate(100));
    }

    #[test]
    fn delegation_chain_rejects_scope_widening() {
        let env = Env::default();
        let a = Address::generate(&env);
        let b = Address::generate(&env);
        let c = Address::generate(&env);

        let mut perms_ab = Vec::new(&env);
        perms_ab.push_back(String::from_str(&env, "read"));

        let mut perms_bc = Vec::new(&env);
        perms_bc.push_back(String::from_str(&env, "read"));
        perms_bc.push_back(String::from_str(&env, "write"));

        let mut links = Vec::new(&env);
        links.push_back(DelegationLink {
            delegator: a.clone(),
            delegatee: b.clone(),
            scoped_permissions: perms_ab,
            expires_at: 0,
        });
        links.push_back(DelegationLink {
            delegator: b.clone(),
            delegatee: c.clone(),
            scoped_permissions: perms_bc,
            expires_at: 0,
        });

        let chain = DelegationChain { links };
        assert!(!chain.validate(100));
    }

    #[test]
    fn delegation_chain_rejects_expired_link() {
        let env = Env::default();
        let a = Address::generate(&env);
        let b = Address::generate(&env);

        let mut perms = Vec::new(&env);
        perms.push_back(String::from_str(&env, "read"));

        let mut links = Vec::new(&env);
        links.push_back(DelegationLink {
            delegator: a.clone(),
            delegatee: b.clone(),
            scoped_permissions: perms,
            expires_at: 50,
        });

        let chain = DelegationChain { links };
        assert!(!chain.validate(100));
    }

    #[test]
    fn delegation_chain_rejects_broken_continuity() {
        let env = Env::default();
        let a = Address::generate(&env);
        let b = Address::generate(&env);
        let c = Address::generate(&env);
        let d = Address::generate(&env);

        let mut perms = Vec::new(&env);
        perms.push_back(String::from_str(&env, "read"));

        let mut links = Vec::new(&env);
        links.push_back(DelegationLink {
            delegator: a.clone(),
            delegatee: b.clone(),
            scoped_permissions: perms.clone(),
            expires_at: 0,
        });
        // Gap: c != b
        links.push_back(DelegationLink {
            delegator: c.clone(),
            delegatee: d.clone(),
            scoped_permissions: perms,
            expires_at: 0,
        });

        let chain = DelegationChain { links };
        assert!(!chain.validate(100));
    }

    #[test]
    fn eval_context_attr_lookup() {
        let env = Env::default();
        let subject = Address::generate(&env);

        let mut keys = Vec::new(&env);
        keys.push_back(String::from_str(&env, "role"));
        keys.push_back(String::from_str(&env, "dept"));

        let mut vals = Vec::new(&env);
        vals.push_back(String::from_str(&env, "doctor"));
        vals.push_back(String::from_str(&env, "cardiology"));

        let ctx = EvalContext {
            subject,
            resource_id: String::from_str(&env, "record_1"),
            action: String::from_str(&env, "read"),
            timestamp: 1000,
            attr_keys: keys,
            attr_vals: vals,
        };

        let role_key = String::from_str(&env, "role");
        assert_eq!(
            ctx.get_attr(&role_key),
            Some(String::from_str(&env, "doctor"))
        );

        let missing = String::from_str(&env, "nonexistent");
        assert_eq!(ctx.get_attr(&missing), None);
    }
}
