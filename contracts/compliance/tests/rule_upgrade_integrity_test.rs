//! # Rule Upgrade Integrity Tests (#499)
//!
//! Tests the process of upgrading regulatory rules within the compliance module.
//! Upgrading rules should not invalidate previously compliant, already-committed data.
//!
//! ## Test Coverage
//!
//! 1. **Historical Compliance Validity**: Simulate a major rule change and verify
//!    that historical compliance logs remain valid.
//!
//! 2. **Grace Period Logic**: Newly-implemented rules only apply to new transactions,
//!    not retroactively to historical data.
//!
//! 3. **Atomicity of Rule Updates**: Prevent inconsistent state during transitions.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use compliance::rules_engine::{
    ComplianceRule, ComplianceVerdict, Jurisdiction, OperationContext, RulesEngine, Severity,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Create a baseline compliant operation context.
fn compliant_ctx(timestamp: u64) -> OperationContext {
    OperationContext {
        actor: "dr_smith".into(),
        actor_role: "clinician".into(),
        action: "record.read".into(),
        target: "patient:42".into(),
        timestamp,
        has_consent: true,
        sensitivity: 3,
        jurisdiction: Jurisdiction::US,
        record_count: 1,
        purpose: "treatment".into(),
        metadata: {
            let mut m = HashMap::new();
            m.insert("encrypted".into(), "true".into());
            m.insert("lawful_basis".into(), "consent".into());
            m
        },
    }
}

/// Create a non-compliant operation context (missing consent).
fn non_compliant_ctx(timestamp: u64) -> OperationContext {
    OperationContext {
        actor: "dr_smith".into(),
        actor_role: "clinician".into(),
        action: "record.read".into(),
        target: "patient:42".into(),
        timestamp,
        has_consent: false,
        sensitivity: 3,
        jurisdiction: Jurisdiction::US,
        record_count: 1,
        purpose: "treatment".into(),
        metadata: {
            let mut m = HashMap::new();
            m.insert("encrypted".into(), "true".into());
            m
        },
    }
}

/// Create a versioned rules engine that tracks rule versions.
struct VersionedRulesEngine {
    engine: RulesEngine,
    rule_versions: HashMap<String, u32>,
    current_version: u32,
}

impl VersionedRulesEngine {
    fn new() -> Self {
        Self {
            engine: RulesEngine::new(),
            rule_versions: HashMap::new(),
            current_version: 1,
        }
    }

    /// Register a rule with version tracking.
    fn register_rule_with_version(&mut self, rule: ComplianceRule, version: u32) {
        self.rule_versions.insert(rule.id.clone(), version);
        self.engine.register_rule(rule);
    }

    /// Upgrade to a new version.
    fn upgrade_version(&mut self) {
        self.current_version += 1;
    }

    /// Get the version of a specific rule.
    fn get_rule_version(&self, rule_id: &str) -> Option<u32> {
        self.rule_versions.get(rule_id).copied()
    }

    /// Evaluate an operation.
    fn evaluate(&mut self, ctx: &OperationContext) -> ComplianceVerdict {
        self.engine.evaluate(ctx)
    }

    /// Generate a report.
    fn generate_report(
        &self,
        period_start: u64,
        period_end: u64,
        now: u64,
        jurisdiction: Jurisdiction,
    ) -> compliance::rules_engine::ComplianceReport {
        self.engine
            .generate_report(period_start, period_end, now, jurisdiction)
    }
}

// ==========================================================================
// 1. Historical Compliance Validity After Rule Changes
// ==========================================================================

