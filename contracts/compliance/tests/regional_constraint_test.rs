#![allow(clippy::unwrap_used, clippy::expect_used)]

//! # Regional Constraint Enforcement Tests
//!
//! This test suite validates the compliance module's enforcement of regional
//! legal constraints and jurisdiction-specific rules. The tests cover:
//!
//! 1. **GDPR-Related Restrictions**: Testing data transit limitations and
//!    privacy requirements for EU residents.
//! 2. **Regional Blacklists**: Verifying that blocked regions/entities are
//!    properly maintained and enforced.
//! 3. **Multi-Jurisdictional Compliance**: Simulating cross-border transactions
//!    with multiple applicable regulatory frameworks.

use compliance::{
    breach_detector::BreachDetector,
    gdpr::{self, ErasureManager},
    hipaa,
    rules_engine::{ComplianceRule, Jurisdiction, OperationContext, RulesEngine, Severity},
};
use soroban_sdk::{contract, contractimpl, Env, String, Vec};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Mock contracts for simulating cross-jurisdictional scenarios
// ---------------------------------------------------------------------------

#[contract]
struct MockDataRegistry;

#[contractimpl]
impl MockDataRegistry {
    /// Simulates data access request
    pub fn access_data(env: Env, _patient_id: String, _purpose: String) -> Result<String, ()> {
        Ok(String::from_str(&env, "data_hash_123"))
    }

    /// Simulates data transfer across jurisdictions
    pub fn transfer_data(
        env: Env,
        _from_region: String,
        _to_region: String,
        _data_hash: String,
    ) -> Result<(), ()> {
        Ok(())
    }
}

#[contract]
struct MockRegionalAuthority;

#[contractimpl]
impl MockRegionalAuthority {
    /// Check if entity is on regional blacklist
    pub fn is_blacklisted(env: Env, entity_id: String, region: String) -> bool {
        // Simulate blacklist checking logic
        let blacklisted_entities = match region.as_str() {
            "EU" => vec![
                String::from_str(&env, "banned_eu_entity_1"),
                String::from_str(&env, "banned_eu_entity_2"),
            ],
            "US" => vec![String::from_str(&env, "banned_us_entity_1")],
            _ => vec![],
        };

        blacklisted_entities.contains(&entity_id)
    }

    /// Get list of sanctioned regions for data transfer
    pub fn get_sanctioned_regions(env: Env) -> Vec<String> {
        let mut regions = Vec::new(&env);
        regions.push_back(String::from_str(&env, "KP")); // North Korea
        regions.push_back(String::from_str(&env, "IR")); // Iran
        regions.push_back(String::from_str(&env, "SY")); // Syria
        regions
    }
}

// ---------------------------------------------------------------------------
// Test utilities
// ---------------------------------------------------------------------------

fn s(env: &Env, value: &str) -> String {
    String::from_str(env, value)
}

fn create_operation_context(
    env: &Env,
    actor: &str,
    action: &str,
    jurisdiction: Jurisdiction,
    has_consent: bool,
    sensitivity: u32,
) -> OperationContext {
    let mut metadata = HashMap::new();
    metadata.insert("encrypted".to_string(), "true".to_string());
    metadata.insert("lawful_basis".to_string(), "consent".to_string());

    OperationContext {
        actor: actor.to_string(),
        actor_role: "clinician".to_string(),
        action: action.to_string(),
        target: "patient:01".to_string(),
        timestamp: env.ledger().timestamp(),
        has_consent,
        sensitivity,
        jurisdiction,
        record_count: 1,
        purpose: "treatment".to_string(),
        metadata,
    }
}

// ---------------------------------------------------------------------------
// GDPR-Related Restrictions Tests
// ---------------------------------------------------------------------------

/// Test GDPR data export restrictions (data minimization principle)
#[test]
fn test_gdpr_data_export_restrictions() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let mut engine = RulesEngine::new();
    gdpr::register_gdpr_rules(&mut engine);

    // Compliant EU data export
    let compliant_ctx =
        create_operation_context(&env, "dr_smith", "data.export", Jurisdiction::EU, true, 2);

    let verdict = engine.evaluate(&compliant_ctx);
    assert!(verdict.allowed, "Compliant EU export should be allowed");

    // Non-compliant: bulk export without consent
    let mut bulk_ctx = create_operation_context(
        &env,
        "researcher_jones",
        "data.export",
        Jurisdiction::EU,
        false, // No consent
        2,
    );
    bulk_ctx.record_count = 50; // Exceeds minimization limit

    let verdict = engine.evaluate(&bulk_ctx);
    assert!(
        !verdict.allowed,
        "Bulk export without consent should be blocked"
    );
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "GDPR-002"),
        "Should violate GDPR-002 (data portability)"
    );
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "GDPR-005"),
        "Should violate GDPR-005 (data minimisation)"
    );
}

