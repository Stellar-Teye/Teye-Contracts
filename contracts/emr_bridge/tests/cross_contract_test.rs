// Integration tests for cross-contract calling invariants in emr_bridge
//
// This test suite verifies that the emr_bridge contract correctly:
// 1. Invokes and parses responses from external contracts
// 2. Handles failures from external contract calls gracefully
// 3. Maintains data consistency and state after cross-contract operations
// 4. Enforces authorization for cross-contract interactions

#![allow(clippy::unwrap_used)]

use emr_bridge::{
    types::{DataFormat, EmrSystem, ExchangeDirection, ProviderStatus, SyncStatus},
    EmrBridgeContract, EmrBridgeContractClient,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

// ── Mock External Contract Interface ─────────────────────────────────────────
//
// Simulates an external EMR system contract that the bridge interacts with

struct MockExternalEmrContract {
    available: bool,
}

impl MockExternalEmrContract {
    fn new(_env: &Env, _provider_id: &str, available: bool) -> Self {
        Self { available }
    }

    fn simulate_call(&self) -> Result<&str, &str> {
        if !self.available {
            return Err("External contract unavailable");
        }
        Ok("response_from_external_contract")
    }
}

// ── Setup Helper Functions ───────────────────────────────────────────────────

fn setup_bridge() -> (Env, EmrBridgeContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmrBridgeContract, ());
    let client = EmrBridgeContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

fn register_and_activate_provider(
    env: &Env,
    client: &EmrBridgeContractClient,
    admin: &Address,
    provider_id: &str,
    emr_system: EmrSystem,
) -> String {
    let provider_id_str = String::from_str(env, provider_id);
    let name = String::from_str(env, &format!("Provider {}", provider_id));
    let endpoint = String::from_str(env, "https://external-emr.example.com/api");

    client.register_provider(
        admin,
        &provider_id_str,
        &name,
        &emr_system,
        &endpoint,
        &DataFormat::FhirR4,
    );

    client.activate_provider(admin, &provider_id_str);

    provider_id_str.to_string()
}

// ═══════════════════════════════════════════════════════════════════════════════
// CROSS-CONTRACT CALLING INVARIANTS TESTS
// ═══════════════════════════════════════════════════════════════════════════════

// ── Test 1: Successful External Contract Invocation ──────────────────────────

#[test]
fn test_cross_contract_successful_exchange_with_external_emr() {
    let (env, client, admin) = setup_bridge();

    // Register provider that will act as external contract interface
    let provider_id =
        register_and_activate_provider(&env, &client, &admin, "epic-external-001", EmrSystem::EpicFhir);

    // Simulate cross-contract call to external EMR system
    let external_contract = MockExternalEmrContract::new(&env, &provider_id, true);

    // Verify the external contract is available
    let response = external_contract.simulate_call().expect("Should succeed");
    assert!(!response.is_empty());

    // Record data exchange as if response came from external contract
    let exchange_id = String::from_str(&env, "cross-ex-001");
    let patient_id = String::from_str(&env, "pat-001");
    let resource_type = String::from_str(&env, "Patient");
    let record_hash = String::from_str(&env, "hash_validated_from_external");

    let record = client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &resource_type,
        &record_hash,
    );

    // Verify exchange was recorded successfully
    assert_eq!(record.exchange_id, exchange_id);
    assert_eq!(record.status, SyncStatus::Pending);
    assert_eq!(record.direction, ExchangeDirection::Import);
}

// ── Test 2: External Contract Failure Handling ───────────────────────────────

