//! # Declarative Compliance Rules Engine
//!
//! This module provides a framework for defining, evaluating, and enforcing
//! compliance rules against contract operations. Rules are declarative —
//! each rule specifies a condition function and a severity level.
//!
//! The engine evaluates all applicable rules for an operation and produces
//! a [`ComplianceVerdict`] that either allows or blocks the operation,
//! along with a compliance score and any violations found.
//!
//! ## Architecture
//!
//! ```text
//! Operation → RulesEngine::evaluate()
//!               ├─ HIPAA rules (hipaa.rs)
//!               ├─ GDPR rules (gdpr.rs)
//!               └─ Jurisdiction filter
//!           → ComplianceVerdict { allowed, score, violations }
//! ```

use std::collections::HashMap;

// ── Rule severity ───────────────────────────────────────────────────────────

/// Severity level for a compliance rule violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Informational finding; does not block the operation.
    Info,
    /// Warning; operation proceeds but is flagged for review.
    Warning,
    /// Critical violation; operation MUST be blocked.
    Critical,
}

// ── Jurisdiction ────────────────────────────────────────────────────────────

/// Jurisdiction determines which compliance rules apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Jurisdiction {
    /// United States — HIPAA rules apply.
    US,
    /// European Union — GDPR rules apply.
    EU,
    /// Both HIPAA and GDPR rules apply (e.g. US entity processing EU data).
    Both,
}

// ── Operation context ───────────────────────────────────────────────────────

/// Context provided for each operation to be compliance-checked.
#[derive(Debug, Clone)]
pub struct OperationContext {
    /// The actor performing the operation.
    pub actor: String,
    /// The role of the actor (e.g. "clinician", "admin", "patient").
    pub actor_role: String,
    /// The operation being performed (e.g. "record.read", "data.export").
    pub action: String,
    /// The target resource (e.g. "patient:42", "record:123").
    pub target: String,
    /// Unix timestamp of the operation.
    pub timestamp: u64,
    /// Whether the actor has explicit consent for this operation.
    pub has_consent: bool,
    /// The data sensitivity level (0 = public, 1 = internal, 2 = sensitive, 3 = PHI).
    pub sensitivity: u32,
    /// The jurisdiction governing this operation.
    pub jurisdiction: Jurisdiction,
    /// Number of records accessed in this request (for bulk detection).
    pub record_count: u32,
    /// Purpose of the data access (e.g. "treatment", "research", "billing").
    pub purpose: String,
    /// Additional metadata key-value pairs for custom rule evaluation.
    pub metadata: HashMap<String, String>,
}

// ── Rule definition ─────────────────────────────────────────────────────────

/// A single compliance rule violation.
#[derive(Debug, Clone)]
pub struct Violation {
    /// Unique rule identifier (e.g. "HIPAA-001", "GDPR-003").
    pub rule_id: String,
    /// Human-readable description of the violation.
    pub description: String,
    /// Severity of the violation.
    pub severity: Severity,
    /// Suggested remediation action.
    pub remediation: String,
}

/// A declarative compliance rule.
pub struct ComplianceRule {
    /// Unique rule identifier.
    pub id: String,
    /// Human-readable rule name.
    pub name: String,
    /// Which jurisdictions this rule applies to.
    pub jurisdictions: Vec<Jurisdiction>,
    /// Severity if this rule is violated.
    pub severity: Severity,
    /// Suggested remediation if violated.
    pub remediation: String,
    /// The evaluation function: returns `true` if the operation is compliant.
    pub evaluate: Box<dyn Fn(&OperationContext) -> bool + Send + Sync>,
}

// ── Compliance verdict ──────────────────────────────────────────────────────

/// The result of evaluating all rules against an operation.
#[derive(Debug, Clone)]
pub struct ComplianceVerdict {
    /// Whether the operation is allowed to proceed.
    pub allowed: bool,
    /// Compliance score (0.0 = fully non-compliant, 100.0 = fully compliant).
    pub score: f64,
    /// List of violations found.
    pub violations: Vec<Violation>,
    /// Total rules evaluated.
    pub rules_evaluated: u32,
    /// Rules that passed.
    pub rules_passed: u32,
}

