//! # Zero-Value Parameter Passing Edge Cases for `compliance` (#321)
//!
//! Validates consistent handling when critical state-modifying functions
//! receive zero, empty, or blank inputs:
//!
//! - Empty strings for actor, target, action, purpose, and role fields
//! - Zero / u64::MAX timestamps
//! - Zero record counts and sensitivity levels
//! - Empty metadata maps
//! - Empty data target vectors (ErasureManager)
//! - Zero retention periods
//! - Empty / blank BAA template fields
//! - Blank search keywords (ComplianceAuditLog)

use std::collections::HashMap;

use compliance::access_control::{AccessControl, PolicyAwareAccessControl, Role};
use compliance::audit::{AuditLog, ComplianceAuditLog, ComplianceVerdictLogger, SearchKey};
use compliance::breach_detector::{AccessEvent, AlertType, BreachDetector};
use compliance::gdpr::ErasureManager;
use compliance::retention::RetentionManager;
use compliance::rules_engine::{Jurisdiction, OperationContext, RulesEngine};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Fully-compliant baseline context. Tests mutate individual fields to zero
/// values and assert the expected violation or graceful handling.
fn baseline_ctx() -> OperationContext {
    OperationContext {
        actor: "dr_smith".into(),
        actor_role: "clinician".into(),
        action: "record.read".into(),
        target: "patient:42".into(),
        timestamp: 43200, // noon UTC
        has_consent: true,
        sensitivity: 3,
        jurisdiction: Jurisdiction::Both,
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

fn engine_all_rules() -> RulesEngine {
    let mut engine = RulesEngine::new();
    compliance::hipaa::register_hipaa_rules(&mut engine);
    compliance::gdpr::register_gdpr_rules(&mut engine);
    engine
}

fn make_audit_log() -> ComplianceAuditLog {
    let key = SearchKey::from_bytes(&[0x77u8; 32]).unwrap();
    ComplianceAuditLog::new(key)
}

// ==========================================================================
// 1. RulesEngine — Empty / Zero OperationContext Fields
// ==========================================================================

#[test]
fn empty_actor_triggers_hipaa_access_logging_violation() {
    let mut engine = engine_all_rules();
    let mut ctx = baseline_ctx();
    ctx.actor = String::new();

    let verdict = engine.evaluate(&ctx);
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "HIPAA-002"),
        "Empty actor must violate HIPAA-002 (access logging)"
    );
}

#[test]
fn empty_target_triggers_hipaa_access_logging_violation() {
    let mut engine = engine_all_rules();
    let mut ctx = baseline_ctx();
    ctx.target = String::new();

    let verdict = engine.evaluate(&ctx);
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "HIPAA-002"),
        "Empty target must violate HIPAA-002 (access logging)"
    );
}

#[test]
fn empty_actor_and_target_both_empty() {
    let mut engine = engine_all_rules();
    let mut ctx = baseline_ctx();
    ctx.actor = String::new();
    ctx.target = String::new();

    let verdict = engine.evaluate(&ctx);
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "HIPAA-002"),
        "Both empty must still trigger HIPAA-002"
    );
}

#[test]
fn empty_purpose_triggers_gdpr_purpose_limitation() {
    let mut engine = engine_all_rules();
    let mut ctx = baseline_ctx();
    ctx.purpose = String::new();

    let verdict = engine.evaluate(&ctx);
    // HIPAA-001 flags empty purpose for clinical + PHI access.
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "HIPAA-001"),
        "Empty purpose must violate HIPAA-001 (minimum necessary)"
    );
    // GDPR-004 flags empty purpose for sensitive data.
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "GDPR-004"),
        "Empty purpose must violate GDPR-004 (purpose limitation)"
    );
}

#[test]
fn empty_action_string_does_not_panic() {
    let mut engine = engine_all_rules();
    let mut ctx = baseline_ctx();
    ctx.action = String::new();

    // Must not panic; action checks use `contains()` which handles empty strings.
    let verdict = engine.evaluate(&ctx);
    assert!(
        verdict.rules_evaluated > 0,
        "Engine should still evaluate rules with empty action"
    );
}

#[test]
fn empty_actor_role_blocks_phi_access() {
    let mut engine = engine_all_rules();
    let mut ctx = baseline_ctx();
    ctx.actor_role = String::new();

    let verdict = engine.evaluate(&ctx);
    // Empty role is not in CLINICAL_ROLES, so HIPAA-001 blocks PHI.
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "HIPAA-001"),
        "Empty role must violate HIPAA-001"
    );
}