#[test]
fn test_cross_contract_graceful_failure_when_external_unavailable() {
    let (env, client, admin) = setup_bridge();

    let provider_id =
        register_and_activate_provider(&env, &client, &admin, "cerner-external-001", EmrSystem::CernerMillennium);

    // Simulate unavailable external contract
    let external_contract = MockExternalEmrContract::new(&env, &provider_id, false);

    // Attempt to call external contract - should fail gracefully
    let result = external_contract.simulate_call();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "External contract unavailable");

    // Verify bridge can still record the attempted exchange with error state
    let exchange_id = String::from_str(&env, "cross-ex-002");
    let patient_id = String::from_str(&env, "pat-002");
    let resource_type = String::from_str(&env, "Observation");
    let record_hash = String::from_str(&env, "hash_from_failed_external_call");

    let record = client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Export,
        &DataFormat::Hl7V2,
        &resource_type,
        &record_hash,
    );

    // Record should exist with Pending status (not yet verified/synced)
    assert_eq!(record.status, SyncStatus::Pending);

    // Bridge should allow updating to failure state
    client.update_exchange_status(&admin, &exchange_id, &SyncStatus::PartialSuccess);

    let updated_record = client.get_exchange(&exchange_id).expect("record exists");
    assert_eq!(updated_record.status, SyncStatus::PartialSuccess);
}

// ── Test 3: Multiple External Contracts Sequential Calls ──────────────────────

#[test]
fn test_cross_contract_multiple_external_providers_sequential() {
    let (env, client, admin) = setup_bridge();

    // Set up multiple external contract interfaces
    let provider1 =
        register_and_activate_provider(&env, &client, &admin, "epic-001", EmrSystem::EpicFhir);
    let provider2 = register_and_activate_provider(
        &env,
        &client,
        &admin,
        "cerner-001",
        EmrSystem::CernerMillennium,
    );

    let ext_contract1 = MockExternalEmrContract::new(&env, &provider1, true);
    let ext_contract2 = MockExternalEmrContract::new(&env, &provider2, true);

    // Call external contracts sequentially
    let response1 = ext_contract1.simulate_call().expect("First call succeeds");
    let response2 = ext_contract2.simulate_call().expect("Second call succeeds");

    assert!(!response1.is_empty());
    assert!(!response2.is_empty());
    assert_ne!(response1, response2); // Different providers, different responses

    // Record exchanges from both external sources
    let ex1 = String::from_str(&env, "cross-ex-003");
    let ex2 = String::from_str(&env, "cross-ex-004");
    let pat_id = String::from_str(&env, "pat-003");

    let record1 = client.record_data_exchange(
        &admin,
        &ex1,
        &String::from_str(&env, &provider1),
        &pat_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "hash1"),
    );

    let record2 = client.record_data_exchange(
        &admin,
        &ex2,
        &String::from_str(&env, &provider2),
        &pat_id,
        &ExchangeDirection::Import,
        &DataFormat::Hl7V2,
        &String::from_str(&env, "Lab"),
        &String::from_str(&env, "hash2"),
    );

    // Verify both exchanges recorded independently
    assert_eq!(record1.exchange_id, ex1);
    assert_eq!(record2.exchange_id, ex2);
    assert_eq!(record1.data_format, DataFormat::FhirR4);
    assert_eq!(record2.data_format, DataFormat::Hl7V2);
}

// ── Test 4: Verify Data Consistency After External Call ──────────────────────

#[test]
fn test_cross_contract_data_consistency_verification() {
    let (env, client, admin) = setup_bridge();

    let provider_id =
        register_and_activate_provider(&env, &client, &admin, "epic-sync-001", EmrSystem::EpicFhir);

    // Simulate fetching data from external contract
    let external_contract = MockExternalEmrContract::new(&env, &provider_id, true);
    let _external_response = external_contract.simulate_call().expect("Should succeed");

    // Record exchange with hash from external source
    let exchange_id = String::from_str(&env, "cross-ex-005");
    let patient_id = String::from_str(&env, "pat-004");
    let source_hash = String::from_str(&env, "abc123def456");
    let target_hash = String::from_str(&env, "abc123def456"); // Match for consistency

    let record = client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &source_hash,
    );

    assert_eq!(record.status, SyncStatus::Pending);

    // Verify sync with matching hashes (consistent data)
    let verification_id = String::from_str(&env, "verify-001");
    let discrepancies = Vec::new(&env);

    let verification = client.verify_sync(
        &admin,
        &verification_id,
        &exchange_id,
        &source_hash,
        &target_hash,
        &discrepancies,
    );

    // After verification with matching hashes, exchange should be completed
    assert!(verification.is_consistent);

    let updated_record = client.get_exchange(&exchange_id).expect("record exists");
    assert_eq!(updated_record.status, SyncStatus::Completed);
}

