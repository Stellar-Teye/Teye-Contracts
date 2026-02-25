use crate::{
    types::{DataFormat, EmrSystem, ExchangeDirection, ProviderStatus, SyncStatus},
    EmrBridgeContract, EmrBridgeContractClient,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

fn setup_env() -> (Env, EmrBridgeContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmrBridgeContract, ());
    let client = EmrBridgeContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

// ── Initialization Tests ─────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmrBridgeContract, ());
    let client = EmrBridgeContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_initialize_twice_fails() {
    let (env, client, _admin) = setup_env();
    let another_admin = Address::generate(&env);
    client.initialize(&another_admin);
}

// ── Provider Onboarding Tests ────────────────────────────────────────────────

#[test]
fn test_register_provider() {
    let (env, client, admin) = setup_env();

    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital Epic");
    let endpoint = String::from_str(&env, "https://epic.cityhospital.org/fhir");

    let provider = client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );

    assert_eq!(provider.provider_id, provider_id);
    assert_eq!(provider.name, name);
    assert_eq!(provider.emr_system, EmrSystem::EpicFhir);
    assert_eq!(provider.data_format, DataFormat::FhirR4);
    assert_eq!(provider.status, ProviderStatus::Pending);
    assert_eq!(provider.registered_by, admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_register_duplicate_provider_fails() {
    let (env, client, admin) = setup_env();

    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital Epic");
    let endpoint = String::from_str(&env, "https://epic.cityhospital.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );

    // Registering again with same ID should fail
    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );
}

#[test]
fn test_activate_provider() {
    let (env, client, admin) = setup_env();

    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital Epic");
    let endpoint = String::from_str(&env, "https://epic.cityhospital.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );

    client.activate_provider(&admin, &provider_id);

    let provider = client.get_provider(&provider_id);
    assert_eq!(provider.status, ProviderStatus::Active);
}

#[test]
fn test_suspend_provider() {
    let (env, client, admin) = setup_env();

    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital Epic");
    let endpoint = String::from_str(&env, "https://epic.cityhospital.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );

    client.activate_provider(&admin, &provider_id);
    client.suspend_provider(&admin, &provider_id);

    let provider = client.get_provider(&provider_id);
    assert_eq!(provider.status, ProviderStatus::Suspended);
}

#[test]
fn test_list_providers() {
    let (env, client, admin) = setup_env();

    let id1 = String::from_str(&env, "epic-001");
    let id2 = String::from_str(&env, "cerner-001");
    let name1 = String::from_str(&env, "City Hospital");
    let name2 = String::from_str(&env, "County Clinic");
    let endpoint1 = String::from_str(&env, "https://epic.city.org/fhir");
    let endpoint2 = String::from_str(&env, "https://cerner.county.org/api");

    client.register_provider(
        &admin,
        &id1,
        &name1,
        &EmrSystem::EpicFhir,
        &endpoint1,
        &DataFormat::FhirR4,
    );
    client.register_provider(
        &admin,
        &id2,
        &name2,
        &EmrSystem::CernerMillennium,
        &endpoint2,
        &DataFormat::Hl7V2,
    );

    let providers = client.list_providers();
    assert_eq!(providers.len(), 2);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_non_admin_cannot_register_provider() {
    let (env, client, _admin) = setup_env();

    let non_admin = Address::generate(&env);
    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital");
    let endpoint = String::from_str(&env, "https://epic.city.org/fhir");

    client.register_provider(
        &non_admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );
}

// ── Data Exchange Tests ──────────────────────────────────────────────────────

#[test]
fn test_record_data_exchange() {
    let (env, client, admin) = setup_env();

    // Register and activate provider first
    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital");
    let endpoint = String::from_str(&env, "https://epic.city.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );
    client.activate_provider(&admin, &provider_id);

    let exchange_id = String::from_str(&env, "ex-001");
    let patient_id = String::from_str(&env, "pat-123");
    let resource_type = String::from_str(&env, "Patient");
    let record_hash = String::from_str(&env, "abc123hash");

    let record = client.record_data_exchange(
        &admin,
        &exchange_id,
        &provider_id,
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &resource_type,
        &record_hash,
    );

    assert_eq!(record.exchange_id, exchange_id);
    assert_eq!(record.provider_id, provider_id);
    assert_eq!(record.patient_id, patient_id);
    assert_eq!(record.direction, ExchangeDirection::Import);
    assert_eq!(record.status, SyncStatus::Pending);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_exchange_with_inactive_provider_fails() {
    let (env, client, admin) = setup_env();

    // Register provider but do NOT activate
    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital");
    let endpoint = String::from_str(&env, "https://epic.city.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );

    let exchange_id = String::from_str(&env, "ex-001");
    let patient_id = String::from_str(&env, "pat-123");
    let resource_type = String::from_str(&env, "Patient");
    let record_hash = String::from_str(&env, "abc123hash");

    client.record_data_exchange(
        &admin,
        &exchange_id,
        &provider_id,
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &resource_type,
        &record_hash,
    );
}

#[test]
fn test_update_exchange_status() {
    let (env, client, admin) = setup_env();

    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital");
    let endpoint = String::from_str(&env, "https://epic.city.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );
    client.activate_provider(&admin, &provider_id);

    let exchange_id = String::from_str(&env, "ex-001");
    let patient_id = String::from_str(&env, "pat-123");
    let resource_type = String::from_str(&env, "Patient");
    let record_hash = String::from_str(&env, "abc123hash");

    client.record_data_exchange(
        &admin,
        &exchange_id,
        &provider_id,
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &resource_type,
        &record_hash,
    );

    client.update_exchange_status(&admin, &exchange_id, &SyncStatus::InProgress);

    let record = client.get_exchange(&exchange_id);
    assert_eq!(record.status, SyncStatus::InProgress);
}

#[test]
fn test_get_patient_exchanges() {
    let (env, client, admin) = setup_env();

    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital");
    let endpoint = String::from_str(&env, "https://epic.city.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );
    client.activate_provider(&admin, &provider_id);

    let patient_id = String::from_str(&env, "pat-123");
    let resource_type = String::from_str(&env, "Patient");
    let hash = String::from_str(&env, "hash1");

    let ex1 = String::from_str(&env, "ex-001");
    let ex2 = String::from_str(&env, "ex-002");

    client.record_data_exchange(
        &admin,
        &ex1,
        &provider_id,
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &resource_type,
        &hash,
    );
    client.record_data_exchange(
        &admin,
        &ex2,
        &provider_id,
        &patient_id,
        &ExchangeDirection::Export,
        &DataFormat::FhirR4,
        &resource_type,
        &hash,
    );

    let exchanges = client.get_patient_exchanges(&patient_id);
    assert_eq!(exchanges.len(), 2);
}

// ── Data Mapping Tests ───────────────────────────────────────────────────────

#[test]
fn test_create_field_mapping() {
    let (env, client, admin) = setup_env();

    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital");
    let endpoint = String::from_str(&env, "https://epic.city.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );

    let mapping_id = String::from_str(&env, "map-001");
    let source_field = String::from_str(&env, "patient.name.given");
    let target_field = String::from_str(&env, "first_name");
    let transform_rule = String::from_str(&env, "direct_copy");

    let mapping = client.create_field_mapping(
        &admin,
        &mapping_id,
        &provider_id,
        &source_field,
        &target_field,
        &transform_rule,
    );

    assert_eq!(mapping.mapping_id, mapping_id);
    assert_eq!(mapping.provider_id, provider_id);
    assert_eq!(mapping.source_field, source_field);
    assert_eq!(mapping.target_field, target_field);
    assert_eq!(mapping.transform_rule, transform_rule);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_create_mapping_with_empty_fields_fails() {
    let (env, client, admin) = setup_env();

    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital");
    let endpoint = String::from_str(&env, "https://epic.city.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );

    let mapping_id = String::from_str(&env, "map-001");
    let empty = String::from_str(&env, "");
    let target_field = String::from_str(&env, "first_name");
    let transform_rule = String::from_str(&env, "direct_copy");

    client.create_field_mapping(
        &admin,
        &mapping_id,
        &provider_id,
        &empty,
        &target_field,
        &transform_rule,
    );
}

#[test]
fn test_get_provider_mappings() {
    let (env, client, admin) = setup_env();

    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital");
    let endpoint = String::from_str(&env, "https://epic.city.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );

    let map1 = String::from_str(&env, "map-001");
    let map2 = String::from_str(&env, "map-002");
    let src1 = String::from_str(&env, "patient.name.given");
    let tgt1 = String::from_str(&env, "first_name");
    let src2 = String::from_str(&env, "patient.name.family");
    let tgt2 = String::from_str(&env, "last_name");
    let rule = String::from_str(&env, "direct_copy");

    client.create_field_mapping(&admin, &map1, &provider_id, &src1, &tgt1, &rule);
    client.create_field_mapping(&admin, &map2, &provider_id, &src2, &tgt2, &rule);

    let mappings = client.get_provider_mappings(&provider_id);
    assert_eq!(mappings.len(), 2);
}

// ── Sync Verification Tests ──────────────────────────────────────────────────

#[test]
fn test_verify_sync_consistent() {
    let (env, client, admin) = setup_env();

    // Setup provider and exchange
    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital");
    let endpoint = String::from_str(&env, "https://epic.city.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );
    client.activate_provider(&admin, &provider_id);

    let exchange_id = String::from_str(&env, "ex-001");
    let patient_id = String::from_str(&env, "pat-123");
    let resource_type = String::from_str(&env, "Patient");
    let record_hash = String::from_str(&env, "abc123hash");

    client.record_data_exchange(
        &admin,
        &exchange_id,
        &provider_id,
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &resource_type,
        &record_hash,
    );

    let verification_id = String::from_str(&env, "ver-001");
    let source_hash = String::from_str(&env, "hash_abc");
    let target_hash = String::from_str(&env, "hash_abc");
    let discrepancies: Vec<String> = Vec::new(&env);

    let verification = client.verify_sync(
        &admin,
        &verification_id,
        &exchange_id,
        &source_hash,
        &target_hash,
        &discrepancies,
    );

    assert!(verification.is_consistent);
    assert_eq!(verification.verification_id, verification_id);

    // Exchange status should be updated to Completed
    let exchange = client.get_exchange(&exchange_id);
    assert_eq!(exchange.status, SyncStatus::Completed);
}

#[test]
fn test_verify_sync_inconsistent() {
    let (env, client, admin) = setup_env();

    let provider_id = String::from_str(&env, "epic-001");
    let name = String::from_str(&env, "City Hospital");
    let endpoint = String::from_str(&env, "https://epic.city.org/fhir");

    client.register_provider(
        &admin,
        &provider_id,
        &name,
        &EmrSystem::EpicFhir,
        &endpoint,
        &DataFormat::FhirR4,
    );
    client.activate_provider(&admin, &provider_id);

    let exchange_id = String::from_str(&env, "ex-001");
    let patient_id = String::from_str(&env, "pat-123");
    let resource_type = String::from_str(&env, "Patient");
    let record_hash = String::from_str(&env, "abc123hash");

    client.record_data_exchange(
        &admin,
        &exchange_id,
        &provider_id,
        &patient_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &resource_type,
        &record_hash,
    );

    let verification_id = String::from_str(&env, "ver-001");
    let source_hash = String::from_str(&env, "hash_abc");
    let target_hash = String::from_str(&env, "hash_xyz");
    let mut discrepancies: Vec<String> = Vec::new(&env);
    discrepancies.push_back(String::from_str(&env, "name field mismatch"));

    let verification = client.verify_sync(
        &admin,
        &verification_id,
        &exchange_id,
        &source_hash,
        &target_hash,
        &discrepancies,
    );

    assert!(!verification.is_consistent);

    // Exchange status should be PartialSuccess
    let exchange = client.get_exchange(&exchange_id);
    assert_eq!(exchange.status, SyncStatus::PartialSuccess);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_verify_sync_nonexistent_exchange_fails() {
    let (env, client, admin) = setup_env();

    let verification_id = String::from_str(&env, "ver-001");
    let exchange_id = String::from_str(&env, "nonexistent");
    let source_hash = String::from_str(&env, "hash_abc");
    let target_hash = String::from_str(&env, "hash_abc");
    let discrepancies: Vec<String> = Vec::new(&env);

    client.verify_sync(
        &admin,
        &verification_id,
        &exchange_id,
        &source_hash,
        &target_hash,
        &discrepancies,
    );
}