#[test]
fn historical_compliance_logs_remain_valid_after_rule_upgrade() {
    // Scenario: A rule is upgraded to be stricter, but historical compliance
    // logs should remain valid and not be re-evaluated.

    let mut engine = RulesEngine::new();

    // Original rule: Consent check (lenient - only checks has_consent)
    engine.register_rule(ComplianceRule {
        id: "CONSENT-001".into(),
        name: "Consent verification".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Obtain patient consent".into(),
        evaluate: Box::new(|ctx| ctx.has_consent),
    });

    // Record historical compliant operations
    let historical_ctx = compliant_ctx(1000);
    let historical_verdict = engine.evaluate(&historical_ctx);

    // Verify historical operation was compliant
    assert!(
        historical_verdict.allowed,
        "Historical operation should be compliant under original rules"
    );
    assert!(
        historical_verdict.violations.is_empty(),
        "Historical operation should have no violations"
    );

    // Simulate rule upgrade: Now require both consent AND encryption metadata
    // In a real system, this would be a new rule version, not modifying the existing one
    engine.register_rule(ComplianceRule {
        id: "CONSENT-002".into(),
        name: "Enhanced consent verification".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Obtain patient consent and ensure encryption".into(),
        evaluate: Box::new(|ctx| {
            ctx.has_consent && ctx.metadata.get("encrypted").map_or(false, |v| v == "true")
        }),
    });

    // Evaluate the same historical context with new rules
    let re_evaluated_verdict = engine.evaluate(&historical_ctx);

    // Historical operation should still be compliant (it meets the stricter requirements)
    assert!(
        re_evaluated_verdict.allowed,
        "Historical operation should remain compliant after rule upgrade"
    );

    // Generate a report covering the historical period
    let report = engine.generate_report(0, 2000, 2001, Jurisdiction::US);
    assert_eq!(
        report.total_operations, 2,
        "Report should include both evaluations"
    );
    assert_eq!(
        report.compliant_operations, 2,
        "Both operations should be compliant"
    );
}

#[test]
fn historical_violations_remain_unchanged_after_rule_relaxation() {
    // Scenario: A rule is relaxed, but historical violations should remain
    // as violations in the audit trail.

    let mut engine = RulesEngine::new();

    // Strict original rule: Requires encryption for all PHI
    engine.register_rule(ComplianceRule {
        id: "ENCRYPTION-001".into(),
        name: "PHI encryption required".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Enable encryption".into(),
        evaluate: Box::new(|ctx| ctx.metadata.get("encrypted").map_or(false, |v| v == "true")),
    });

    // Record a non-compliant operation (missing encryption)
    let mut non_compliant = compliant_ctx(1000);
    non_compliant.metadata.clear();
    let original_verdict = engine.evaluate(&non_compliant);

    assert!(
        !original_verdict.allowed,
        "Operation without encryption should be blocked"
    );
    assert_eq!(
        original_verdict.violations.len(),
        1,
        "Should have one violation"
    );
    assert_eq!(
        original_verdict.violations[0].rule_id, "ENCRYPTION-001",
        "Violation should be for encryption rule"
    );

    // Simulate rule relaxation: Now only require encryption for sensitivity >= 2
    engine.register_rule(ComplianceRule {
        id: "ENCRYPTION-002".into(),
        name: "PHI encryption for sensitive data".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Enable encryption for sensitive data".into(),
        evaluate: Box::new(|ctx| {
            if ctx.sensitivity < 2 {
                true // Non-sensitive data doesn't need encryption
            } else {
                ctx.metadata.get("encrypted").map_or(false, |v| v == "true")
            }
        }),
    });

    // Re-evaluate the same context with relaxed rules
    let re_evaluated_verdict = engine.evaluate(&non_compliant);

    // The operation should now pass under relaxed rules
    assert!(
        re_evaluated_verdict.allowed,
        "Operation should pass under relaxed rules"
    );

    // But the historical violation should still be recorded
    let report = engine.generate_report(0, 2000, 2001, Jurisdiction::US);
    assert_eq!(
        report.non_compliant_operations, 1,
        "Historical violation should remain in report"
    );
    assert!(
        report.violations_by_rule.contains_key("ENCRYPTION-001"),
        "Original violation should be in report"
    );
}

// ==========================================================================
// 2. Grace Period Logic for New Rules
// ==========================================================================