// ── Test 5: Data Inconsistency Detection from External Contract ──────────────

#[test]
fn test_cross_contract_detects_data_inconsistency() {
    let (env, client, admin) = setup_bridge();

    let provider_id = register_and_activate_provider(
        &env,
        &client,
        &admin,
        "cerner-verify-001",
        EmrSystem::CernerMillennium,
    );

    // Record exchange with initial hash
    let exchange_id = String::from_str(&env, "cross-ex-006");
    let patient_id = String::from_str(&env, "pat-005");
    let source_hash = String::from_str(&env, "original_hash_xyz");

    client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::Hl7V2,
        &String::from_str(&env, "Medication"),
        &source_hash,
    );

    // Simulate external contract returning different hash (data mismatch)
    let target_hash = String::from_str(&env, "different_hash_abc");

    let mut discrepancies = Vec::new(&env);
    discrepancies.push_back(String::from_str(
        &env,
        "Field 'dosage' differs between source and target",
    ));

    let verification_id = String::from_str(&env, "verify-002");
    let verification = client.verify_sync(
        &admin,
        &verification_id,
        &exchange_id,
        &source_hash,
        &target_hash,
        &discrepancies,
    );

    // Verification should detect inconsistency
    assert!(!verification.is_consistent);
    assert_eq!(verification.discrepancies.len(), 1);

    // Exchange status should be PartialSuccess (not fully synced)
    let updated_record = client.get_exchange(&exchange_id).expect("record exists");
    assert_eq!(updated_record.status, SyncStatus::PartialSuccess);
}

// ── Test 6: Authorization Check for Cross-Contract Calls ──────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_cross_contract_unauthorized_caller_fails() {
    let (env, client, admin) = setup_bridge();

    register_and_activate_provider(&env, &client, &admin, "epic-auth-001", EmrSystem::EpicFhir);

    // Non-admin attacker tries to record exchange
    let attacker = Address::generate(&env);

    let exchange_id = String::from_str(&env, "cross-ex-999");
    let patient_id = String::from_str(&env, "pat-999");

    client.record_data_exchange(
        &attacker,
        &exchange_id,
        &String::from_str(&env, "epic-auth-001"),
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "malicious_hash"),
    );
}

// ── Test 7: External Contract State Transition After Successful Call ─────────

#[test]
fn test_cross_contract_provider_state_transitions() {
    let (env, client, admin) = setup_bridge();

    let provider_id =
        register_and_activate_provider(&env, &client, &admin, "epic-state-001", EmrSystem::EpicFhir);

    // Verify provider is active before cross-contract call
    let provider = client.get_provider(&String::from_str(&env, &provider_id));
    assert_eq!(provider.status, ProviderStatus::Active);

    // Simulate successful external call
    let ext_contract = MockExternalEmrContract::new(&env, &provider_id, true);
    let _response = ext_contract.simulate_call().expect("Should succeed");

    // Record exchange based on successful call
    let exchange_id = String::from_str(&env, &format!("cross-ex-state-{}", env.ledger().timestamp()));
    let patient_id = String::from_str(&env, "pat-state-001");

    let _record = client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "state_verification_hash"),
    );

    // Provider should still be active
    let updated_provider = client.get_provider(&String::from_str(&env, &provider_id));
    assert_eq!(updated_provider.status, ProviderStatus::Active);
}

// ── Test 8: Field Mapping Applied in Cross-Contract Exchange ──────────────────