// ── Compliance report ───────────────────────────────────────────────────────

/// An aggregate compliance report covering multiple operations.
#[derive(Debug, Clone)]
pub struct ComplianceReport {
    /// Unix timestamp when the report was generated.
    pub generated_at: u64,
    /// Time period start (Unix timestamp).
    pub period_start: u64,
    /// Time period end (Unix timestamp).
    pub period_end: u64,
    /// Total operations evaluated.
    pub total_operations: u64,
    /// Operations that were fully compliant.
    pub compliant_operations: u64,
    /// Operations that had violations.
    pub non_compliant_operations: u64,
    /// Aggregate compliance score.
    pub aggregate_score: f64,
    /// All violations found in the period, grouped by rule ID.
    pub violations_by_rule: HashMap<String, Vec<Violation>>,
    /// Jurisdiction breakdown.
    pub jurisdiction: Jurisdiction,
}

// ── Rules Engine ────────────────────────────────────────────────────────────

/// The main compliance rules engine. Holds all registered rules and evaluates
/// operations against them.
pub struct RulesEngine {
    rules: Vec<ComplianceRule>,
    /// History of verdicts for report generation.
    verdict_history: Vec<(OperationContext, ComplianceVerdict)>,
}

impl RulesEngine {
    /// Create an empty rules engine.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            verdict_history: Vec::new(),
        }
    }

    /// Register a compliance rule.
    pub fn register_rule(&mut self, rule: ComplianceRule) {
        self.rules.push(rule);
    }

    /// Evaluate all applicable rules for an operation context.
    ///
    /// Rules are filtered by jurisdiction before evaluation. A single
    /// `Critical` violation causes the operation to be blocked.
    pub fn evaluate(&mut self, ctx: &OperationContext) -> ComplianceVerdict {
        let mut violations = Vec::new();
        let mut rules_evaluated: u32 = 0;
        let mut rules_passed: u32 = 0;

        for rule in &self.rules {
            // Check jurisdiction applicability.
            let applicable = rule.jurisdictions.iter().any(|j| {
                *j == ctx.jurisdiction
                    || ctx.jurisdiction == Jurisdiction::Both
                    || *j == Jurisdiction::Both
            });

            if !applicable {
                continue;
            }

            rules_evaluated += 1;

            let compliant = (rule.evaluate)(ctx);
            if compliant {
                rules_passed += 1;
            } else {
                violations.push(Violation {
                    rule_id: rule.id.clone(),
                    description: rule.name.clone(),
                    severity: rule.severity,
                    remediation: rule.remediation.clone(),
                });
            }
        }

        let score = if rules_evaluated == 0 {
            100.0
        } else {
            (rules_passed as f64 / rules_evaluated as f64) * 100.0
        };

        let has_critical = violations.iter().any(|v| v.severity == Severity::Critical);

        let verdict = ComplianceVerdict {
            allowed: !has_critical,
            score,
            violations,
            rules_evaluated,
            rules_passed,
        };

        self.verdict_history.push((ctx.clone(), verdict.clone()));

        verdict
    }

    /// Generate a compliance report for a time period.
    pub fn generate_report(
        &self,
        period_start: u64,
        period_end: u64,
        now: u64,
        jurisdiction: Jurisdiction,
    ) -> ComplianceReport {
        let mut total: u64 = 0;
        let mut compliant: u64 = 0;
        let mut non_compliant: u64 = 0;
        let mut score_sum: f64 = 0.0;
        let mut violations_by_rule: HashMap<String, Vec<Violation>> = HashMap::new();

        for (ctx, verdict) in &self.verdict_history {
            if ctx.timestamp < period_start || ctx.timestamp > period_end {
                continue;
            }

            total += 1;
            score_sum += verdict.score;

            if verdict.violations.is_empty() {
                compliant += 1;
            } else {
                non_compliant += 1;
                for v in &verdict.violations {
                    violations_by_rule
                        .entry(v.rule_id.clone())
                        .or_default()
                        .push(v.clone());
                }
            }
        }

        let aggregate_score = if total == 0 {
            100.0
        } else {
            score_sum / total as f64
        };

        ComplianceReport {
            generated_at: now,
            period_start,
            period_end,
            total_operations: total,
            compliant_operations: compliant,
            non_compliant_operations: non_compliant,
            aggregate_score,
            violations_by_rule,
            jurisdiction,
        }
    }

    /// Get the total number of registered rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Clear the verdict history.
    pub fn clear_history(&mut self) {
        self.verdict_history.clear();
    }
}

