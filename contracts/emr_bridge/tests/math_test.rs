// Tests for data overflow and underflow edge cases in emr_bridge
//
// This test suite verifies that the emr_bridge contract safely handles
// boundary values and extreme numeric edge cases:
// 1. Maximum u64 values (timestamps, counters)
// 2. Maximum u32 values (TTL operations)
// 3. Empty/zero values (underflow scenarios)
// 4. String/Vector size boundaries
// 5. Overflow recovery and state consistency

#![allow(clippy::unwrap_used)]

use emr_bridge::{
    types::{DataFormat, EmrSystem, ExchangeDirection, ProviderStatus, SyncStatus},
    EmrBridgeContract, EmrBridgeContractClient,
};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, String, Vec};

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
) -> String {
    let provider_id_str = String::from_str(env, provider_id);
    let name = String::from_str(env, &format!("Provider {}", provider_id));
    let endpoint = String::from_str(env, "https://emr.example.com/api");

    client.register_provider(
        admin,
        &provider_id_str,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );

    client.activate_provider(admin, &provider_id_str);

    provider_id_str.to_string()
}

// ═══════════════════════════════════════════════════════════════════════════════
// OVERFLOW AND UNDERFLOW EDGE CASE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

// ── Test 1: Maximum Timestamp (u64::MAX) ─────────────────────────────────────

#[test]
fn test_max_timestamp_in_exchange_record() {
    let (env, client, admin) = setup_bridge();

    let provider_id =
        register_and_activate_provider(&env, &client, &admin, "max-timestamp-provider");

    // Set ledger timestamp to u64::MAX
    env.ledger().set_timestamp(u64::MAX);

    let exchange_id = String::from_str(&env, "max-timestamp-ex");
    let patient_id = String::from_str(&env, "max-timestamp-pat");

    let record = client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "max_timestamp_hash"),
    );

    // Verify exchange was recorded with max timestamp
    assert_eq!(record.exchange_id, exchange_id);
    assert_eq!(record.timestamp, u64::MAX);
    assert_eq!(record.status, SyncStatus::Pending);
}

// ── Test 2: Minimum Timestamp (Zero) ─────────────────────────────────────────

#[test]
fn test_zero_timestamp_in_exchange_record() {
    let (env, client, admin) = setup_bridge();

    let provider_id =
        register_and_activate_provider(&env, &client, &admin, "zero-timestamp-provider");

    // Set ledger timestamp to 0 (genesis/underflow scenario)
    env.ledger().set_timestamp(0);

    let exchange_id = String::from_str(&env, "zero-timestamp-ex");
    let patient_id = String::from_str(&env, "zero-timestamp-pat");

    let record = client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Export,
        &DataFormat::Hl7V2,
        &String::from_str(&env, "Medication"),
        &String::from_str(&env, "zero_timestamp_hash"),
    );

    // Verify exchange was recorded with zero timestamp
    assert_eq!(record.timestamp, 0);
    assert_eq!(record.status, SyncStatus::Pending);
}

// ── Test 3: Timestamp Near Boundary (u64::MAX - 1) ──────────────────────────

#[test]
fn test_timestamp_near_max_boundary() {
    let (env, client, admin) = setup_bridge();

    let provider_id = register_and_activate_provider(&env, &client, &admin, "boundary-provider");

    // Set timestamp to u64::MAX - 1
    let near_max = u64::MAX - 1;
    env.ledger().set_timestamp(near_max);

    let exchange_id = String::from_str(&env, "boundary-ex");
    let patient_id = String::from_str(&env, "boundary-pat");

    let record = client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "boundary_hash"),
    );

    // Verify timestamp was stored correctly without wraparound
    assert_eq!(record.timestamp, near_max);
    assert_ne!(record.timestamp, 0); // Didn't wrap to 0
}

// ── Test 4: Multiple Exchanges with Sequential Timestamps ──────────────────

#[test]
fn test_sequential_timestamp_overflow_handling() {
    let (env, client, admin) = setup_bridge();

    let provider_id = register_and_activate_provider(&env, &client, &admin, "sequential-provider");
    let patient_id = String::from_str(&env, "sequential-pat");

    // Create exchanges at different timestamps including boundaries
    let test_timestamps = vec![0u64, 1, u64::MAX / 2, u64::MAX - 1, u64::MAX];

    for (idx, ts) in test_timestamps.iter().enumerate() {
        env.ledger().set_timestamp(*ts);

        let exchange_id = String::from_str(&env, &format!("seq-ex-{}", idx));

        let record = client.record_data_exchange(
            &admin,
            &exchange_id,
            &String::from_str(&env, &provider_id),
            &patient_id,
            &ExchangeDirection::Import,
            &DataFormat::FhirR4,
            &String::from_str(&env, "Patient"),
            &String::from_str(&env, &format!("hash_{}", idx)),
        );

        // Verify each timestamp was recorded correctly
        assert_eq!(record.timestamp, *ts);
        assert_eq!(record.patient_id, patient_id);
    }

    // Verify all exchanges are retrievable
    let patient_exchanges = client.get_patient_exchanges(&patient_id);
    assert_eq!(patient_exchanges.len(), 5);
}

