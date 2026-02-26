//! # GDPR Compliance Rules
//!
//! Implements automated GDPR rule evaluation for data operations involving
//! EU residents' personal data.
//!
//! ## Covered Rules
//!
//! | Rule ID    | GDPR Article | Description                                     |
//! |------------|-------------|-------------------------------------------------|
//! | GDPR-001   | Art. 17     | Right to erasure ("right to be forgotten")       |
//! | GDPR-002   | Art. 20     | Data portability                                 |
//! | GDPR-003   | Art. 6/7    | Consent tracking — lawful basis required         |
//! | GDPR-004   | Art. 5(1)(b)| Purpose limitation                               |
//! | GDPR-005   | Art. 5(1)(c)| Data minimisation                                |
//! | GDPR-006   | Art. 33     | Breach notification (72-hour window)             |
//! | GDPR-007   | Art. 25     | Data protection by design (encryption)           |

use crate::rules_engine::{ComplianceRule, Jurisdiction, OperationContext, Severity};

/// Maximum number of data fields that should be accessed in a single read
/// operation under the data-minimisation principle.
const MINIMISATION_FIELD_LIMIT: u32 = 20;

/// Permitted lawful bases for processing personal data under GDPR Art. 6.
const LAWFUL_BASES: &[&str] = &[
    "consent",
    "contract",
    "legal_obligation",
    "vital_interests",
    "public_task",
    "legitimate_interests",
];

// ── Rule constructors ───────────────────────────────────────────────────────

/// Register all GDPR rules with the rules engine.
pub fn register_gdpr_rules(engine: &mut crate::rules_engine::RulesEngine) {
    engine.register_rule(right_to_erasure_rule());
    engine.register_rule(data_portability_rule());
    engine.register_rule(consent_tracking_rule());
    engine.register_rule(purpose_limitation_rule());
    engine.register_rule(data_minimisation_rule());
    engine.register_rule(breach_notification_rule());
    engine.register_rule(data_protection_by_design_rule());
}

/// GDPR-001: Right to erasure (Art. 17).
///
/// When a data subject requests erasure, the system must not block the
/// operation. Erasure requests must target specific patient data and be
/// performed by the patient or an admin.
fn right_to_erasure_rule() -> ComplianceRule {
    ComplianceRule {
        id: "GDPR-001".into(),
        name: "Right to erasure — must honor deletion requests".into(),
        jurisdictions: vec![Jurisdiction::EU, Jurisdiction::Both],
        severity: Severity::Critical,
        remediation: "Erasure requests from data subjects must be fulfilled within 30 days. \
                      Ensure the deletion pipeline fully purges all copies of the data."
            .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            // If this is an erasure request, it must be from the patient or admin.
            if ctx.action.contains("erase") || ctx.action.contains("purge") {
                let allowed_roles = ["patient", "admin"];
                return allowed_roles.iter().any(|r| *r == ctx.actor_role);
            }
            true
        }),
    }
}

/// GDPR-002: Data portability (Art. 20).
///
/// Data export must produce machine-readable format and be requested by
/// the data subject or on their behalf with consent.
fn data_portability_rule() -> ComplianceRule {
    ComplianceRule {
        id: "GDPR-002".into(),
        name: "Data portability — export in machine-readable format".into(),
        jurisdictions: vec![Jurisdiction::EU, Jurisdiction::Both],
        severity: Severity::Warning,
        remediation: "Data export must be in a structured, commonly used, machine-readable format \
                      (e.g. JSON, XML). The data subject must authorize the export."
            .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            if ctx.action.contains("export") {
                // Export must have consent from the data subject.
                if !ctx.has_consent {
                    return false;
                }
                // Export format should be specified.
                if !ctx.metadata.contains_key("export_format") {
                    return false;
                }
            }
            true
        }),
    }
}

/// GDPR-003: Consent tracking (Art. 6/7).
///
/// Processing personal data requires a lawful basis. If the basis is
/// "consent", it must be explicitly recorded.
fn consent_tracking_rule() -> ComplianceRule {
    ComplianceRule {
        id: "GDPR-003".into(),
        name: "Lawful basis required for data processing".into(),
        jurisdictions: vec![Jurisdiction::EU, Jurisdiction::Both],
        severity: Severity::Critical,
        remediation: "Ensure a lawful basis (Art. 6) is documented for every personal data \
                      processing operation. If basis is consent, ensure it is freely given, \
                      specific, informed, and unambiguous."
            .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            if ctx.sensitivity >= 1 {
                // Must have a documented lawful basis.
                let basis = ctx.metadata.get("lawful_basis");
                match basis {
                    Some(b) => LAWFUL_BASES.iter().any(|lb| *lb == b.as_str()),
                    None => {
                        // Fall back: if consent is recorded, that counts.
                        ctx.has_consent
                    }
                }
            } else {
                true
            }
        }),
    }
}