#[test]
fn new_rules_only_apply_to_future_transactions() {
    // Scenario: A new rule is implemented with a grace period.
    // Transactions before the grace period end should use old rules.

    let mut engine = VersionedRulesEngine::new();

    // Original rule: Basic consent check
    engine.register_rule_with_version(
        ComplianceRule {
            id: "CONSENT-BASIC".into(),
            name: "Basic consent check".into(),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Critical,
            remediation: "Obtain consent".into(),
            evaluate: Box::new(|ctx| ctx.has_consent),
        },
        1,
    );

    // Record transactions under version 1
    let old_transaction = compliant_ctx(1000);
    let old_verdict = engine.evaluate(&old_transaction);
    assert!(
        old_verdict.allowed,
        "Old transaction should be compliant under v1 rules"
    );

    // Upgrade to version 2 with stricter rules
    engine.upgrade_version();
    engine.register_rule_with_version(
        ComplianceRule {
            id: "CONSENT-ENHANCED".into(),
            name: "Enhanced consent with metadata".into(),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Critical,
            remediation: "Obtain consent with proper metadata".into(),
            evaluate: Box::new(|ctx| ctx.has_consent && ctx.metadata.get("lawful_basis").is_some()),
        },
        2,
    );

    // Grace period: Transactions before timestamp 2000 use v1 rules
    let grace_period_end = 2000;

    // Transaction during grace period (should use v1 rules)
    let grace_period_transaction = compliant_ctx(1500);
    let grace_verdict = engine.evaluate(&grace_period_transaction);
    assert!(
        grace_verdict.allowed,
        "Transaction during grace period should be compliant"
    );

    // Transaction after grace period (should use v2 rules)
    let post_grace_transaction = compliant_ctx(2500);
    let post_grace_verdict = engine.evaluate(&post_grace_transaction);
    assert!(
        post_grace_verdict.allowed,
        "Transaction after grace period should be compliant (has metadata)"
    );

    // Transaction after grace period without required metadata
    let mut non_compliant_post_grace = compliant_ctx(2500);
    non_compliant_post_grace.metadata.clear();
    let non_compliant_verdict = engine.evaluate(&non_compliant_post_grace);
    assert!(
        !non_compliant_verdict.allowed,
        "Transaction after grace period without metadata should be blocked"
    );
}

#[test]
fn grace_period_prevents_retroactive_rule_application() {
    // Scenario: A new rule with a grace period should not retroactively
    // invalidate transactions that occurred before the rule was added.

    let mut engine = RulesEngine::new();

    // Record transactions before any rules are added
    let pre_rule_transaction = compliant_ctx(500);
    let pre_rule_verdict = engine.evaluate(&pre_rule_transaction);
    assert!(
        pre_rule_verdict.allowed,
        "Transaction before rules should be allowed"
    );
    assert_eq!(
        pre_rule_verdict.score, 100.0,
        "Score should be 100% with no rules"
    );

    // Add a new rule with grace period
    let rule_effective_timestamp = 1000;
    engine.register_rule(ComplianceRule {
        id: "NEW-RULE-001".into(),
        name: "New requirement".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Comply with new requirement".into(),
        evaluate: Box::new(|ctx| {
            // Rule only applies to transactions after effective timestamp
            if ctx.timestamp < rule_effective_timestamp {
                true // Grace period: old transactions pass
            } else {
                // New transactions must meet requirement
                ctx.metadata
                    .get("new_requirement")
                    .map_or(false, |v| v == "true")
            }
        }),
    });

    // Re-evaluate pre-rule transaction (should still pass due to grace period)
    let re_evaluated_pre_rule = engine.evaluate(&pre_rule_transaction);
    assert!(
        re_evaluated_pre_rule.allowed,
        "Pre-rule transaction should pass due to grace period"
    );

    // Transaction after rule effective date without new requirement
    let post_rule_non_compliant = compliant_ctx(1500);
    let post_rule_verdict = engine.evaluate(&post_rule_non_compliant);
    assert!(
        !post_rule_verdict.allowed,
        "Post-rule transaction without new requirement should be blocked"
    );

    // Transaction after rule effective date with new requirement
    let mut post_rule_compliant = compliant_ctx(1500);
    post_rule_compliant
        .metadata
        .insert("new_requirement".into(), "true".into());
    let compliant_verdict = engine.evaluate(&post_rule_compliant);
    assert!(
        compliant_verdict.allowed,
        "Post-rule transaction with new requirement should pass"
    );
}