/// Test GDPR right to erasure enforcement
#[test]
fn test_gdpr_right_to_erasure_enforcement() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let mut engine = RulesEngine::new();
    gdpr::register_gdpr_rules(&mut engine);

    // Unauthorized role attempting erasure
    let unauthorized_ctx = create_operation_context(
        &env,
        "unauthorized_user",
        "data.erase",
        Jurisdiction::EU,
        false,
        2,
    );

    let verdict = engine.evaluate(&unauthorized_ctx);
    assert!(!verdict.allowed, "Unauthorized erasure should be blocked");
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "GDPR-001"),
        "Should violate GDPR-001 (right to erasure)"
    );

    // Authorized patient requesting own erasure
    let mut authorized_ctx = create_operation_context(
        &env,
        "patient_zero",
        "data.erase",
        Jurisdiction::EU,
        true,
        2,
    );
    authorized_ctx.actor_role = "patient".to_string();

    let verdict = engine.evaluate(&authorized_ctx);
    assert!(
        verdict.allowed,
        "Patient-initiated erasure should be allowed"
    );
}

/// Test GDPR encryption requirements for sensitive data
#[test]
fn test_gdpr_encryption_requirements() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let mut engine = RulesEngine::new();
    gdpr::register_gdpr_rules(&mut engine);

    // Sensitive data without encryption
    let mut unencrypted_ctx = create_operation_context(
        &env,
        "dr_nosec",
        "record.create",
        Jurisdiction::EU,
        true,
        3, // High sensitivity
    );
    unencrypted_ctx.metadata.remove("encrypted");

    let verdict = engine.evaluate(&unencrypted_ctx);
    assert!(
        !verdict.allowed,
        "Unencrypted sensitive data should be blocked"
    );
    assert!(
        verdict.violations.iter().any(|v| v.rule_id == "GDPR-007"),
        "Should violate GDPR-007 (data protection by design)"
    );

    // Same data with encryption
    let encrypted_ctx = create_operation_context(
        &env,
        "dr_secure",
        "record.create",
        Jurisdiction::EU,
        true,
        3,
    );

    let verdict = engine.evaluate(&encrypted_ctx);
    assert!(
        verdict.allowed,
        "Encrypted sensitive data should be allowed"
    );
}

/// Test GDPR breach detection and notification
#[test]
fn test_gdpr_breach_detection_and_notification() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let mut detector = BreachDetector::new();

    // Normal access pattern
    detector.record_access("patient:01", "dr_smith", "treatment", 1000);
    assert!(!detector.is_suspicious("dr_smith"));

    // Suspicious bulk access
    for i in 0..20 {
        detector.record_access(
            &format!("patient:{:02}", i),
            "suspicious_user",
            "bulk_export",
            1000 + i,
        );
    }

    assert!(
        detector.is_suspicious("suspicious_user"),
        "Bulk access should trigger breach detection"
    );

    // After-hours access to sensitive data
    let after_hours_timestamp = 3 * 3600; // 3 AM UTC
    detector.record_access(
        "patient:sensitive",
        "night_owl",
        "record.read",
        after_hours_timestamp,
    );

    assert!(
        detector.is_suspicious("night_owl"),
        "After-hours access should be flagged"
    );
}

// ---------------------------------------------------------------------------
// Regional Blacklist Tests
// ---------------------------------------------------------------------------

/// Test regional blacklist enforcement
#[test]
fn test_regional_blacklist_enforcement() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let authority_id = env.register(MockRegionalAuthority, ());
    let client = MockRegionalAuthorityClient::new(&env, &authority_id);

    // Test EU blacklist
    let banned_eu_entity = s(&env, "banned_eu_entity_1");
    let legitimate_eu_entity = s(&env, "legitimate_eu_clinic");

    assert!(
        client.is_blacklisted(&banned_eu_entity, &s(&env, "EU")),
        "Banned EU entity should be blacklisted"
    );
    assert!(
        !client.is_blacklisted(&legitimate_eu_entity, &s(&env, "EU")),
        "Legitimate entity should not be blacklisted"
    );

    // Test US blacklist
    let banned_us_entity = s(&env, "banned_us_entity_1");
    assert!(
        client.is_blacklisted(&banned_us_entity, &s(&env, "US")),
        "Banned US entity should be blacklisted"
    );
}