impl Default for RulesEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ctx() -> OperationContext {
        OperationContext {
            actor: "dr_smith".into(),
            actor_role: "clinician".into(),
            action: "record.read".into(),
            target: "patient:42".into(),
            timestamp: 1_000_000,
            has_consent: true,
            sensitivity: 3,
            jurisdiction: Jurisdiction::US,
            record_count: 1,
            purpose: "treatment".into(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn empty_engine_allows_everything() {
        let mut engine = RulesEngine::new();
        let verdict = engine.evaluate(&sample_ctx());
        assert!(verdict.allowed);
        assert_eq!(verdict.score, 100.0);
        assert_eq!(verdict.rules_evaluated, 0);
    }

    #[test]
    fn critical_violation_blocks_operation() {
        let mut engine = RulesEngine::new();
        engine.register_rule(ComplianceRule {
            id: "TEST-001".into(),
            name: "Always fails".into(),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Critical,
            remediation: "Fix it".into(),
            evaluate: Box::new(|_| false),
        });

        let verdict = engine.evaluate(&sample_ctx());
        assert!(!verdict.allowed);
        assert_eq!(verdict.violations.len(), 1);
        assert_eq!(verdict.violations[0].rule_id, "TEST-001");
    }

    #[test]
    fn warning_does_not_block() {
        let mut engine = RulesEngine::new();
        engine.register_rule(ComplianceRule {
            id: "TEST-002".into(),
            name: "Warning rule".into(),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Warning,
            remediation: "Review it".into(),
            evaluate: Box::new(|_| false),
        });

        let verdict = engine.evaluate(&sample_ctx());
        assert!(verdict.allowed);
        assert_eq!(verdict.violations.len(), 1);
    }

    #[test]
    fn jurisdiction_filtering() {
        let mut engine = RulesEngine::new();
        engine.register_rule(ComplianceRule {
            id: "EU-ONLY".into(),
            name: "EU only rule".into(),
            jurisdictions: vec![Jurisdiction::EU],
            severity: Severity::Critical,
            remediation: "N/A".into(),
            evaluate: Box::new(|_| false),
        });

        // US context should not trigger EU-only rule.
        let verdict = engine.evaluate(&sample_ctx());
        assert!(verdict.allowed);
        assert_eq!(verdict.rules_evaluated, 0);
    }

    #[test]
    fn report_generation() {
        let mut engine = RulesEngine::new();
        engine.register_rule(ComplianceRule {
            id: "TEST-003".into(),
            name: "Consent check".into(),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Critical,
            remediation: "Get consent".into(),
            evaluate: Box::new(|ctx| ctx.has_consent),
        });

        let mut ctx = sample_ctx();
        ctx.timestamp = 500;
        engine.evaluate(&ctx);

        ctx.has_consent = false;
        ctx.timestamp = 600;
        engine.evaluate(&ctx);

        let report = engine.generate_report(0, 1000, 1001, Jurisdiction::US);
        assert_eq!(report.total_operations, 2);
        assert_eq!(report.compliant_operations, 1);
        assert_eq!(report.non_compliant_operations, 1);
    }

    #[test]
    fn score_calculation() {
        let mut engine = RulesEngine::new();
        for i in 0..4 {
            let pass = i < 3;
            engine.register_rule(ComplianceRule {
                id: format!("R-{}", i),
                name: format!("Rule {}", i),
                jurisdictions: vec![Jurisdiction::US],
                severity: Severity::Warning,
                remediation: "Fix".into(),
                evaluate: Box::new(move |_| pass),
            });
        }

        let verdict = engine.evaluate(&sample_ctx());
        assert_eq!(verdict.rules_evaluated, 4);
        assert_eq!(verdict.rules_passed, 3);
        assert!((verdict.score - 75.0).abs() < 0.01);
    }
}