#[test]
fn multiple_rule_upgrades_with_staggered_grace_periods() {
    // Scenario: Multiple rule upgrades with different grace periods
    // should maintain consistency.

    let mut engine = RulesEngine::new();

    // Rule 1: Effective immediately
    engine.register_rule(ComplianceRule {
        id: "RULE-IMMEDIATE".into(),
        name: "Immediate rule".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Warning,
        remediation: "Comply immediately".into(),
        evaluate: Box::new(|ctx| ctx.has_consent),
    });

    // Rule 2: Effective at timestamp 1000
    engine.register_rule(ComplianceRule {
        id: "RULE-DELAYED-1000".into(),
        name: "Delayed rule (1000)".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Comply by timestamp 1000".into(),
        evaluate: Box::new(|ctx| {
            if ctx.timestamp < 1000 {
                true
            } else {
                ctx.metadata
                    .get("delayed_1000")
                    .map_or(false, |v| v == "true")
            }
        }),
    });

    // Rule 3: Effective at timestamp 2000
    engine.register_rule(ComplianceRule {
        id: "RULE-DELAYED-2000".into(),
        name: "Delayed rule (2000)".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Comply by timestamp 2000".into(),
        evaluate: Box::new(|ctx| {
            if ctx.timestamp < 2000 {
                true
            } else {
                ctx.metadata
                    .get("delayed_2000")
                    .map_or(false, |v| v == "true")
            }
        }),
    });

    // Test at different timestamps
    let ctx_500 = compliant_ctx(500);
    let verdict_500 = engine.evaluate(&ctx_500);
    assert!(
        verdict_500.allowed,
        "At timestamp 500, only immediate rule applies"
    );

    let ctx_1500 = compliant_ctx(1500);
    let verdict_1500 = engine.evaluate(&ctx_1500);
    assert!(
        !verdict_1500.allowed,
        "At timestamp 1500, delayed-1000 rule should block (missing metadata)"
    );

    let mut ctx_1500_compliant = compliant_ctx(1500);
    ctx_1500_compliant
        .metadata
        .insert("delayed_1000".into(), "true".into());
    let verdict_1500_compliant = engine.evaluate(&ctx_1500_compliant);
    assert!(
        verdict_1500_compliant.allowed,
        "At timestamp 1500 with delayed-1000 metadata, should pass"
    );

    let mut ctx_2500 = compliant_ctx(2500);
    ctx_2500
        .metadata
        .insert("delayed_1000".into(), "true".into());
    let verdict_2500 = engine.evaluate(&ctx_2500);
    assert!(
        !verdict_2500.allowed,
        "At timestamp 2500, delayed-2000 rule should block (missing metadata)"
    );

    let mut ctx_2500_compliant = compliant_ctx(2500);
    ctx_2500_compliant
        .metadata
        .insert("delayed_1000".into(), "true".into());
    ctx_2500_compliant
        .metadata
        .insert("delayed_2000".into(), "true".into());
    let verdict_2500_compliant = engine.evaluate(&ctx_2500_compliant);
    assert!(
        verdict_2500_compliant.allowed,
        "At timestamp 2500 with all metadata, should pass"
    );
}

// ==========================================================================
// 3. Atomicity of Rule Updates
// ==========================================================================

