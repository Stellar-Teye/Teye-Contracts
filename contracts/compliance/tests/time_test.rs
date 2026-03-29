use compliance::breach_detector::{AccessEvent, AlertType, BreachDetector, BreachDetectorConfig};
use compliance::gdpr::register_gdpr_rules;
use compliance::gdpr::{ErasureManager, ErasureRequest};
use compliance::hipaa::register_hipaa_rules;
use compliance::retention::RetentionManager;
use compliance::rules_engine::{Jurisdiction, OperationContext, RulesEngine};
use std::collections::HashMap;

// ── Helpers ────────────────────────────────────────────────────────────────

const ONE_HOUR: u64 = 3600;
const ONE_DAY: u64 = 86_400;
const THIRTY_DAYS: u64 = 30 * ONE_DAY;

/// A compliant noon-UTC context for HIPAA rules.
fn compliant_us_ctx(timestamp: u64) -> OperationContext {
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
            m
        },
    }
}

/// A compliant noon-UTC context for GDPR rules.
fn compliant_eu_ctx(timestamp: u64) -> OperationContext {
    OperationContext {
        actor: "patient_01".into(),
        actor_role: "clinician".into(),
        action: "record.read".into(),
        target: "patient:01".into(),
        timestamp,
        has_consent: true,
        sensitivity: 2,
        jurisdiction: Jurisdiction::EU,
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

fn normal_access_event(timestamp: u64) -> AccessEvent {
    AccessEvent {
        actor: "dr_smith".into(),
        actor_role: "clinician".into(),
        action: "record.read".into(),
        target: "patient:1".into(),
        timestamp,
        record_count: 1,
        sensitivity: 3,
        success: true,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Retention — Expiry & Time-Bound Purge Logic
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn retention_record_not_purged_before_expiry() {
    let mut rm = RetentionManager::new(0);
    rm.add_policy("phi", ONE_DAY);

    // One second before the retention period elapses.
    assert!(!rm.should_purge(0, "phi", ONE_DAY - 1));
}

#[test]
fn retention_record_purged_exactly_at_expiry() {
    let mut rm = RetentionManager::new(0);
    rm.add_policy("phi", ONE_DAY);

    // Exactly at the expiry boundary: created(0) + period(86400) <= now(86400).
    assert!(rm.should_purge(0, "phi", ONE_DAY));
}

#[test]
fn retention_record_purged_after_expiry() {
    let mut rm = RetentionManager::new(0);
    rm.add_policy("phi", ONE_DAY);

    assert!(rm.should_purge(0, "phi", ONE_DAY + 1));
}

#[test]
fn retention_unknown_policy_never_purges() {
    let mut rm = RetentionManager::new(0);
    rm.add_policy("phi", ONE_DAY);

    // A policy that doesn't exist should never trigger purge.
    assert!(!rm.should_purge(0, "nonexistent", u64::MAX));
}

#[test]
fn retention_saturating_add_prevents_overflow() {
    let mut rm = RetentionManager::new(0);
    rm.add_policy("forever", u64::MAX);

    // created(u64::MAX) + period(u64::MAX) would overflow without saturating_add.
    // saturating_add clamps to u64::MAX, so u64::MAX <= u64::MAX is true.
    assert!(rm.should_purge(u64::MAX, "forever", u64::MAX));
}

#[test]
fn retention_large_created_at_no_overflow() {
    let mut rm = RetentionManager::new(0);
    rm.add_policy("short", 100);

    // created near u64::MAX: saturating_add prevents overflow.
    let created = u64::MAX - 50;
    // u64::MAX - 50 + 100 saturates to u64::MAX. now = u64::MAX → purge.
    assert!(rm.should_purge(created, "short", u64::MAX));
    // now < u64::MAX → not purged because u64::MAX > (u64::MAX - 1).
    assert!(!rm.should_purge(created, "short", u64::MAX - 1));
}

#[test]
fn retention_zero_period_purges_immediately() {
    let mut rm = RetentionManager::new(0);
    rm.add_policy("ephemeral", 0);

    // Zero retention means created + 0 <= now, which is always true when now >= created.
    assert!(rm.should_purge(1000, "ephemeral", 1000));
    assert!(rm.should_purge(1000, "ephemeral", 1001));
    // Even when created == 0.
    assert!(rm.should_purge(0, "ephemeral", 0));
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. GDPR Erasure — 30-Day Deadline & Overdue Detection
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn erasure_deadline_is_30_days_from_request() {
    let now = 1_000_000;
    let req = ErasureRequest::new("patient:01".into(), vec!["records".into()], now);
    assert_eq!(req.deadline, now + THIRTY_DAYS);
    assert_eq!(req.requested_at, now);
    assert!(!req.completed);
}

#[test]
fn erasure_not_overdue_before_deadline() {
    let now = 1_000_000;
    let req = ErasureRequest::new("patient:01".into(), vec!["records".into()], now);

    // Exactly at the deadline: not overdue (now > deadline required, not >=).
    assert!(!req.is_overdue(now + THIRTY_DAYS));
    // One second before deadline.
    assert!(!req.is_overdue(now + THIRTY_DAYS - 1));
}

#[test]
fn erasure_overdue_after_deadline() {
    let now = 1_000_000;
    let req = ErasureRequest::new("patient:01".into(), vec!["records".into()], now);

    // One second past deadline.
    assert!(req.is_overdue(now + THIRTY_DAYS + 1));
}

#[test]
fn erasure_completed_never_overdue() {
    let now = 1_000_000;
    let mut req = ErasureRequest::new("patient:01".into(), vec!["records".into()], now);
    req.mark_completed();

    // Even far in the future, completed requests are not overdue.
    assert!(!req.is_overdue(u64::MAX));
}

#[test]
fn erasure_manager_tracks_overdue_requests() {
    let mut mgr = ErasureManager::new();
    let base = 1_000_000;

    mgr.submit_request("patient:01".into(), vec!["data_a".into()], base);
    mgr.submit_request("patient:02".into(), vec!["data_b".into()], base + ONE_DAY);

    // 31 days after the first request — only first is overdue.
    let check_time = base + THIRTY_DAYS + 1;
    let overdue = mgr.overdue_requests(check_time);
    assert_eq!(overdue.len(), 1);
    assert_eq!(overdue[0].data_subject, "patient:01");

    // 31 days after the second request — both overdue.
    let check_time_2 = base + ONE_DAY + THIRTY_DAYS + 1;
    let overdue_2 = mgr.overdue_requests(check_time_2);
    assert_eq!(overdue_2.len(), 2);
}

#[test]
fn erasure_completing_request_removes_from_overdue() {
    let mut mgr = ErasureManager::new();
    mgr.submit_request("patient:01".into(), vec!["data".into()], 0);

    // Past deadline.
    assert_eq!(mgr.overdue_requests(THIRTY_DAYS + 1).len(), 1);

    // Complete the request.
    assert!(mgr.complete_request("patient:01"));
    assert!(mgr.overdue_requests(THIRTY_DAYS + 1).is_empty());
}

#[test]
fn erasure_deadline_saturating_add_at_max_timestamp() {
    let req = ErasureRequest::new("patient:01".into(), vec!["records".into()], u64::MAX);

    // saturating_add prevents overflow: deadline clamps to u64::MAX.
    assert_eq!(req.deadline, u64::MAX);
    // Since now > deadline is false when both are u64::MAX, request is never overdue.
    assert!(!req.is_overdue(u64::MAX));
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. After-Hours Detection — HIPAA-003 Work-Hour Boundaries
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn hipaa003_during_work_hours_no_violation() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    // Test several timestamps within work hours (6-22 UTC).
    // hour = (timestamp / 3600) % 24
    for hour in 6..22 {
        let ts = hour * ONE_HOUR;
        let ctx = compliant_us_ctx(ts);
        let verdict = engine.evaluate(&ctx);
        assert!(
            !verdict.violations.iter().any(|v| v.rule_id == "HIPAA-003"),
            "Hour {} should be within work hours",
            hour
        );
    }
}

#[test]
fn hipaa003_after_hours_flags_violation() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    // After-hours: 0-5 and 22-23 UTC.
    let after_hours = [0, 1, 2, 3, 4, 5, 22, 23];
    for &hour in &after_hours {
        let ts = hour * ONE_HOUR;
        let ctx = compliant_us_ctx(ts);
        let verdict = engine.evaluate(&ctx);
        assert!(
            verdict.violations.iter().any(|v| v.rule_id == "HIPAA-003"),
            "Hour {} should be after hours and flagged",
            hour
        );
    }
}

#[test]
fn hipaa003_work_hours_boundary_start() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    // Hour 5 (last after-hours) → violation.
    let ctx_5 = compliant_us_ctx(5 * ONE_HOUR);
    let v5 = engine.evaluate(&ctx_5);
    assert!(v5.violations.iter().any(|v| v.rule_id == "HIPAA-003"));

    // Hour 6 (first work hour) → no violation.
    let ctx_6 = compliant_us_ctx(6 * ONE_HOUR);
    let v6 = engine.evaluate(&ctx_6);
    assert!(!v6.violations.iter().any(|v| v.rule_id == "HIPAA-003"));
}

#[test]
fn hipaa003_work_hours_boundary_end() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    // Hour 21 (last work hour) → no violation.
    let ctx_21 = compliant_us_ctx(21 * ONE_HOUR);
    let v21 = engine.evaluate(&ctx_21);
    assert!(!v21.violations.iter().any(|v| v.rule_id == "HIPAA-003"));

    // Hour 22 (first after-hours) → violation.
    let ctx_22 = compliant_us_ctx(22 * ONE_HOUR);
    let v22 = engine.evaluate(&ctx_22);
    assert!(v22.violations.iter().any(|v| v.rule_id == "HIPAA-003"));
}

#[test]
fn hipaa003_timestamp_zero_is_after_hours() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    // timestamp=0 → hour=0 → after hours.
    let ctx = compliant_us_ctx(0);
    let verdict = engine.evaluate(&ctx);
    assert!(verdict.violations.iter().any(|v| v.rule_id == "HIPAA-003"));
}

#[test]
fn hipaa003_max_timestamp_wraps_correctly() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    // u64::MAX / 3600 % 24 — verify it doesn't panic.
    let hour = (u64::MAX / 3600) % 24;
    let ctx = compliant_us_ctx(u64::MAX);
    let verdict = engine.evaluate(&ctx);

    if (6..22).contains(&hour) {
        assert!(!verdict.violations.iter().any(|v| v.rule_id == "HIPAA-003"));
    } else {
        assert!(verdict.violations.iter().any(|v| v.rule_id == "HIPAA-003"));
    }
}