#[test]
fn zero_sensitivity_bypasses_phi_rules() {
    let mut engine = engine_all_rules();
    let mut ctx = baseline_ctx();
    ctx.sensitivity = 0;
    ctx.actor = String::new(); // Would normally trigger HIPAA-002

    let verdict = engine.evaluate(&ctx);
    // Sensitivity < 2 means HIPAA-002 (access logging) is not evaluated for PHI.
    assert!(
        !verdict.violations.iter().any(|v| v.rule_id == "HIPAA-002"),
        "Zero sensitivity should skip PHI-gated rules"
    );
}

#[test]
fn zero_record_count_passes_bulk_checks() {
    let mut engine = engine_all_rules();
    let mut ctx = baseline_ctx();
    ctx.record_count = 0;

    let verdict = engine.evaluate(&ctx);
    assert!(
        !verdict.violations.iter().any(|v| v.rule_id == "HIPAA-003"),
        "Zero records should not trigger bulk access detection"
    );
    assert!(
        !verdict.violations.iter().any(|v| v.rule_id == "GDPR-005"),
        "Zero records should not trigger data minimisation"
    );
}

#[test]
fn zero_timestamp_triggers_after_hours_detection() {
    let mut engine = engine_all_rules();
    let mut ctx = baseline_ctx();
    ctx.timestamp = 0; // hour = 0 → outside 6..22

    let verdict = engine.evaluate(&ctx);
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "HIPAA-003"),
        "Timestamp 0 (midnight) must trigger after-hours breach detection"
    );
}

#[test]
fn empty_metadata_blocks_encryption_check() {
    let mut engine = engine_all_rules();
    let mut ctx = baseline_ctx();
    ctx.metadata.clear();

    let verdict = engine.evaluate(&ctx);
    // Missing "encrypted" key for sensitivity >= 2.
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "HIPAA-005"),
        "Empty metadata must violate HIPAA-005 (encryption)"
    );
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "GDPR-007"),
        "Empty metadata must violate GDPR-007 (data protection by design)"
    );
}

#[test]
fn empty_metadata_without_consent_blocks_gdpr_lawful_basis() {
    let mut engine = engine_all_rules();
    let mut ctx = baseline_ctx();
    ctx.metadata.clear();
    ctx.has_consent = false;

    let verdict = engine.evaluate(&ctx);
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "GDPR-003"),
        "No metadata + no consent must violate GDPR-003 (lawful basis)"
    );
}

// ==========================================================================
// 2. BreachDetector — Zero / Empty AccessEvent Fields
// ==========================================================================

#[test]
fn breach_detector_zero_record_count_no_bulk_alert() {
    let mut detector = BreachDetector::new();
    let event = AccessEvent {
        actor: "test".into(),
        actor_role: "clinician".into(),
        action: "data.export".into(),
        target: "patient:1".into(),
        timestamp: 43200,
        record_count: 0,
        sensitivity: 3,
        success: true,
    };

    let alerts = detector.record_event(event);
    assert!(
        !alerts.iter().any(|a| a.alert_type == AlertType::BulkExport),
        "Zero record count export must not trigger bulk export alert"
    );
}

#[test]
fn breach_detector_empty_actor_records_without_panic() {
    let mut detector = BreachDetector::new();
    let event = AccessEvent {
        actor: String::new(),
        actor_role: String::new(),
        action: String::new(),
        target: String::new(),
        timestamp: 0,
        record_count: 0,
        sensitivity: 0,
        success: true,
    };

    // All-zero/empty event must not panic.
    let alerts = detector.record_event(event);
    // Low sensitivity + non-clinical role → no role anomaly (sensitivity < 2).
    assert!(
        !alerts.iter().any(|a| a.alert_type == AlertType::RoleAnomaly),
        "Zero sensitivity should skip role anomaly check"
    );
}

#[test]
fn breach_detector_zero_timestamp_after_hours_for_phi() {
    let mut detector = BreachDetector::new();
    let event = AccessEvent {
        actor: "user".into(),
        actor_role: "clinician".into(),
        action: "record.read".into(),
        target: "patient:1".into(),
        timestamp: 0, // hour = 0 → outside 6..22
        record_count: 1,
        sensitivity: 3,
        success: true,
    };

    let alerts = detector.record_event(event);
    assert!(
        alerts
            .iter()
            .any(|a| a.alert_type == AlertType::AfterHoursAccess),
        "Timestamp 0 with PHI sensitivity must trigger after-hours alert"
    );
}