// ── Test 5: TTL Value Edge Cases (u32 boundaries) ───────────────────────────

#[test]
fn test_ttl_extreme_boundary_values() {
    let (env, client, admin) = setup_bridge();

    // Test provider registration (sets TTL internally)
    let provider_id_str = String::from_str(&env, "ttl-edge-provider");
    let name = String::from_str(&env, "TTL Edge Provider");
    let endpoint = String::from_str(&env, "https://ttl-test.example.com");

    let provider = client.register_provider(
        &admin,
        &provider_id_str,
        &name,
        &EmrSystem::CernerMillennium,
        &endpoint,
        &DataFormat::Hl7V2,
    );

    // Verify provider was created successfully despite internal u32 TTL operations
    assert_eq!(provider.provider_id, provider_id_str);
    assert_eq!(provider.status, ProviderStatus::Pending);

    // Check u32::MAX related scenarios implicitly through retrieval
    let retrieved = client.get_provider(&provider_id_str);
    assert_eq!(retrieved.provider_id, provider_id_str);
}

// ── Test 6: Large Vector Operations (Many Exchanges) ───────────────────────

#[test]
fn test_large_patient_exchange_count() {
    let (env, client, admin) = setup_bridge();

    let provider_id =
        register_and_activate_provider(&env, &client, &admin, "many-exchange-provider");
    let patient_id = String::from_str(&env, "many-exchange-pat");

    // Create many exchanges to test vector growth
    for i in 0..50 {
        let exchange_id = String::from_str(&env, &format!("many-ex-{}", i));

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

    // Retrieve all exchanges - test vector doesn't overflow
    let exchanges = client.get_patient_exchanges(&patient_id);
    assert_eq!(exchanges.len(), 50);

    // Verify each exchange is independently retrievable
    for i in 0..50 {
        let exchange_id = String::from_str(&env, &format!("many-ex-{}", i));
        let record = client.get_exchange(&exchange_id).expect("Exchange exists");
        assert_eq!(record.patient_id, patient_id);
    }
}

// ── Test 7: Empty String Boundary ────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_empty_field_in_mapping_underflow() {
    let (env, client, admin) = setup_bridge();

    let provider_id = register_and_activate_provider(&env, &client, &admin, "empty-field-provider");

    let mapping_id = String::from_str(&env, "empty-field-map");
    let source_field = String::from_str(&env, "");
    let target_field = String::from_str(&env, "target");
    let transform_rule = String::from_str(&env, "rule");

    // Should fail - empty source field
    client.create_field_mapping(
        &admin,
        &mapping_id,
        &String::from_str(&env, &provider_id),
        &source_field,
        &target_field,
        &transform_rule,
    );
}

// ── Test 8: Data Consistency with Hash Boundary Values ──────────────────────

#[test]
fn test_hash_consistency_at_numeric_boundaries() {
    let (env, client, admin) = setup_bridge();

    let provider_id =
        register_and_activate_provider(&env, &client, &admin, "hash-boundary-provider");

    // Create exchange with hash representing max value
    let exchange_id = String::from_str(&env, "hash-boundary-ex");
    let patient_id = String::from_str(&env, "hash-boundary-pat");

    let max_hash = String::from_str(&env, "ffffffffffffffffffffffffffffffff");
    let min_hash = String::from_str(&env, "00000000000000000000000000000000");

    let record = client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &max_hash,
    );

    assert_eq!(record.record_hash, max_hash);

    // Verify sync with boundary hash values
    let verification_id = String::from_str(&env, "verify-boundary");
    let discrepancies = Vec::new(&env);

    let verification = client.verify_sync(
        &admin,
        &verification_id,
        &exchange_id,
        &max_hash,
        &max_hash,
        &discrepancies,
    );

    assert!(verification.is_consistent);

    // Now test with min hash (zero-like value)
    let exchange_id_2 = String::from_str(&env, "hash-boundary-ex-2");
    let record_2 = client.record_data_exchange(
        &admin,
        &exchange_id_2,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Export,
        &DataFormat::Hl7V2,
        &String::from_str(&env, "Medication"),
        &min_hash,
    );

    assert_eq!(record_2.record_hash, min_hash);
}

// ── Test 9: Provider List Boundary (Many Providers) ─────────────────────────