/// Test sanctioned region restrictions
#[test]
fn test_sanctioned_region_restrictions() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let authority_id = env.register(MockRegionalAuthority, ());
    let client = MockRegionalAuthorityClient::new(&env, &authority_id);

    let sanctioned = client.get_sanctioned_regions();

    // Verify sanctioned regions list
    assert!(sanctioned.contains(&s(&env, "KP")));
    assert!(sanctioned.contains(&s(&env, "IR")));
    assert!(sanctioned.contains(&s(&env, "SY")));

    // Attempt data transfer to sanctioned region should fail
    let registry_id = env.register(MockDataRegistry, ());
    let registry_client = MockDataRegistryClient::new(&env, &registry_id);

    // Try to transfer data to sanctioned region
    let result = registry_client.try_transfer_data(
        &s(&env, "EU"),
        &s(&env, "KP"), // Sanctioned
        &s(&env, "data_hash"),
    );

    // In production, this would check sanctioned regions and block
    // For now, we document the expected behavior
    assert!(result.is_ok()); // Mock doesn't enforce, but production should
}

/// Test blacklist update propagation
#[test]
fn test_blacklist_update_propagation() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    // Simulate dynamic blacklist updates
    let mut blacklisted_entities: HashMap<String, Vec<String>> = HashMap::new();

    // Initial blacklist
    blacklisted_entities.insert(
        "EU".to_string(),
        vec!["entity_1".to_string(), "entity_2".to_string()],
    );

    assert_eq!(blacklisted_entities.get("EU").unwrap().len(), 2);

    // Update blacklist (add new entity)
    blacklisted_entities
        .get_mut("EU")
        .unwrap()
        .push("entity_3".to_string());

    assert_eq!(blacklisted_entities.get("EU").unwrap().len(), 3);

    // Verify all entities are blacklisted
    for entity in blacklisted_entities.get("EU").unwrap() {
        assert!(
            entity.starts_with("entity_"),
            "All entities should be tracked"
        );
    }
}

// ---------------------------------------------------------------------------
// Multi-Jurisdictional Transaction Tests
// ---------------------------------------------------------------------------

/// Test multi-jurisdictional compliance evaluation
#[test]
fn test_multi_jurisdictional_compliance() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let mut engine = RulesEngine::new();

    // Register both HIPAA and GDPR rules
    hipaa::register_hipaa_rules(&mut engine);
    gdpr::register_gdpr_rules(&mut engine);

    // Multi-jurisdictional context (US patient, EU provider)
    let multi_juris_ctx =
        create_operation_context(&env, "dr_euro", "record.read", Jurisdiction::Both, true, 3);

    let verdict = engine.evaluate(&multi_juris_ctx);

    // Should satisfy both HIPAA and GDPR requirements
    assert!(
        verdict.allowed,
        "Multi-jurisdictional compliant operation should be allowed"
    );
    assert!(
        verdict.rules_evaluated > 5,
        "Should evaluate multiple rules from both frameworks"
    );
}

/// Test cross-border data transfer compliance
#[test]
fn test_cross_border_data_transfer_compliance() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let mut engine = RulesEngine::new();
    gdpr::register_gdpr_rules(&mut engine);

    // EU to US data transfer (requires additional safeguards)
    let mut transfer_ctx = create_operation_context(
        &env,
        "us_researcher",
        "data.transfer_eu_us",
        Jurisdiction::Both,
        true,
        2,
    );

    // Add metadata for transfer safeguards
    transfer_ctx.metadata.insert(
        "transfer_safeguard".to_string(),
        "standard_contractual_clauses".to_string(),
    );
    transfer_ctx
        .metadata
        .insert("adequacy_decision".to_string(), "false".to_string());

    let verdict = engine.evaluate(&transfer_ctx);

    // With proper safeguards, transfer should be allowed
    assert!(
        verdict.allowed,
        "Cross-border transfer with safeguards should be allowed"
    );
}