#[test]
fn breach_detector_zero_sensitivity_skips_after_hours() {
    let mut detector = BreachDetector::new();
    let event = AccessEvent {
        actor: "user".into(),
        actor_role: "clinician".into(),
        action: "record.read".into(),
        target: "resource:1".into(),
        timestamp: 0,
        record_count: 1,
        sensitivity: 0,
        success: true,
    };

    let alerts = detector.record_event(event);
    assert!(
        !alerts
            .iter()
            .any(|a| a.alert_type == AlertType::AfterHoursAccess),
        "Zero sensitivity should skip after-hours detection"
    );
}

#[test]
fn breach_detector_empty_action_skips_export_check() {
    let mut detector = BreachDetector::new();
    let event = AccessEvent {
        actor: "user".into(),
        actor_role: "clinician".into(),
        action: String::new(),
        target: "patient:1".into(),
        timestamp: 43200,
        record_count: 100,
        sensitivity: 3,
        success: true,
    };

    let alerts = detector.record_event(event);
    assert!(
        !alerts.iter().any(|a| a.alert_type == AlertType::BulkExport),
        "Empty action must not match export pattern"
    );
}

// ==========================================================================
// 3. AccessControl — Unknown / Empty Permission Strings
// ==========================================================================

#[test]
fn access_control_empty_permission_denied() {
    let ac = AccessControl::new();
    assert!(
        !ac.check(&Role::Admin, ""),
        "Empty permission string must return false"
    );
}

#[test]
fn policy_aware_empty_permission_denied() {
    let pac = PolicyAwareAccessControl::new().with_verdict(true);
    assert!(
        !pac.check_with_policy(&Role::Admin, ""),
        "Empty permission via policy-aware must return false"
    );
}

// ==========================================================================
// 4. ErasureManager — Empty / Zero-Value Inputs
// ==========================================================================

#[test]
fn erasure_manager_empty_data_subject() {
    let mut mgr = ErasureManager::new();
    mgr.submit_request(String::new(), vec!["records".into()], 1000);

    assert_eq!(mgr.pending_requests().len(), 1);
    // Can complete by matching the empty subject.
    assert!(mgr.complete_request(""));
    assert!(mgr.pending_requests().is_empty());
}

#[test]
fn erasure_manager_empty_data_targets_vec() {
    let mut mgr = ErasureManager::new();
    mgr.submit_request("patient:1".into(), vec![], 1000);

    let pending = mgr.pending_requests();
    assert_eq!(pending.len(), 1);
    assert!(
        pending[0].data_targets.is_empty(),
        "Empty targets vec must be preserved"
    );
}

#[test]
fn erasure_manager_zero_timestamp() {
    let mut mgr = ErasureManager::new();
    mgr.submit_request("patient:1".into(), vec!["data".into()], 0);

    // Not overdue at time 0.
    assert!(mgr.overdue_requests(0).is_empty());
    // Deadline = 0 + 30 days = 2_592_000. Not overdue at deadline.
    assert!(mgr.overdue_requests(2_592_000).is_empty());
    // Overdue one second after deadline.
    assert_eq!(mgr.overdue_requests(2_592_001).len(), 1);
}

#[test]
fn erasure_manager_max_timestamp_no_overflow() {
    let mut mgr = ErasureManager::new();
    mgr.submit_request("patient:1".into(), vec!["data".into()], u64::MAX);

    // Deadline = u64::MAX (saturating_add). Not overdue at u64::MAX.
    assert!(
        mgr.overdue_requests(u64::MAX).is_empty(),
        "Saturated deadline must not cause spurious overdue"
    );
}

#[test]
fn erasure_manager_complete_nonexistent_returns_false() {
    let mut mgr = ErasureManager::new();
    assert!(
        !mgr.complete_request("nonexistent"),
        "Completing a nonexistent subject must return false"
    );
}

#[test]
fn erasure_manager_complete_empty_on_empty_store() {
    let mut mgr = ErasureManager::new();
    assert!(
        !mgr.complete_request(""),
        "Completing empty subject on empty store must return false"
    );
}

// ==========================================================================
// 5. RetentionManager — Zero / Boundary Period Values
// ==========================================================================

#[test]
fn retention_zero_period_purges_immediately() {
    let mut mgr = RetentionManager::new(0);
    mgr.add_policy("zero", 0);
    // created_at = 0 + period 0 = 0 <= now = 0 → should purge.
    assert!(mgr.should_purge(0, "zero", 0));
}

#[test]
fn retention_zero_period_does_not_purge_future_records() {
    let mut mgr = RetentionManager::new(0);
    mgr.add_policy("zero", 0);
    // Even with zero period, a record created at time 5 shouldn't purge at time 4.
    assert!(!mgr.should_purge(5, "zero", 4));
}