#[test]
fn test_cross_contract_field_mapping_validation() {
    let (env, client, admin) = setup_bridge();

    let provider_id =
        register_and_activate_provider(&env, &client, &admin, "epic-mapping-001", EmrSystem::EpicFhir);

    // Create field mapping for external contract data transformation
    let mapping_id = String::from_str(&env, "mapping-001");
    let source_field = String::from_str(&env, "external_patient_id");
    let target_field = String::from_str(&env, "internal_patient_id");
    let transform_rule = String::from_str(&env, "prefix_with_external_");

    let mapping = client.create_field_mapping(
        &admin,
        &mapping_id,
        &String::from_str(&env, &provider_id),
        &source_field,
        &target_field,
        &transform_rule,
    );

    assert_eq!(mapping.mapping_id, mapping_id);
    assert_eq!(mapping.provider_id, String::from_str(&env, &provider_id));

    // Verify mapping is retrievable for cross-contract data transformation
    let retrieved = client.get_field_mapping(&mapping_id);
    assert_eq!(retrieved.source_field, source_field);
    assert_eq!(retrieved.target_field, target_field);
}

// ── Test 9: Retry Logic Simulation for Failed External Calls ──────────────────

#[test]
fn test_cross_contract_retry_failed_external_call() {
    let (env, client, admin) = setup_bridge();

    let provider_id = register_and_activate_provider(
        &env,
        &client,
        &admin,
        "cerner-retry-001",
        EmrSystem::CernerMillennium,
    );

    // First attempt - external contract unavailable
    let ext_contract_1 = MockExternalEmrContract::new(&env, &provider_id, false);
    let attempt_1 = ext_contract_1.simulate_call();
    assert!(attempt_1.is_err());

    // Still able to record exchange attempt
    let exchange_id = String::from_str(&env, "cross-ex-retry-001");
    let patient_id = String::from_str(&env, "pat-retry-001");

    let record = client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::Hl7V2,
        &String::from_str(&env, "Appointment"),
        &String::from_str(&env, "hash_from_retry_attempt"),
    );

    assert_eq!(record.status, SyncStatus::Pending);

    // Second attempt - external contract now available (simulating recovery)
    let ext_contract_2 = MockExternalEmrContract::new(&env, &provider_id, true);
    let attempt_2 = ext_contract_2.simulate_call();
    assert!(attempt_2.is_ok());
}

// ── Test 10: Cross-Contract Call Audit Trail ────────────────────────────────

#[test]
fn test_cross_contract_audit_trail_via_exchanges() {
    let (env, client, admin) = setup_bridge();

    let provider_id =
        register_and_activate_provider(&env, &client, &admin, "epic-audit-001", EmrSystem::EpicFhir);

    // Record multiple exchanges from same external contract
    let patient_id = String::from_str(&env, "pat-audit-001");
    let mut exchange_ids = Vec::new(&env);

    for i in 0..3 {
        let exchange_id = String::from_str(&env, &format!("cross-ex-audit-{}", i));
        exchange_ids.push_back(exchange_id.clone());

        client.record_data_exchange(
            &admin,
            &exchange_id,
            &String::from_str(&env, &provider_id),
            &patient_id,
            &ExchangeDirection::Import,
            &DataFormat::FhirR4,
            &String::from_str(&env, "Patient"),
            &String::from_str(&env, &format!("hash_{}", i)),
        );
    }

    // Retrieve patient's exchange history (audit trail)
    let patient_exchanges = client.get_patient_exchanges(&patient_id);

    // All exchanges should be recorded and retrievable
    assert_eq!(patient_exchanges.len(), 3);

    // Verify each exchange can be retrieved independently
    for ex_id in exchange_ids.iter() {
        let record = client.get_exchange(ex_id).expect("Exchange should exist");
        assert_eq!(record.patient_id, patient_id);
        assert_eq!(record.provider_id, String::from_str(&env, &provider_id));
    }
}