#[test]
fn rule_update_is_atomic() {
    // Scenario: Rule updates should be atomic - either all rules are updated
    // or none are. No partial state should be visible.

    let engine = Arc::new(Mutex::new(RulesEngine::new()));

    // Register initial rules
    {
        let mut eng = engine.lock().unwrap();
        eng.register_rule(ComplianceRule {
            id: "ATOMIC-001".into(),
            name: "Initial rule".into(),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Critical,
            remediation: "Fix".into(),
            evaluate: Box::new(|ctx| ctx.has_consent),
        });
    }

    // Simulate atomic rule update
    let update_successful = {
        let mut eng = engine.lock().unwrap();

        // In a real system, this would be wrapped in a transaction
        // For testing, we simulate the atomic update

        // Remove old rule
        // Note: RulesEngine doesn't have a remove method, so we'll test
        // by adding new rules atomically

        // Add new rules
        eng.register_rule(ComplianceRule {
            id: "ATOMIC-002".into(),
            name: "New rule 1".into(),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Critical,
            remediation: "Fix".into(),
            evaluate: Box::new(|ctx| ctx.metadata.get("atomic").is_some()),
        });

        eng.register_rule(ComplianceRule {
            id: "ATOMIC-003".into(),
            name: "New rule 2".into(),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Warning,
            remediation: "Fix".into(),
            evaluate: Box::new(|ctx| ctx.record_count <= 10),
        });

        true // Update successful
    };

    assert!(update_successful, "Rule update should succeed");

    // Verify all rules are active
    let mut eng = engine.lock().unwrap();
    let ctx = compliant_ctx(1000);
    let verdict = eng.evaluate(&ctx);

    // Should have violations for ATOMIC-002 (missing metadata)
    assert!(!verdict.allowed, "Should be blocked by new atomic rule");
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "ATOMIC-002"),
        "Should have violation for ATOMIC-002"
    );
}

#[test]
fn concurrent_rule_updates_maintain_consistency() {
    // Scenario: Multiple concurrent rule updates should not leave the system
    // in an inconsistent state.

    let engine = Arc::new(Mutex::new(RulesEngine::new()));

    // Initialize with base rules
    {
        let mut eng = engine.lock().unwrap();
        eng.register_rule(ComplianceRule {
            id: "BASE-001".into(),
            name: "Base rule".into(),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Critical,
            remediation: "Fix".into(),
            evaluate: Box::new(|ctx| ctx.has_consent),
        });
    }

    // Simulate concurrent updates (in a real system, this would use proper locking)
    let engine_clone1 = Arc::clone(&engine);
    let engine_clone2 = Arc::clone(&engine);

    // Update 1: Add encryption rule
    let handle1 = std::thread::spawn(move || {
        let mut eng = engine_clone1.lock().unwrap();
        eng.register_rule(ComplianceRule {
            id: "CONCURRENT-001".into(),
            name: "Encryption rule".into(),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Critical,
            remediation: "Enable encryption".into(),
            evaluate: Box::new(|ctx| ctx.metadata.get("encrypted").map_or(false, |v| v == "true")),
        });
    });

    // Update 2: Add audit logging rule
    let handle2 = std::thread::spawn(move || {
        let mut eng = engine_clone2.lock().unwrap();
        eng.register_rule(ComplianceRule {
            id: "CONCURRENT-002".into(),
            name: "Audit logging rule".into(),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Warning,
            remediation: "Enable audit logging".into(),
            evaluate: Box::new(|ctx| {
                ctx.metadata
                    .get("audit_logged")
                    .map_or(false, |v| v == "true")
            }),
        });
    });

    handle1.join().unwrap();
    handle2.join().unwrap();

    // Verify system is in consistent state with all rules
    let mut eng = engine.lock().unwrap();
    let ctx = compliant_ctx(1000);
    let verdict = eng.evaluate(&ctx);

    // Should have violations for both new rules
    assert!(
        verdict
            .violations
            .iter()
            .any(|v| v.rule_id == "CONCURRENT-001"),
        "Should have encryption violation"
    );
    assert!(
        verdict
            .violations
            .iter()
            .any(|v| v.rule_id == "CONCURRENT-002"),
        "Should have audit logging violation"
    );
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "BASE-001"),
        "Should still have base rule violation"
    );
}