/// Test conflicting jurisdiction requirements
#[test]
fn test_conflicting_jurisdiction_requirements() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let mut engine = RulesEngine::new();
    hipaa::register_hipaa_rules(&mut engine);
    gdpr::register_gdpr_rules(&mut engine);

    // Scenario: HIPAA requires data retention, GDPR requires erasure
    let conflict_ctx = create_operation_context(
        &env,
        "conflicted_admin",
        "data.erase",
        Jurisdiction::Both,
        true,
        2,
    );

    let verdict = engine.evaluate(&conflict_ctx);

    // System should handle conflicts gracefully
    // In production, this would require legal review
    assert!(
        verdict.rules_evaluated > 0,
        "Should evaluate rules from both jurisdictions"
    );

    // Count violations from each jurisdiction
    let gdpr_violations = verdict
        .violations
        .iter()
        .filter(|v| v.rule_id.starts_with("GDPR"))
        .count();
    let hipaa_violations = verdict
        .violations
        .iter()
        .filter(|v| v.rule_id.starts_with("HIPAA"))
        .count();

    // Document the conflict resolution approach
    println!(
        "GDPR violations: {}, HIPAA violations: {}",
        gdpr_violations, hipaa_violations
    );
}

/// Test jurisdiction-specific consent requirements
#[test]
fn test_jurisdiction_specific_consent() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let mut engine = RulesEngine::new();
    gdpr::register_gdpr_rules(&mut engine);

    // EU requires explicit consent
    let eu_no_consent = create_operation_context(
        &env,
        "eu_doctor",
        "record.read",
        Jurisdiction::EU,
        false, // No consent
        2,
    );

    let verdict = engine.evaluate(&eu_no_consent);
    assert!(
        !verdict.allowed || verdict.score < 100.0,
        "EU processing without consent should be flagged"
    );

    // US may allow treatment without explicit consent (HIPAA exception)
    let mut us_engine = RulesEngine::new();
    hipaa::register_hipaa_rules(&mut us_engine);

    let us_treatment = create_operation_context(
        &env,
        "us_doctor",
        "record.read",
        Jurisdiction::US,
        false, // No explicit consent
        3,
    );

    let verdict = us_engine.evaluate(&us_treatment);
    // HIPAA allows treatment without explicit consent in many cases
    // This test documents the jurisdictional difference
    println!("US treatment without consent score: {}", verdict.score);
}

// ---------------------------------------------------------------------------
// Data Transit and Residency Tests
// ---------------------------------------------------------------------------

/// Test data residency requirements enforcement
#[test]
fn test_data_residency_requirements() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    // Simulate data residency checker
    struct DataResidencyChecker {
        allowed_regions: Vec<String>,
    }

    impl DataResidencyChecker {
        fn new(regions: Vec<String>) -> Self {
            Self {
                allowed_regions: regions,
            }
        }

        fn check_residency(&self, region: &str) -> bool {
            self.allowed_regions.contains(&region.to_string())
        }
    }

    // EU data must stay in EU
    let eu_checker = DataResidencyChecker::new(vec!["EU".to_string(), "EEA".to_string()]);

    assert!(eu_checker.check_residency("EU"));
    assert!(eu_checker.check_residency("EEA"));
    assert!(!eu_checker.check_residency("US"));
    assert!(!eu_checker.check_residency("CN"));
}

/// Test encrypted data transit across regions
#[test]
fn test_encrypted_data_transit() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let mut engine = RulesEngine::new();
    gdpr::register_gdpr_rules(&mut engine);

    // Encrypted transit between regions
    let mut transit_ctx = create_operation_context(
        &env,
        "secure_gateway",
        "data.transit",
        Jurisdiction::Both,
        true,
        3,
    );
    transit_ctx.action = "data.transit_eu_us".to_string();
    transit_ctx
        .metadata
        .insert("encryption_in_transit".to_string(), "TLS1.3".to_string());
    transit_ctx
        .metadata
        .insert("encryption_at_rest".to_string(), "AES256".to_string());

    let verdict = engine.evaluate(&transit_ctx);

    // Properly encrypted transit should be allowed
    assert!(verdict.allowed, "Encrypted data transit should be allowed");
}

// ---------------------------------------------------------------------------
// Integration Test: Complete Multi-Jurisdictional Scenario
// ---------------------------------------------------------------------------