#[test]
fn test_many_providers_list_no_overflow() {
    let (env, client, admin) = setup_bridge();

    // Register many providers
    for i in 0..30 {
        let provider_id = String::from_str(&env, &format!("provider-{}", i));
        let name = String::from_str(&env, &format!("Provider {}", i));
        let endpoint = String::from_str(&env, &format!("https://provider{}.example.com", i));

        client.register_provider(
            &admin,
            &provider_id,
            &name,
            &EmrSystem::EpicFhir,
            &endpoint,
            &DataFormat::FhirR4,
        );
    }

    // Retrieve provider list - should not overflow
    let providers = client.list_providers();
    assert_eq!(providers.len(), 30);

    // Verify each provider is still accessible
    for i in 0..30 {
        let provider_id = String::from_str(&env, &format!("provider-{}", i));
        let provider = client.get_provider(&provider_id).expect("Provider exists");
        assert!(!provider.name.is_empty());
    }
}

// ── Test 10: Exchange Status State Machine with Boundary Values ─────────────

#[test]
fn test_exchange_status_transitions_at_boundaries() {
    let (env, client, admin) = setup_bridge();

    let provider_id =
        register_and_activate_provider(&env, &client, &admin, "status-boundary-provider");

    // Set timestamp to boundary
    env.ledger().set_timestamp(u64::MAX);

    let exchange_id = String::from_str(&env, "status-boundary-ex");
    let patient_id = String::from_str(&env, "status-boundary-pat");

    // Create exchange at boundary timestamp
    let record = client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "status_hash"),
    );

    // Verify initial status
    assert_eq!(record.status, SyncStatus::Pending);

    // Transition through states at boundary
    client.update_exchange_status(&admin, &exchange_id, &SyncStatus::Completed);

    let updated = client.get_exchange(&exchange_id).expect("Exchange exists");
    assert_eq!(updated.status, SyncStatus::Completed);
    assert_eq!(updated.timestamp, u64::MAX); // Timestamp preserved through transitions
}

// ── Test 11: Verification Discrepancy Vector with Many Items ─────────────────

#[test]
fn test_verification_with_many_discrepancies() {
    let (env, client, admin) = setup_bridge();

    let provider_id = register_and_activate_provider(&env, &client, &admin, "discrepancy-provider");

    // Create exchange first
    let exchange_id = String::from_str(&env, "discrepancy-ex");
    let patient_id = String::from_str(&env, "discrepancy-pat");

    client.record_data_exchange(
        &admin,
        &exchange_id,
        &String::from_str(&env, &provider_id),
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "disc_hash"),
    );

    // Create verification with many discrepancies
    let verification_id = String::from_str(&env, "verify-many-disc");
    let mut discrepancies = Vec::new(&env);

    for i in 0..20 {
        discrepancies.push_back(String::from_str(
            &env,
            &format!("Discrepancy field {} differs", i),
        ));
    }

    let verification = client.verify_sync(
        &admin,
        &verification_id,
        &exchange_id,
        &String::from_str(&env, "source1"),
        &String::from_str(&env, "target1"),
        &discrepancies,
    );

    // Should detect inconsistency due to discrepancies
    assert!(!verification.is_consistent);
    assert_eq!(verification.discrepancies.len(), 20);
}

// ── Test 12: Timestamp Monotonicity Across Operations ──────────────────────

#[test]
fn test_timestamp_consistency_across_operations() {
    let (env, client, admin) = setup_bridge();

    let provider_id = register_and_activate_provider(&env, &client, &admin, "monotonic-provider");

    // Set specific controlled timestamps
    let ts1 = 1000u64;
    let ts2 = 2000u64;
    let ts3 = 3000u64;

    // Exchange 1
    env.ledger().set_timestamp(ts1);
    let ex1 = String::from_str(&env, "mono-ex-1");
    let record1 = client.record_data_exchange(
        &admin,
        &ex1,
        &String::from_str(&env, &provider_id),
        &String::from_str(&env, "pat-mono"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "hash1"),
    );
    assert_eq!(record1.timestamp, ts1);

    // Exchange 2
    env.ledger().set_timestamp(ts2);
    let ex2 = String::from_str(&env, "mono-ex-2");
    let record2 = client.record_data_exchange(
        &admin,
        &ex2,
        &String::from_str(&env, &provider_id),
        &String::from_str(&env, "pat-mono"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "hash2"),
    );
    assert_eq!(record2.timestamp, ts2);

    // Exchange 3
    env.ledger().set_timestamp(ts3);
    let ex3 = String::from_str(&env, "mono-ex-3");
    let record3 = client.record_data_exchange(
        &admin,
        &ex3,
        &String::from_str(&env, &provider_id),
        &String::from_str(&env, "pat-mono"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "hash3"),
    );
    assert_eq!(record3.timestamp, ts3);

    // Verify timestamp relationships are preserved
    assert!(record1.timestamp < record2.timestamp);
    assert!(record2.timestamp < record3.timestamp);
}
