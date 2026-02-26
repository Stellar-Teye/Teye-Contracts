//! # HIPAA Compliance Rules
//!
//! Implements automated HIPAA rule evaluation for healthcare data operations.
//!
//! ## Covered Rules
//!
//! | Rule ID    | HIPAA Section  | Description                                    |
//! |------------|----------------|------------------------------------------------|
//! | HIPAA-001  | §164.502(b)    | Minimum necessary access                       |
//! | HIPAA-002  | §164.312(b)    | Access logging for all PHI operations           |
//! | HIPAA-003  | §164.308(a)(6) | Breach notification triggers                   |
//! | HIPAA-004  | §164.312(a)(1) | Access control — role-based authorization       |
//! | HIPAA-005  | §164.312(e)(1) | Encryption requirement for sensitive data       |
//! | HIPAA-006  | §164.530(j)    | Retention — PHI audit logs ≥ 6 years            |
//! | HIPAA-007  | §164.502(a)    | Purpose limitation for PHI access               |

use crate::rules_engine::{ComplianceRule, Jurisdiction, OperationContext, Severity};

/// Working hours boundaries (UTC): 06:00 to 22:00.
const WORK_HOURS_START: u64 = 6;
const WORK_HOURS_END: u64 = 22;

/// Bulk access threshold — accessing more than this many records in a single
/// operation triggers a breach detection warning.
const BULK_ACCESS_THRESHOLD: u32 = 50;

/// Maximum PHI sensitivity level that non-clinical roles can access.
const MAX_NON_CLINICAL_SENSITIVITY: u32 = 1;

/// Roles allowed to access PHI (sensitivity >= 2).
const CLINICAL_ROLES: &[&str] = &["clinician", "admin", "emergency"];

/// Permitted purposes for PHI access under HIPAA.
const PERMITTED_PHI_PURPOSES: &[&str] = &[
    "treatment",
    "payment",
    "healthcare_operations",
    "emergency",
    "public_health",
    "judicial",
];

// ── Rule constructors ───────────────────────────────────────────────────────

/// Register all HIPAA rules with the rules engine.
pub fn register_hipaa_rules(engine: &mut crate::rules_engine::RulesEngine) {
    engine.register_rule(minimum_necessary_rule());
    engine.register_rule(access_logging_rule());
    engine.register_rule(breach_notification_rule());
    engine.register_rule(access_control_rule());
    engine.register_rule(encryption_requirement_rule());
    engine.register_rule(retention_rule());
    engine.register_rule(purpose_limitation_rule());
}

/// HIPAA-001: Minimum necessary access (§164.502(b)).
///
/// Non-clinical roles must not access data with sensitivity > 1.
/// Clinical roles accessing PHI must have a stated purpose.
fn minimum_necessary_rule() -> ComplianceRule {
    ComplianceRule {
        id: "HIPAA-001".into(),
        name: "Minimum necessary access".into(),
        jurisdictions: vec![Jurisdiction::US, Jurisdiction::Both],
        severity: Severity::Critical,
        remediation: "Restrict access to the minimum data needed for the stated purpose. \
                      Non-clinical roles should not access PHI."
            .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            // Non-clinical roles cannot access high-sensitivity data.
            let is_clinical = CLINICAL_ROLES.iter().any(|r| *r == ctx.actor_role);
            if !is_clinical && ctx.sensitivity > MAX_NON_CLINICAL_SENSITIVITY {
                return false;
            }

            // Clinical roles must have a purpose for PHI access.
            if is_clinical && ctx.sensitivity >= 2 && ctx.purpose.is_empty() {
                return false;
            }

            true
        }),
    }
}

/// HIPAA-002: Access logging (§164.312(b)).
///
/// All PHI operations must be loggable — the operation must have a
/// non-empty actor and target.
fn access_logging_rule() -> ComplianceRule {
    ComplianceRule {
        id: "HIPAA-002".into(),
        name: "Access logging for PHI operations".into(),
        jurisdictions: vec![Jurisdiction::US, Jurisdiction::Both],
        severity: Severity::Critical,
        remediation: "Ensure all PHI access operations include actor identity and target resource."
            .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            if ctx.sensitivity >= 2 {
                // PHI access must have identifiable actor and target.
                return !ctx.actor.is_empty() && !ctx.target.is_empty();
            }
            true
        }),
    }
}