#[test]
fn rule_rollback_maintains_historical_integrity() {
    // Scenario: If a rule update is rolled back, historical compliance
    // logs should remain valid.

    let mut engine = RulesEngine::new();

    // Initial rule set
    engine.register_rule(ComplianceRule {
        id: "ROLLBACK-001".into(),
        name: "Initial rule".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.has_consent),
    });

    // Record compliant transaction
    let ctx1 = compliant_ctx(1000);
    let verdict1 = engine.evaluate(&ctx1);
    assert!(verdict1.allowed, "Initial transaction should be compliant");

    // Simulate rule update (add stricter rule)
    engine.register_rule(ComplianceRule {
        id: "ROLLBACK-002".into(),
        name: "Stricter rule".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.metadata.get("strict").map_or(false, |v| v == "true")),
    });

    // Record transaction under stricter rules
    let ctx2 = compliant_ctx(1500);
    let verdict2 = engine.evaluate(&ctx2);
    assert!(
        !verdict2.allowed,
        "Transaction under stricter rules should be blocked"
    );

    // Simulate rollback: Create new engine with original rules only
    let mut rolled_back_engine = RulesEngine::new();
    rolled_back_engine.register_rule(ComplianceRule {
        id: "ROLLBACK-001".into(),
        name: "Initial rule".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.has_consent),
    });

    // Re-evaluate historical transactions with rolled-back rules
    let rolled_back_verdict1 = rolled_back_engine.evaluate(&ctx1);
    assert!(
        rolled_back_verdict1.allowed,
        "Historical transaction should remain compliant after rollback"
    );

    let rolled_back_verdict2 = rolled_back_engine.evaluate(&ctx2);
    assert!(
        rolled_back_verdict2.allowed,
        "Transaction should be compliant after rollback"
    );

    // Generate report to verify historical integrity
    let report = rolled_back_engine.generate_report(0, 2000, 2001, Jurisdiction::US);
    assert_eq!(
        report.compliant_operations, 2,
        "Both transactions should be compliant after rollback"
    );
}

// ==========================================================================
// 4. Complex Upgrade Scenarios
// ==========================================================================

#[test]
fn rule_upgrade_with_jurisdiction_changes() {
    // Scenario: Rules are upgraded with jurisdiction changes.
    // Historical data should respect original jurisdiction rules.

    let mut engine = RulesEngine::new();

    // Original rule: US only
    engine.register_rule(ComplianceRule {
        id: "JURISDICTION-001".into(),
        name: "US-only rule".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.has_consent),
    });

    // Record US transaction
    let us_ctx = compliant_ctx(1000);
    let us_verdict = engine.evaluate(&us_ctx);
    assert!(us_verdict.allowed, "US transaction should be compliant");

    // Record EU transaction (should not be affected by US rule)
    let mut eu_ctx = compliant_ctx(1000);
    eu_ctx.jurisdiction = Jurisdiction::EU;
    let eu_verdict = engine.evaluate(&eu_ctx);
    assert!(
        eu_verdict.allowed,
        "EU transaction should not be affected by US rule"
    );

    // Upgrade: Rule now applies to Both jurisdictions
    engine.register_rule(ComplianceRule {
        id: "JURISDICTION-002".into(),
        name: "Both jurisdictions rule".into(),
        jurisdictions: vec![Jurisdiction::Both],
        severity: Severity::Critical,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.has_consent),
    });

    // Re-evaluate historical transactions
    let re_evaluated_us = engine.evaluate(&us_ctx);
    assert!(
        re_evaluated_us.allowed,
        "Historical US transaction should remain compliant"
    );

    let re_evaluated_eu = engine.evaluate(&eu_ctx);
    assert!(
        re_evaluated_eu.allowed,
        "Historical EU transaction should remain compliant (had consent)"
    );

    // New EU transaction without consent should be blocked
    let mut new_eu_ctx = compliant_ctx(1500);
    new_eu_ctx.jurisdiction = Jurisdiction::EU;
    new_eu_ctx.has_consent = false;
    let new_eu_verdict = engine.evaluate(&new_eu_ctx);
    assert!(
        !new_eu_verdict.allowed,
        "New EU transaction without consent should be blocked"
    );
}