#[test]
fn hipaa003_low_sensitivity_skips_after_hours_check() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    // sensitivity < 2 should not trigger the after-hours branch of HIPAA-003.
    let mut ctx = compliant_us_ctx(0); // midnight = after hours
    ctx.sensitivity = 1;
    ctx.actor_role = "admin".into(); // avoid HIPAA-001 violation
    let verdict = engine.evaluate(&ctx);
    // HIPAA-003 should not fire for low sensitivity even at midnight.
    assert!(!verdict.violations.iter().any(|v| v.rule_id == "HIPAA-003"));
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. After-Hours Detection — GDPR-006 Breach Notification
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn gdpr006_during_work_hours_no_violation() {
    let mut engine = RulesEngine::new();
    register_gdpr_rules(&mut engine);

    let ctx = compliant_eu_ctx(12 * ONE_HOUR); // noon
    let verdict = engine.evaluate(&ctx);
    assert!(!verdict.violations.iter().any(|v| v.rule_id == "GDPR-006"));
}

#[test]
fn gdpr006_after_hours_flags_sensitive_access() {
    let mut engine = RulesEngine::new();
    register_gdpr_rules(&mut engine);

    let ctx = compliant_eu_ctx(2 * ONE_HOUR); // 2 AM
    let verdict = engine.evaluate(&ctx);
    assert!(verdict.violations.iter().any(|v| v.rule_id == "GDPR-006"));
}