/// HIPAA-003: Breach notification triggers (§164.308(a)(6)).
///
/// Detects patterns that indicate a potential breach:
/// - Bulk data access (many records at once).
/// - After-hours access to PHI.
/// - Data export operations on PHI.
fn breach_notification_rule() -> ComplianceRule {
    ComplianceRule {
        id: "HIPAA-003".into(),
        name: "Breach detection — unusual access patterns".into(),
        jurisdictions: vec![Jurisdiction::US, Jurisdiction::Both],
        severity: Severity::Warning,
        remediation:
            "Review this access for potential breach. Bulk access, after-hours PHI access, \
                      and data exports require additional justification."
                .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            // Bulk access detection.
            if ctx.record_count > BULK_ACCESS_THRESHOLD {
                return false;
            }

            // After-hours PHI access.
            if ctx.sensitivity >= 2 {
                let hour = (ctx.timestamp / 3600) % 24;
                if hour < WORK_HOURS_START || hour >= WORK_HOURS_END {
                    // After-hours PHI access is flagged (but not blocked).
                    return false;
                }
            }

            // Data export operations on PHI.
            if ctx.action.contains("export") && ctx.sensitivity >= 2 {
                return false;
            }

            true
        }),
    }
}

/// HIPAA-004: Access control (§164.312(a)(1)).
///
/// Only authorized roles may perform write/delete operations on PHI.
fn access_control_rule() -> ComplianceRule {
    ComplianceRule {
        id: "HIPAA-004".into(),
        name: "Role-based access control for PHI".into(),
        jurisdictions: vec![Jurisdiction::US, Jurisdiction::Both],
        severity: Severity::Critical,
        remediation:
            "Only clinicians and admins may modify PHI. Researchers have read-only access.".into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            if ctx.sensitivity >= 2 {
                let is_write = ctx.action.contains("write")
                    || ctx.action.contains("create")
                    || ctx.action.contains("update")
                    || ctx.action.contains("delete");

                if is_write {
                    return ctx.actor_role == "clinician"
                        || ctx.actor_role == "admin"
                        || ctx.actor_role == "emergency";
                }
            }
            true
        }),
    }
}

/// HIPAA-005: Encryption requirement (§164.312(e)(1)).
///
/// Operations on PHI data must indicate encryption is in use.
fn encryption_requirement_rule() -> ComplianceRule {
    ComplianceRule {
        id: "HIPAA-005".into(),
        name: "Encryption required for PHI".into(),
        jurisdictions: vec![Jurisdiction::US, Jurisdiction::Both],
        severity: Severity::Critical,
        remediation:
            "Enable encryption for all PHI data at rest and in transit. Set metadata 'encrypted'=true."
                .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            if ctx.sensitivity >= 2 {
                return ctx
                    .metadata
                    .get("encrypted")
                    .map_or(false, |v| v == "true");
            }
            true
        }),
    }
}

/// HIPAA-006: Retention (§164.530(j)).
///
/// Audit logs for PHI access must be retained for at least 6 years.
/// This rule flags delete operations on audit data.
fn retention_rule() -> ComplianceRule {
    ComplianceRule {
        id: "HIPAA-006".into(),
        name: "PHI audit log retention (6 years)".into(),
        jurisdictions: vec![Jurisdiction::US, Jurisdiction::Both],
        severity: Severity::Critical,
        remediation:
            "HIPAA requires audit logs of PHI access be retained for a minimum of 6 years. \
             Do not delete or purge audit records before this period."
                .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            // Block deletion of audit records.
            if ctx.action.contains("audit") && ctx.action.contains("delete") {
                return false;
            }
            true
        }),
    }
}