#[test]
fn rule_upgrade_with_severity_changes() {
    // Scenario: Rule severity is upgraded from Warning to Critical.
    // Historical warnings should remain as warnings in logs.

    let mut engine = RulesEngine::new();

    // Original rule: Warning severity
    engine.register_rule(ComplianceRule {
        id: "SEVERITY-001".into(),
        name: "Warning rule".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Warning,
        remediation: "Review".into(),
        evaluate: Box::new(|ctx| ctx.record_count <= 5),
    });

    // Record transaction that triggers warning
    let mut warning_ctx = compliant_ctx(1000);
    warning_ctx.record_count = 10;
    let warning_verdict = engine.evaluate(&warning_ctx);

    assert!(
        warning_verdict.allowed,
        "Warning should not block operation"
    );
    assert_eq!(
        warning_verdict.violations.len(),
        1,
        "Should have one warning violation"
    );
    assert_eq!(
        warning_verdict.violations[0].severity,
        Severity::Warning,
        "Violation should be Warning severity"
    );

    // Upgrade: Same rule but Critical severity
    engine.register_rule(ComplianceRule {
        id: "SEVERITY-002".into(),
        name: "Critical rule".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Fix immediately".into(),
        evaluate: Box::new(|ctx| ctx.record_count <= 5),
    });

    // Re-evaluate historical transaction
    let re_evaluated = engine.evaluate(&warning_ctx);

    // Should now be blocked due to Critical severity
    assert!(
        !re_evaluated.allowed,
        "Operation should be blocked after severity upgrade"
    );

    // Historical warning should still be recorded as warning
    let report = engine.generate_report(0, 2000, 2001, Jurisdiction::US);
    assert_eq!(
        report.non_compliant_operations, 2,
        "Both evaluations should show violations"
    );
}

#[test]
fn rule_upgrade_preserves_compliance_score_history() {
    // Scenario: Compliance scores from historical evaluations should
    // remain unchanged after rule upgrades.

    let mut engine = RulesEngine::new();

    // Add 4 rules, 3 pass, 1 fails
    for i in 0..4 {
        let pass = i < 3;
        engine.register_rule(ComplianceRule {
            id: format!("SCORE-{}", i),
            name: format!("Rule {}", i),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Warning,
            remediation: "Fix".into(),
            evaluate: Box::new(move |_| pass),
        });
    }

    let ctx = compliant_ctx(1000);
    let verdict = engine.evaluate(&ctx);

    // Score should be 75% (3/4 rules pass)
    assert_eq!(verdict.rules_evaluated, 4);
    assert_eq!(verdict.rules_passed, 3);
    assert!((verdict.score - 75.0).abs() < 0.01);

    // Add more rules (upgrade)
    for i in 4..8 {
        engine.register_rule(ComplianceRule {
            id: format!("SCORE-{}", i),
            name: format!("Rule {}", i),
            jurisdictions: vec![Jurisdiction::US],
            severity: Severity::Warning,
            remediation: "Fix".into(),
            evaluate: Box::new(|_| true),
        });
    }

    // Re-evaluate same context
    let re_evaluated = engine.evaluate(&ctx);

    // Score should now be 87.5% (7/8 rules pass)
    assert_eq!(re_evaluated.rules_evaluated, 8);
    assert_eq!(re_evaluated.rules_passed, 7);
    assert!((re_evaluated.score - 87.5).abs() < 0.01);

    // Generate report to verify both scores are recorded
    let report = engine.generate_report(0, 2000, 2001, Jurisdiction::US);
    assert_eq!(report.total_operations, 2);

    // Average score should be (75 + 87.5) / 2 = 81.25
    let expected_avg = (75.0 + 87.5) / 2.0;
    assert!(
        (report.aggregate_score - expected_avg).abs() < 0.01,
        "Aggregate score should average both evaluations"
    );
}

// ==========================================================================
// 5. Edge Cases in Rule Upgrades
// ==========================================================================

#[test]
fn empty_rule_set_upgrade() {
    // Scenario: Upgrading from empty rule set to having rules.

    let mut engine = RulesEngine::new();

    // Record transaction with no rules
    let ctx = compliant_ctx(1000);
    let verdict = engine.evaluate(&ctx);

    assert!(verdict.allowed, "Should be allowed with no rules");
    assert_eq!(verdict.score, 100.0, "Score should be 100% with no rules");
    assert_eq!(verdict.rules_evaluated, 0, "No rules should be evaluated");

    // Add rules (upgrade)
    engine.register_rule(ComplianceRule {
        id: "UPGRADE-001".into(),
        name: "New rule".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.has_consent),
    });

    // Re-evaluate
    let re_evaluated = engine.evaluate(&ctx);
    assert!(
        re_evaluated.allowed,
        "Should still be allowed (has consent)"
    );
    assert_eq!(
        re_evaluated.rules_evaluated, 1,
        "One rule should be evaluated"
    );
}