/// GDPR-004: Purpose limitation (Art. 5(1)(b)).
///
/// Personal data must be collected for specified, explicit, and legitimate
/// purposes and not further processed in a manner incompatible.
fn purpose_limitation_rule() -> ComplianceRule {
    ComplianceRule {
        id: "GDPR-004".into(),
        name: "Purpose limitation — data used only for stated purpose".into(),
        jurisdictions: vec![Jurisdiction::EU, Jurisdiction::Both],
        severity: Severity::Critical,
        remediation:
            "Every data processing operation must have a stated purpose. \
                      Data must not be used for purposes incompatible with the original collection."
                .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            if ctx.sensitivity >= 1 {
                // Must have a non-empty purpose.
                return !ctx.purpose.is_empty();
            }
            true
        }),
    }
}

/// GDPR-005: Data minimisation (Art. 5(1)(c)).
///
/// Only the minimum necessary data should be processed.
fn data_minimisation_rule() -> ComplianceRule {
    ComplianceRule {
        id: "GDPR-005".into(),
        name: "Data minimisation — collect only what is necessary".into(),
        jurisdictions: vec![Jurisdiction::EU, Jurisdiction::Both],
        severity: Severity::Warning,
        remediation: "Limit data access to the minimum fields necessary for the stated purpose. \
                      Avoid bulk retrieval when specific fields suffice."
            .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            // Flag operations that access too many records.
            if ctx.record_count > MINIMISATION_FIELD_LIMIT {
                return false;
            }
            true
        }),
    }
}

/// GDPR-006: Breach notification (Art. 33).
///
/// Personal data breaches must be reported to the supervisory authority
/// within 72 hours. This rule flags suspicious patterns.
fn breach_notification_rule() -> ComplianceRule {
    ComplianceRule {
        id: "GDPR-006".into(),
        name: "Breach detection — flag suspicious access patterns".into(),
        jurisdictions: vec![Jurisdiction::EU, Jurisdiction::Both],
        severity: Severity::Warning,
        remediation: "If a personal data breach is detected, notify the supervisory authority \
                      within 72 hours (Art. 33) and affected data subjects without undue delay \
                      if high risk (Art. 34)."
            .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            // Bulk data export is suspicious.
            if ctx.action.contains("export") && ctx.record_count > 10 {
                return false;
            }

            // After-hours access to sensitive data.
            if ctx.sensitivity >= 2 {
                let hour = (ctx.timestamp / 3600) % 24;
                if hour < 6 || hour >= 22 {
                    return false;
                }
            }

            true
        }),
    }
}

/// GDPR-007: Data protection by design (Art. 25).
///
/// Appropriate technical measures (encryption, pseudonymisation) must be
/// implemented for personal data processing.
fn data_protection_by_design_rule() -> ComplianceRule {
    ComplianceRule {
        id: "GDPR-007".into(),
        name: "Data protection by design — encryption required".into(),
        jurisdictions: vec![Jurisdiction::EU, Jurisdiction::Both],
        severity: Severity::Critical,
        remediation: "Implement appropriate technical measures including encryption at rest \
                      and in transit, pseudonymisation where feasible, and access controls."
            .into(),
        evaluate: Box::new(|ctx: &OperationContext| {
            if ctx.sensitivity >= 2 {
                return ctx.metadata.get("encrypted").map_or(false, |v| v == "true");
            }
            true
        }),
    }
}

// ── Data erasure helper ─────────────────────────────────────────────────────

/// Represents a data erasure request under GDPR Art. 17.
#[derive(Debug, Clone)]
pub struct ErasureRequest {
    /// The data subject requesting erasure.
    pub data_subject: String,
    /// Target data identifiers to be erased.
    pub data_targets: Vec<String>,
    /// Unix timestamp of the request.
    pub requested_at: u64,
    /// Deadline for erasure (requested_at + 30 days).
    pub deadline: u64,
    /// Whether the erasure has been completed.
    pub completed: bool,
}

impl ErasureRequest {
    /// Create a new erasure request with a 30-day deadline.
    pub fn new(data_subject: String, data_targets: Vec<String>, now: u64) -> Self {
        const THIRTY_DAYS: u64 = 30 * 24 * 3600;
        Self {
            data_subject,
            data_targets,
            requested_at: now,
            deadline: now.saturating_add(THIRTY_DAYS),
            completed: false,
        }
    }

    /// Check if the erasure deadline has passed.
    pub fn is_overdue(&self, now: u64) -> bool {
        !self.completed && now > self.deadline
    }

    /// Mark the erasure as completed.
    pub fn mark_completed(&mut self) {
        self.completed = true;
    }
}

/// Manages pending erasure requests.
#[derive(Default)]
pub struct ErasureManager {
    pub requests: Vec<ErasureRequest>,
}