#[test]
fn gdpr006_low_sensitivity_no_after_hours_flag() {
    let mut engine = RulesEngine::new();
    register_gdpr_rules(&mut engine);

    let mut ctx = compliant_eu_ctx(2 * ONE_HOUR); // 2 AM
    ctx.sensitivity = 1; // below threshold
    let verdict = engine.evaluate(&ctx);
    assert!(!verdict.violations.iter().any(|v| v.rule_id == "GDPR-006"));
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Breach Detector — Access Spike Time Window (1-Hour Sliding Window)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn access_spike_within_one_hour_triggers_alert() {
    let config = BreachDetectorConfig {
        access_spike_threshold: 3,
        ..Default::default()
    };
    let mut detector = BreachDetector::with_config(config);

    let base_ts = 10_000;
    // 4 events within the same hour → exceeds threshold of 3.
    for i in 0..4 {
        let mut event = normal_access_event(base_ts + i * 60);
        event.actor = "rapid_user".into();
        detector.record_event(event);
    }

    assert!(detector
        .alerts()
        .iter()
        .any(|a| a.alert_type == AlertType::AccessSpike));
}

#[test]
fn access_spike_spread_across_hours_no_alert() {
    let config = BreachDetectorConfig {
        access_spike_threshold: 3,
        ..Default::default()
    };
    let mut detector = BreachDetector::with_config(config);

    // 4 events, but each more than 1 hour apart from the next.
    for i in 0..4 {
        let mut event = normal_access_event(i * (ONE_HOUR + 1));
        event.actor = "slow_user".into();
        detector.record_event(event);
    }

    assert!(!detector
        .alerts()
        .iter()
        .any(|a| a.alert_type == AlertType::AccessSpike));
}

#[test]
fn access_spike_timestamp_zero_saturating_sub() {
    let config = BreachDetectorConfig {
        access_spike_threshold: 2,
        ..Default::default()
    };
    let mut detector = BreachDetector::with_config(config);

    // Events at timestamp 0 — saturating_sub(3600) should produce 0, not underflow.
    for _ in 0..3 {
        let event = normal_access_event(0);
        detector.record_event(event);
    }

    assert!(detector
        .alerts()
        .iter()
        .any(|a| a.alert_type == AlertType::AccessSpike));
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Breach Detector — Brute Force Time Window
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn brute_force_within_window_triggers_alert() {
    let config = BreachDetectorConfig {
        brute_force_threshold: 3,
        brute_force_window: 60, // 1-minute window
        ..Default::default()
    };
    let mut detector = BreachDetector::with_config(config);

    let base_ts = 5000;
    for i in 0..4 {
        let event = AccessEvent {
            actor: "attacker".into(),
            actor_role: "unknown".into(),
            action: "auth.login".into(),
            target: "system".into(),
            timestamp: base_ts + i * 10, // every 10 seconds
            record_count: 0,
            sensitivity: 0,
            success: false,
        };
        detector.record_event(event);
    }

    assert!(detector
        .alerts()
        .iter()
        .any(|a| a.alert_type == AlertType::BruteForce));
}

#[test]
fn brute_force_outside_window_no_alert() {
    let config = BreachDetectorConfig {
        brute_force_threshold: 3,
        brute_force_window: 60,
        ..Default::default()
    };
    let mut detector = BreachDetector::with_config(config);

    // Failures spread more than 60 seconds apart — each one falls outside the window.
    for i in 0..4 {
        let event = AccessEvent {
            actor: "slow_attacker".into(),
            actor_role: "unknown".into(),
            action: "auth.login".into(),
            target: "system".into(),
            timestamp: i * 120, // every 2 minutes
            record_count: 0,
            sensitivity: 0,
            success: false,
        };
        detector.record_event(event);
    }

    assert!(!detector
        .alerts()
        .iter()
        .any(|a| a.alert_type == AlertType::BruteForce));
}

#[test]
fn brute_force_successful_attempts_not_counted() {
    let config = BreachDetectorConfig {
        brute_force_threshold: 3,
        brute_force_window: 300,
        ..Default::default()
    };
    let mut detector = BreachDetector::with_config(config);

    // Mix of successes and failures — only 2 failures, below threshold.
    for i in 0..5 {
        let event = AccessEvent {
            actor: "mixed_user".into(),
            actor_role: "clinician".into(),
            action: "auth.login".into(),
            target: "system".into(),
            timestamp: 1000 + i,
            record_count: 0,
            sensitivity: 0,
            success: i % 3 != 0, // fails at i=0 and i=3
        };
        detector.record_event(event);
    }

    assert!(!detector
        .alerts()
        .iter()
        .any(|a| a.alert_type == AlertType::BruteForce));
}

#[test]
fn brute_force_timestamp_zero_saturating_sub() {
    let config = BreachDetectorConfig {
        brute_force_threshold: 2,
        brute_force_window: 300,
        ..Default::default()
    };
    let mut detector = BreachDetector::with_config(config);

    // Events at timestamp 0 — saturating_sub should handle gracefully.
    for _ in 0..3 {
        let event = AccessEvent {
            actor: "attacker".into(),
            actor_role: "unknown".into(),
            action: "auth.login".into(),
            target: "system".into(),
            timestamp: 0,
            record_count: 0,
            sensitivity: 0,
            success: false,
        };
        detector.record_event(event);
    }

    assert!(detector
        .alerts()
        .iter()
        .any(|a| a.alert_type == AlertType::BruteForce));
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Breach Detector — After-Hours Access Detection
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn breach_detector_after_hours_phi_triggers_alert() {
    let mut detector = BreachDetector::new();

    let event = AccessEvent {
        actor: "dr_late".into(),
        actor_role: "clinician".into(),
        action: "record.read".into(),
        target: "patient:1".into(),
        timestamp: 2 * ONE_HOUR, // 2 AM UTC
        record_count: 1,
        sensitivity: 3,
        success: true,
    };
    let alerts = detector.record_event(event);
    assert!(alerts
        .iter()
        .any(|a| a.alert_type == AlertType::AfterHoursAccess));
}

#[test]
fn breach_detector_within_hours_no_after_hours_alert() {
    let mut detector = BreachDetector::new();

    let event = normal_access_event(12 * ONE_HOUR); // noon
    let alerts = detector.record_event(event);
    assert!(!alerts
        .iter()
        .any(|a| a.alert_type == AlertType::AfterHoursAccess));
}

#[test]
fn breach_detector_work_hours_boundary() {
    let mut detector = BreachDetector::new();

    // Hour 6 (start of work hours) — no alert.
    let event_6 = normal_access_event(6 * ONE_HOUR);
    let alerts_6 = detector.record_event(event_6);
    assert!(!alerts_6
        .iter()
        .any(|a| a.alert_type == AlertType::AfterHoursAccess));

    // Hour 22 (end of work hours) — alert.
    let event_22 = normal_access_event(22 * ONE_HOUR);
    let alerts_22 = detector.record_event(event_22);
    assert!(alerts_22
        .iter()
        .any(|a| a.alert_type == AlertType::AfterHoursAccess));
}

#[test]
fn breach_detector_custom_work_hours() {
    let config = BreachDetectorConfig {
        work_hours_start: 9,
        work_hours_end: 17,
        ..Default::default()
    };
    let mut detector = BreachDetector::with_config(config);

    // Hour 8 (before custom start) — alert.
    let event_8 = normal_access_event(8 * ONE_HOUR);
    let alerts = detector.record_event(event_8);
    assert!(alerts
        .iter()
        .any(|a| a.alert_type == AlertType::AfterHoursAccess));

    // Hour 12 (within custom hours) — no alert.
    let event_12 = normal_access_event(12 * ONE_HOUR);
    let alerts = detector.record_event(event_12);
    assert!(!alerts
        .iter()
        .any(|a| a.alert_type == AlertType::AfterHoursAccess));

    // Hour 17 (custom end) — alert.
    let event_17 = normal_access_event(17 * ONE_HOUR);
    let alerts = detector.record_event(event_17);
    assert!(alerts
        .iter()
        .any(|a| a.alert_type == AlertType::AfterHoursAccess));
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Rules Engine — Report Period Time Filtering
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn report_includes_only_operations_within_period() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    // Evaluate operations at different timestamps.
    let timestamps = [100, 500, 1000, 1500, 2000];
    for &ts in &timestamps {
        let ctx = compliant_us_ctx(ts);
        engine.evaluate(&ctx);
    }

    // Report for period 400..1600 should include 500, 1000, 1500 (3 operations).
    let report = engine.generate_report(400, 1600, 2000, Jurisdiction::US);
    assert_eq!(report.total_operations, 3);
}

#[test]
fn report_empty_period_returns_defaults() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    let ctx = compliant_us_ctx(1000);
    engine.evaluate(&ctx);

    // Report for a period that excludes all operations.
    let report = engine.generate_report(2000, 3000, 3001, Jurisdiction::US);
    assert_eq!(report.total_operations, 0);
    assert_eq!(report.aggregate_score, 100.0);
}

#[test]
fn report_period_boundary_inclusion() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    let ctx = compliant_us_ctx(1000);
    engine.evaluate(&ctx);

    // period_start == timestamp → included (uses <, not <=).
    let report_start = engine.generate_report(1000, 2000, 2001, Jurisdiction::US);
    assert_eq!(report_start.total_operations, 1);

    // period_end == timestamp → included (uses >, not >=).
    let report_end = engine.generate_report(0, 1000, 2001, Jurisdiction::US);
    assert_eq!(report_end.total_operations, 1);

    // period_end < timestamp → excluded.
    let report_excluded = engine.generate_report(0, 999, 2001, Jurisdiction::US);
    assert_eq!(report_excluded.total_operations, 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Multi-Day Timestamp Wraparound — Hour Calculation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn hour_calculation_wraps_correctly_across_days() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    // Day 10, hour 12 (noon) = 10 * 86400 + 12 * 3600 = 907200.
    // (907200 / 3600) % 24 = 252 % 24 = 12 → work hours.
    let ts = 10 * ONE_DAY + 12 * ONE_HOUR;
    let ctx = compliant_us_ctx(ts);
    let verdict = engine.evaluate(&ctx);
    assert!(!verdict.violations.iter().any(|v| v.rule_id == "HIPAA-003"));

    // Day 10, hour 3 (3 AM) = 10 * 86400 + 3 * 3600 = 874800.
    // (874800 / 3600) % 24 = 243 % 24 = 3 → after hours.
    let ts_night = 10 * ONE_DAY + 3 * ONE_HOUR;
    let ctx_night = compliant_us_ctx(ts_night);
    let verdict_night = engine.evaluate(&ctx_night);
    assert!(verdict_night
        .violations
        .iter()
        .any(|v| v.rule_id == "HIPAA-003"));
}

#[test]
fn hour_calculation_at_day_boundary() {
    let mut engine = RulesEngine::new();
    register_hipaa_rules(&mut engine);

    // Exactly at day boundary: timestamp = N * 86400 → hour = 0 → after hours.
    let ts = 5 * ONE_DAY;
    let ctx = compliant_us_ctx(ts);
    let verdict = engine.evaluate(&ctx);
    assert!(verdict.violations.iter().any(|v| v.rule_id == "HIPAA-003"));
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. Combined Time Scenarios — Advancing Ledger Time
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn retention_and_erasure_advance_time_together() {
    // Simulate advancing ledger time and checking both retention and erasure.
    let mut rm = RetentionManager::new(0);
    rm.add_policy("phi", 7 * ONE_DAY); // 7-day retention

    let mut mgr = ErasureManager::new();
    mgr.submit_request("patient:01".into(), vec!["records".into()], 0);

    // Day 5: record not yet purgeable, erasure not yet overdue.
    let day_5 = 5 * ONE_DAY;
    assert!(!rm.should_purge(0, "phi", day_5));
    assert!(mgr.overdue_requests(day_5).is_empty());

    // Day 8: record purgeable (7-day retention), erasure still not overdue (30-day deadline).
    let day_8 = 8 * ONE_DAY;
    assert!(rm.should_purge(0, "phi", day_8));
    assert!(mgr.overdue_requests(day_8).is_empty());

    // Day 31: both purgeable and overdue.
    let day_31 = 31 * ONE_DAY;
    assert!(rm.should_purge(0, "phi", day_31));
    assert_eq!(mgr.overdue_requests(day_31).len(), 1);
}

#[test]
fn breach_detector_events_expire_from_sliding_window() {
    let config = BreachDetectorConfig {
        access_spike_threshold: 3,
        ..Default::default()
    };
    let mut detector = BreachDetector::with_config(config);

    let base_ts = 10_000;

    // 3 events at base_ts (within threshold, no alert).
    for i in 0..3 {
        let mut event = normal_access_event(base_ts + i);
        event.actor = "user_a".into();
        detector.record_event(event);
    }
    assert!(!detector
        .alerts()
        .iter()
        .any(|a| a.alert_type == AlertType::AccessSpike));

    // 1 more event 2 hours later — the old events are outside the 1-hour window.
    let mut late_event = normal_access_event(base_ts + 2 * ONE_HOUR);
    late_event.actor = "user_a".into();
    let alerts = detector.record_event(late_event);
    // Only 1 event in the new window, well below threshold.
    assert!(!alerts
        .iter()
        .any(|a| a.alert_type == AlertType::AccessSpike));
}