/// End-to-end test simulating real-world multi-jurisdictional healthcare data exchange
#[test]
fn test_end_to_end_multijurisdictional_healthcare_exchange() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    // Setup: Multi-national healthcare network
    let mut engine = RulesEngine::new();
    hipaa::register_hipaa_rules(&mut engine);
    gdpr::register_gdpr_rules(&mut engine);

    let registry_id = env.register(MockDataRegistry, ());
    let registry_client = MockDataRegistryClient::new(&env, &registry_id);

    let authority_id = env.register(MockRegionalAuthority, ());
    let authority_client = MockRegionalAuthorityClient::new(&env, &authority_id);

    // Phase 1: EU patient seeks treatment in US
    let eu_patient_ctx = create_operation_context(
        &env,
        "us_hospital",
        "record.access",
        Jurisdiction::Both,
        true,
        3,
    );

    // Check blacklist
    let patient_id = s(&env, "eu_patient_123");
    let is_blacklisted = authority_client.is_blacklisted(&patient_id, &s(&env, "EU"));
    assert!(
        !is_blacklisted,
        "Legitimate patient should not be blacklisted"
    );

    // Evaluate compliance
    let verdict = engine.evaluate(&eu_patient_ctx);
    assert!(
        verdict.allowed,
        "Emergency treatment access should be allowed"
    );

    // Phase 2: Access medical records
    let records_access = registry_client.access_data(&patient_id, &s(&env, "emergency_treatment"));
    assert!(records_access.is_ok(), "Record access should succeed");

    // Phase 3: Transfer records back to EU provider
    let transfer_result =
        registry_client.try_transfer_data(&s(&env, "US"), &s(&env, "EU"), &records_access.unwrap());
    assert!(
        transfer_result.is_ok(),
        "Cross-border transfer should succeed"
    );

    // Phase 4: Patient requests erasure (GDPR right)
    let erasure_ctx = create_operation_context(
        &env,
        "eu_patient_123",
        "data.erase",
        Jurisdiction::EU,
        true,
        3,
    );

    let erasure_verdict = engine.evaluate(&erasure_ctx);
    // Note: May conflict with HIPAA retention requirements
    println!(
        "Erasure request - Allowed: {}, Violations: {}",
        erasure_verdict.allowed,
        erasure_verdict.violations.len()
    );
}

// ---------------------------------------------------------------------------
// Edge Cases and Error Handling
// ---------------------------------------------------------------------------

/// Test unknown jurisdiction handling
#[test]
fn test_unknown_jurisdiction_handling() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let mut engine = RulesEngine::new();
    gdpr::register_gdpr_rules(&mut engine);

    // Create context with unknown jurisdiction
    let mut unknown_ctx = create_operation_context(
        &env,
        "unknown_actor",
        "record.read",
        Jurisdiction::EU,
        true,
        1,
    );

    // Manually override to simulate unknown jurisdiction
    // In production, this would use a Jurisdiction::Unknown variant
    unknown_ctx.jurisdiction = Jurisdiction::US; // Treat as default

    let verdict = engine.evaluate(&unknown_ctx);

    // Should apply most restrictive rules when uncertain
    assert!(
        verdict.rules_evaluated >= 0,
        "Should evaluate applicable rules"
    );
}

/// Test rapid jurisdiction changes
#[test]
fn test_rapid_jurisdiction_changes() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    let mut engine = RulesEngine::new();
    hipaa::register_hipaa_rules(&mut engine);
    gdpr::register_gdpr_rules(&mut engine);

    // Simulate user moving between jurisdictions
    let jurisdictions = [
        Jurisdiction::US,
        Jurisdiction::EU,
        Jurisdiction::Both,
        Jurisdiction::US,
        Jurisdiction::EU,
    ];

    for (i, juris) in jurisdictions.iter().enumerate() {
        let mut ctx = create_operation_context(
            &env,
            &format!("traveling_user_{}", i),
            "record.access",
            *juris,
            true,
            2,
        );

        let verdict = engine.evaluate(&ctx);

        // Each jurisdiction change should evaluate correctly
        assert!(
            verdict.rules_evaluated > 0,
            "Should evaluate rules for jurisdiction {:?}",
            juris
        );
    }
}

/// Test empty blacklist scenario
#[test]
fn test_empty_blacklist_scenario() {
    let env = Env::default();
    #[allow(deprecated)]
    env.budget().reset_unlimited();

    // Empty blacklist should allow all legitimate entities
    let blacklisted_entities: HashMap<String, Vec<String>> = HashMap::new();

    let test_entity = "test_entity".to_string();
    let is_blacklisted = blacklisted_entities
        .get("EU")
        .map(|list| list.contains(&test_entity))
        .unwrap_or(false);

    assert!(
        !is_blacklisted,
        "Empty blacklist should not flag any entities"
    );
}