/// HIPAA-007: Purpose limitation (§164.502(a)).
///
/// PHI may only be used or disclosed for treatment, payment, or healthcare
/// operations unless the patient has given specific authorization.
fn purpose_limitation_rule() -> ComplianceRule {
    ComplianceRule {
        id: "HIPAA-007".into(),
        name: "Purpose limitation for PHI access".into(),
        jurisdictions: vec![Jurisdiction::US, Jurisdiction::Both],
        severity: Severity::Critical,
        remediation: "PHI access must be for treatment, payment, or healthcare operations. \
                      Other purposes require explicit patient authorization."
            .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            if ctx.sensitivity >= 2 {
                let permitted = PERMITTED_PHI_PURPOSES.iter().any(|p| *p == ctx.purpose);

                // If purpose is not in the permitted list, consent is required.
                if !permitted && !ctx.has_consent {
                    return false;
                }
            }
            true
        }),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules_engine::{Jurisdiction, OperationContext, RulesEngine};
    use std::collections::HashMap;

    fn base_ctx() -> OperationContext {
        OperationContext {
            actor: "dr_jones".into(),
            actor_role: "clinician".into(),
            action: "record.read".into(),
            target: "patient:10".into(),
            timestamp: 43200, // noon UTC
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

    fn engine_with_hipaa() -> RulesEngine {
        let mut engine = RulesEngine::new();
        register_hipaa_rules(&mut engine);
        engine
    }

    #[test]
    fn compliant_clinical_access_passes() {
        let mut engine = engine_with_hipaa();
        let verdict = engine.evaluate(&base_ctx());
        assert!(verdict.allowed, "Violations: {:?}", verdict.violations);
    }

    #[test]
    fn non_clinical_phi_access_blocked() {
        let mut engine = engine_with_hipaa();
        let mut ctx = base_ctx();
        ctx.actor_role = "researcher".into();
        let verdict = engine.evaluate(&ctx);
        assert!(!verdict.allowed);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "HIPAA-001"));
    }

    #[test]
    fn missing_actor_blocked() {
        let mut engine = engine_with_hipaa();
        let mut ctx = base_ctx();
        ctx.actor = "".into();
        let verdict = engine.evaluate(&ctx);
        assert!(!verdict.allowed);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "HIPAA-002"));
    }

    #[test]
    fn bulk_access_flagged() {
        let mut engine = engine_with_hipaa();
        let mut ctx = base_ctx();
        ctx.record_count = 100;
        let verdict = engine.evaluate(&ctx);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "HIPAA-003"));
    }

    #[test]
    fn researcher_write_blocked() {
        let mut engine = engine_with_hipaa();
        let mut ctx = base_ctx();
        ctx.actor_role = "researcher".into();
        ctx.sensitivity = 1; // low sensitivity so HIPAA-001 passes
        ctx.action = "record.write".into();
        let verdict = engine.evaluate(&ctx);
        // Researcher cannot write even at low sensitivity PHI
        // Actually sensitivity < 2 means HIPAA-004 allows it
        // Let's test with sensitivity 2
        ctx.sensitivity = 2;
        let verdict = engine.evaluate(&ctx);
        assert!(!verdict.allowed);
    }

    #[test]
    fn missing_encryption_blocked() {
        let mut engine = engine_with_hipaa();
        let mut ctx = base_ctx();
        ctx.metadata.clear();
        let verdict = engine.evaluate(&ctx);
        assert!(!verdict.allowed);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "HIPAA-005"));
    }

    #[test]
    fn audit_delete_blocked() {
        let mut engine = engine_with_hipaa();
        let mut ctx = base_ctx();
        ctx.action = "audit.delete".into();
        let verdict = engine.evaluate(&ctx);
        assert!(!verdict.allowed);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "HIPAA-006"));
    }

    #[test]
    fn unauthorized_purpose_without_consent_blocked() {
        let mut engine = engine_with_hipaa();
        let mut ctx = base_ctx();
        ctx.purpose = "marketing".into();
        ctx.has_consent = false;
        let verdict = engine.evaluate(&ctx);
        assert!(!verdict.allowed);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "HIPAA-007"));
    }

    #[test]
    fn unauthorized_purpose_with_consent_allowed() {
        let mut engine = engine_with_hipaa();
        let mut ctx = base_ctx();
        ctx.purpose = "marketing".into();
        ctx.has_consent = true;
        let verdict = engine.evaluate(&ctx);
        // Should pass because consent overrides purpose limitation.
        assert!(!verdict.violations.iter().any(|v| v.rule_id == "HIPAA-007"));
    }
}