#[test]
fn retention_nonexistent_policy_does_not_purge() {
    let mgr = RetentionManager::new(0);
    assert!(
        !mgr.should_purge(0, "nonexistent", u64::MAX),
        "Unknown policy ID must never trigger purge"
    );
}

#[test]
fn retention_empty_policy_id_string() {
    let mut mgr = RetentionManager::new(0);
    mgr.add_policy("", 100);
    // Lookup with empty string should match.
    assert!(!mgr.should_purge(0, "", 50));
    assert!(mgr.should_purge(0, "", 100));
}

// ==========================================================================
// 6. ComplianceAuditLog — Empty / Blank Inputs
// ==========================================================================

#[test]
fn audit_log_record_with_all_empty_strings() {
    let mut log = make_audit_log();
    let seq = log.record(0, "", "", "", "");
    assert_eq!(seq, 1, "Empty-string record must still be assigned seq 1");
    assert_eq!(log.len(), 1);

    let entry = log.get_entry(1).unwrap();
    assert_eq!(entry.actor, "");
    assert_eq!(entry.action, "");
    assert_eq!(entry.target, "");
}

#[test]
fn audit_log_search_empty_keyword() {
    let mut log = make_audit_log();
    log.record(100, "alice", "read", "patient:1", "ok");

    // Empty keyword search should return results or empty — must not panic.
    let _hits = log.search("");
    // Behavior is implementation-defined; the invariant is no panic.
}

#[test]
fn audit_log_query_range_zero_to_zero() {
    let log = make_audit_log();
    let entries = log.query_range(0, 0);
    assert!(entries.is_empty(), "Empty log queried at (0,0) must return empty");
}

#[test]
fn audit_log_zero_timestamp_records() {
    let mut log = make_audit_log();
    log.record(0, "sys", "boot", "node", "ok");
    let entry = log.get_entry(1).unwrap();
    assert_eq!(entry.timestamp, 0);
}

#[test]
fn legacy_audit_log_empty_strings() {
    let mut log = AuditLog::default();
    log.record("", "", "", 0);
    assert_eq!(log.query().len(), 1);
    assert_eq!(log.query()[0].actor, "");
}

// ==========================================================================
// 7. ComplianceVerdictLogger — Zero-Value Context
// ==========================================================================

#[test]
fn verdict_logger_all_empty_context() {
    let mut log = make_audit_log();
    let ctx = OperationContext {
        actor: String::new(),
        actor_role: String::new(),
        action: String::new(),
        target: String::new(),
        timestamp: 0,
        has_consent: false,
        sensitivity: 0,
        jurisdiction: Jurisdiction::US,
        record_count: 0,
        purpose: String::new(),
        metadata: HashMap::new(),
    };

    let mut engine = engine_all_rules();
    let verdict = engine.evaluate(&ctx);

    // Logging must not panic.
    let seq = ComplianceVerdictLogger::log_verdict(&mut log, &ctx, &verdict);
    assert!(seq > 0, "Verdict logger must assign a positive sequence");
}

// ==========================================================================
// 8. RulesEngine — Empty Engine / Report Edge Cases
// ==========================================================================

#[test]
fn empty_engine_with_zero_context_allows() {
    let mut engine = RulesEngine::new();
    let ctx = OperationContext {
        actor: String::new(),
        actor_role: String::new(),
        action: String::new(),
        target: String::new(),
        timestamp: 0,
        has_consent: false,
        sensitivity: 0,
        jurisdiction: Jurisdiction::US,
        record_count: 0,
        purpose: String::new(),
        metadata: HashMap::new(),
    };

    let verdict = engine.evaluate(&ctx);
    assert!(verdict.allowed, "Empty engine must allow any input");
    assert_eq!(verdict.score, 100.0);
    assert_eq!(verdict.rules_evaluated, 0);
}

#[test]
fn report_generation_zero_period() {
    let mut engine = engine_all_rules();
    let ctx = baseline_ctx();
    engine.evaluate(&ctx);

    // Period [0, 0] should not include the event at timestamp 43200.
    let report = engine.generate_report(0, 0, 0, Jurisdiction::Both);
    assert_eq!(report.total_operations, 0);
    assert_eq!(report.aggregate_score, 100.0);
}

// ==========================================================================
// 9. BAATemplate — Blank Fields
// ==========================================================================

#[test]
fn baa_default_template_has_non_empty_fields() {
    let tpl = compliance::baa::BAATemplate::default_template();
    assert!(!tpl.provider.is_empty());
    assert!(!tpl.covered_data.is_empty());
    assert!(!tpl.terms.is_empty());
}