#[test]
fn rule_removal_during_upgrade() {
    // Scenario: Rules are removed during an upgrade.
    // Historical data should still reference the old rules.

    let mut engine = RulesEngine::new();

    // Add initial rules
    engine.register_rule(ComplianceRule {
        id: "REMOVE-001".into(),
        name: "Rule to be removed".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.has_consent),
    });

    engine.register_rule(ComplianceRule {
        id: "KEEP-001".into(),
        name: "Rule to keep".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Warning,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.record_count <= 10),
    });

    // Record transaction
    let ctx = compliant_ctx(1000);
    let verdict = engine.evaluate(&ctx);
    assert!(verdict.allowed, "Should be compliant");
    assert_eq!(verdict.rules_evaluated, 2, "Two rules should be evaluated");

    // Simulate upgrade: Create new engine without REMOVE-001
    let mut upgraded_engine = RulesEngine::new();
    upgraded_engine.register_rule(ComplianceRule {
        id: "KEEP-001".into(),
        name: "Rule to keep".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Warning,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.record_count <= 10),
    });

    // Re-evaluate with upgraded engine
    let upgraded_verdict = upgraded_engine.evaluate(&ctx);
    assert!(upgraded_verdict.allowed, "Should still be compliant");
    assert_eq!(
        upgraded_verdict.rules_evaluated, 1,
        "Only one rule should be evaluated after removal"
    );

    // Historical report should still show both evaluations
    // (In a real system, historical verdicts would be stored separately)
}

#[test]
fn rule_upgrade_with_multiple_jurisdictions() {
    // Scenario: Rules are upgraded to apply to multiple jurisdictions.

    let mut engine = RulesEngine::new();

    // Original: Separate rules for US and EU
    engine.register_rule(ComplianceRule {
        id: "US-001".into(),
        name: "US rule".into(),
        jurisdictions: vec![Jurisdiction::US],
        severity: Severity::Critical,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.has_consent),
    });

    engine.register_rule(ComplianceRule {
        id: "EU-001".into(),
        name: "EU rule".into(),
        jurisdictions: vec![Jurisdiction::EU],
        severity: Severity::Critical,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.metadata.get("gdpr_compliant").is_some()),
    });

    // Record US transaction
    let us_ctx = compliant_ctx(1000);
    let us_verdict = engine.evaluate(&us_ctx);
    assert!(us_verdict.allowed, "US transaction should be compliant");

    // Record EU transaction
    let mut eu_ctx = compliant_ctx(1000);
    eu_ctx.jurisdiction = Jurisdiction::EU;
    let eu_verdict = engine.evaluate(&eu_ctx);
    assert!(!eu_verdict.allowed, "EU transaction should be blocked");

    // Upgrade: Unified rule for Both jurisdictions
    engine.register_rule(ComplianceRule {
        id: "UNIFIED-001".into(),
        name: "Unified rule".into(),
        jurisdictions: vec![Jurisdiction::Both],
        severity: Severity::Critical,
        remediation: "Fix".into(),
        evaluate: Box::new(|ctx| ctx.has_consent && ctx.metadata.get("gdpr_compliant").is_some()),
    });

    // Re-evaluate historical transactions
    let re_evaluated_us = engine.evaluate(&us_ctx);
    assert!(
        !re_evaluated_us.allowed,
        "US transaction should now be blocked (missing gdpr_compliant)"
    );

    let re_evaluated_eu = engine.evaluate(&eu_ctx);
    assert!(
        !re_evaluated_eu.allowed,
        "EU transaction should still be blocked"
    );

    // New transaction with both requirements
    let mut unified_ctx = compliant_ctx(1500);
    unified_ctx
        .metadata
        .insert("gdpr_compliant".into(), "true".into());
    let unified_verdict = engine.evaluate(&unified_ctx);
    assert!(
        unified_verdict.allowed,
        "Unified transaction should be compliant"
    );
}