impl ErasureManager {
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
        }
    }

    /// Submit a new erasure request.
    pub fn submit_request(&mut self, data_subject: String, data_targets: Vec<String>, now: u64) {
        self.requests
            .push(ErasureRequest::new(data_subject, data_targets, now));
    }

    /// Get all overdue (unfulfilled) erasure requests.
    pub fn overdue_requests(&self, now: u64) -> Vec<&ErasureRequest> {
        self.requests.iter().filter(|r| r.is_overdue(now)).collect()
    }

    /// Get all pending (incomplete) requests.
    pub fn pending_requests(&self) -> Vec<&ErasureRequest> {
        self.requests.iter().filter(|r| !r.completed).collect()
    }

    /// Complete an erasure request for a specific data subject.
    pub fn complete_request(&mut self, data_subject: &str) -> bool {
        for req in &mut self.requests {
            if req.data_subject == data_subject && !req.completed {
                req.mark_completed();
                return true;
            }
        }
        false
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules_engine::{Jurisdiction, OperationContext, RulesEngine};
    use std::collections::HashMap;

    fn eu_ctx() -> OperationContext {
        OperationContext {
            actor: "patient_01".into(),
            actor_role: "clinician".into(),
            action: "record.read".into(),
            target: "patient:01".into(),
            timestamp: 43200, // noon UTC
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

    fn engine_with_gdpr() -> RulesEngine {
        let mut engine = RulesEngine::new();
        register_gdpr_rules(&mut engine);
        engine
    }

    #[test]
    fn compliant_eu_access_passes() {
        let mut engine = engine_with_gdpr();
        let verdict = engine.evaluate(&eu_ctx());
        assert!(verdict.allowed, "Violations: {:?}", verdict.violations);
    }

    #[test]
    fn erasure_by_unauthorized_role_blocked() {
        let mut engine = engine_with_gdpr();
        let mut ctx = eu_ctx();
        ctx.action = "data.erase".into();
        ctx.actor_role = "researcher".into();
        let verdict = engine.evaluate(&ctx);
        assert!(!verdict.allowed);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "GDPR-001"));
    }

    #[test]
    fn erasure_by_patient_allowed() {
        let mut engine = engine_with_gdpr();
        let mut ctx = eu_ctx();
        ctx.action = "data.erase".into();
        ctx.actor_role = "patient".into();
        let verdict = engine.evaluate(&ctx);
        assert!(!verdict.violations.iter().any(|v| v.rule_id == "GDPR-001"));
    }

    #[test]
    fn export_without_consent_flagged() {
        let mut engine = engine_with_gdpr();
        let mut ctx = eu_ctx();
        ctx.action = "data.export".into();
        ctx.has_consent = false;
        ctx.metadata.insert("export_format".into(), "json".into());
        let verdict = engine.evaluate(&ctx);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "GDPR-002"));
    }

    #[test]
    fn missing_lawful_basis_and_consent_blocked() {
        let mut engine = engine_with_gdpr();
        let mut ctx = eu_ctx();
        ctx.metadata.remove("lawful_basis");
        ctx.has_consent = false;
        let verdict = engine.evaluate(&ctx);
        assert!(!verdict.allowed);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "GDPR-003"));
    }

    #[test]
    fn empty_purpose_blocked() {
        let mut engine = engine_with_gdpr();
        let mut ctx = eu_ctx();
        ctx.purpose = "".into();
        let verdict = engine.evaluate(&ctx);
        assert!(!verdict.allowed);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "GDPR-004"));
    }

    #[test]
    fn bulk_access_flagged() {
        let mut engine = engine_with_gdpr();
        let mut ctx = eu_ctx();
        ctx.record_count = 30;
        let verdict = engine.evaluate(&ctx);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "GDPR-005"));
    }

    #[test]
    fn missing_encryption_blocked() {
        let mut engine = engine_with_gdpr();
        let mut ctx = eu_ctx();
        ctx.metadata.remove("encrypted");
        let verdict = engine.evaluate(&ctx);
        assert!(!verdict.allowed);
        assert!(verdict.violations.iter().any(|v| v.rule_id == "GDPR-007"));
    }

    #[test]
    fn erasure_manager_lifecycle() {
        let mut mgr = ErasureManager::new();
        mgr.submit_request("patient:01".into(), vec!["records".into()], 1000);

        assert_eq!(mgr.pending_requests().len(), 1);
        assert!(mgr.overdue_requests(1000).is_empty());

        // 31 days later — overdue.
        let overdue_time = 1000 + 31 * 24 * 3600;
        assert_eq!(mgr.overdue_requests(overdue_time).len(), 1);

        // Complete it.
        assert!(mgr.complete_request("patient:01"));
        assert!(mgr.pending_requests().is_empty());
        assert!(mgr.overdue_requests(overdue_time).is_empty());
    }
}
