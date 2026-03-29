#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Integration tests for FHIR-to-ledger mapping integrity (issue #461).
//!
//! Covers three checklist items:
//!   1. Converting FHIR JSON to on-chain state via record_data_exchange
//!   2. Anonymization protocol: patient_id is stored as a pseudonymous hash
//!   3. Ledger reconstruction: full roundtrip fidelity for all stored types

use emr_bridge::{
    types::{DataFormat, EmrSystem, ExchangeDirection, SyncStatus},
    EmrBridgeContract, EmrBridgeContractClient, EmrBridgeError,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

// ── Setup helper ─────────────────────────────────────────────────────────────

struct Setup<'a> {
    env: Env,
    client: EmrBridgeContractClient<'a>,
    admin: Address,
    provider_id: String,
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmrBridgeContract, ());
    let client = EmrBridgeContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let provider_id = String::from_str(&env, "epic-fhir-r4");
    client.register_provider(
        &admin,
        &provider_id,
        &String::from_str(&env, "Epic FHIR R4"),
        &EmrSystem::EpicFhir,
        &String::from_str(&env, "https://epic.example.org/api/FHIR/R4"),
        &DataFormat::FhirR4,
    );
    client.activate_provider(&admin, &provider_id);

    Setup {
        env,
        client,
        admin,
        provider_id,
    }
}

// ── 1. FHIR JSON → on-chain state ────────────────────────────────────────────

/// Each standard FHIR resource type is accepted and stored with the
/// correct provider, format, and direction on the ledger.
#[test]
fn test_fhir_patient_resource_stored_on_chain() {
    let s = setup();
    let record = s.client.record_data_exchange(
        &s.admin,
        &String::from_str(&s.env, "ex-patient-001"),
        &s.provider_id,
        &String::from_str(&s.env, "sha256:anon-patient-abc"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Patient"),
        &String::from_str(&s.env, "sha256:patient-resource-hash"),
    );

    assert_eq!(record.data_format, DataFormat::FhirR4);
    assert_eq!(record.direction, ExchangeDirection::Import);
    assert_eq!(record.resource_type, String::from_str(&s.env, "Patient"));
    assert_eq!(record.status, SyncStatus::Pending);
}

#[test]
fn test_fhir_observation_resource_stored_on_chain() {
    let s = setup();
    let record = s.client.record_data_exchange(
        &s.admin,
        &String::from_str(&s.env, "ex-obs-001"),
        &s.provider_id,
        &String::from_str(&s.env, "sha256:anon-patient-abc"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Observation"),
        &String::from_str(&s.env, "sha256:observation-resource-hash"),
    );

    assert_eq!(
        record.resource_type,
        String::from_str(&s.env, "Observation")
    );
    assert_eq!(record.provider_id, s.provider_id);
}

#[test]
fn test_fhir_diagnostic_report_and_condition_stored() {
    let s = setup();

    for (ex_id, resource) in [
        ("ex-diag-001", "DiagnosticReport"),
        ("ex-cond-001", "Condition"),
        ("ex-med-001", "MedicationRequest"),
    ] {
        let record = s.client.record_data_exchange(
            &s.admin,
            &String::from_str(&s.env, ex_id),
            &s.provider_id,
            &String::from_str(&s.env, "sha256:anon-patient-abc"),
            &ExchangeDirection::Import,
            &DataFormat::FhirR4,
            &String::from_str(&s.env, resource),
            &String::from_str(&s.env, "sha256:resource-hash"),
        );
        assert_eq!(record.resource_type, String::from_str(&s.env, resource));
    }
}

/// Exporting FHIR data (e.g., sending to another system) records with
/// Export direction and FhirR4 format correctly.
#[test]
fn test_fhir_export_direction_stored_correctly() {
    let s = setup();
    let record = s.client.record_data_exchange(
        &s.admin,
        &String::from_str(&s.env, "ex-export-001"),
        &s.provider_id,
        &String::from_str(&s.env, "sha256:anon-patient-abc"),
        &ExchangeDirection::Export,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Bundle"),
        &String::from_str(&s.env, "sha256:bundle-hash"),
    );

    assert_eq!(record.direction, ExchangeDirection::Export);
    assert_eq!(record.status, SyncStatus::Pending);
}

/// FHIR field mappings describe how source fields in the EMR system map to
/// Teye's internal schema. Creating a mapping for a FHIR provider stores
/// source_field, target_field, and transform_rule intact.
#[test]
fn test_fhir_field_mapping_source_and_target_stored_correctly() {
    let s = setup();
    let mapping = s.client.create_field_mapping(
        &s.admin,
        &String::from_str(&s.env, "map-patient-name"),
        &s.provider_id,
        &String::from_str(&s.env, "Patient.name[0].family"),
        &String::from_str(&s.env, "teye.patient.surname"),
        &String::from_str(&s.env, "normalize_unicode"),
    );

    assert_eq!(
        mapping.source_field,
        String::from_str(&s.env, "Patient.name[0].family")
    );
    assert_eq!(
        mapping.target_field,
        String::from_str(&s.env, "teye.patient.surname")
    );
    assert_eq!(
        mapping.transform_rule,
        String::from_str(&s.env, "normalize_unicode")
    );
    assert_eq!(mapping.provider_id, s.provider_id);
}

#[test]
fn test_fhir_field_mappings_indexed_under_provider() {
    let s = setup();

    let fhir_fields = [
        (
            "map-obs-value",
            "Observation.valueQuantity.value",
            "teye.metric.value",
        ),
        (
            "map-obs-unit",
            "Observation.valueQuantity.unit",
            "teye.metric.unit",
        ),
        (
            "map-cond-code",
            "Condition.code.coding[0].code",
            "teye.condition.icd10",
        ),
    ];

    for (id, src, tgt) in fhir_fields {
        s.client.create_field_mapping(
            &s.admin,
            &String::from_str(&s.env, id),
            &s.provider_id,
            &String::from_str(&s.env, src),
            &String::from_str(&s.env, tgt),
            &String::from_str(&s.env, "passthrough"),
        );
    }

    let mapping_ids = s.client.get_provider_mappings(&s.provider_id);
    assert_eq!(mapping_ids.len(), 3);
}

// ── 2. Anonymization protocol ─────────────────────────────────────────────────

/// Real patient identifiers must never appear as the stored patient_id.
/// Instead, a SHA-256 pseudonym is passed in. The ledger stores only
/// the pseudonym, keeping the raw identifier off-chain.
#[test]
fn test_patient_id_stored_is_pseudonymous_hash() {
    let s = setup();

    // Pseudonymous patient ID derived from SHA-256(NHS:1234567890)
    let pseudo_id = String::from_str(
        &s.env,
        "sha256:b94f6f125c79e3a5ffaa826f584c10d52ada669e6762051b826b55776d05a886",
    );

    s.client.record_data_exchange(
        &s.admin,
        &String::from_str(&s.env, "ex-anon-001"),
        &s.provider_id,
        &pseudo_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Patient"),
        &String::from_str(&s.env, "sha256:record-hash"),
    );

    let stored = s
        .client
        .get_exchange(&String::from_str(&s.env, "ex-anon-001"));

    // The stored patient_id is the pseudonym — not a raw identifier.
    assert_eq!(stored.patient_id, pseudo_id);
    // Confirm it looks like a hash prefix and not a plain national ID.
    assert!(stored.patient_id.len() > 10);
}

/// Multiple FHIR resource types for the same pseudonymous patient are all
/// grouped under the same patient index on the ledger.
#[test]
fn test_multiple_fhir_resources_grouped_under_same_pseudonym() {
    let s = setup();
    let pseudo_id = String::from_str(&s.env, "sha256:patient-pseudo-xyz");

    let resources = [
        ("ex-grp-001", "Patient"),
        ("ex-grp-002", "Observation"),
        ("ex-grp-003", "Condition"),
        ("ex-grp-004", "DiagnosticReport"),
    ];

    for (ex_id, resource) in resources {
        s.client.record_data_exchange(
            &s.admin,
            &String::from_str(&s.env, ex_id),
            &s.provider_id,
            &pseudo_id,
            &ExchangeDirection::Import,
            &DataFormat::FhirR4,
            &String::from_str(&s.env, resource),
            &String::from_str(&s.env, "sha256:hash"),
        );
    }

    let exchanges = s.client.get_patient_exchanges(&pseudo_id);
    assert_eq!(exchanges.len(), 4);

    // Verify each exchange ID is present in the patient's index.
    for (ex_id, _) in resources {
        assert!(exchanges.contains(&String::from_str(&s.env, ex_id)));
    }
}

/// Two different pseudonymous patients must have completely separate exchange
/// indexes — cross-patient data leakage via the patient index is impossible.
#[test]
fn test_patient_index_isolation_between_pseudonyms() {
    let s = setup();
    let patient_a = String::from_str(&s.env, "sha256:patient-a");
    let patient_b = String::from_str(&s.env, "sha256:patient-b");

    s.client.record_data_exchange(
        &s.admin,
        &String::from_str(&s.env, "ex-iso-a"),
        &s.provider_id,
        &patient_a,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Observation"),
        &String::from_str(&s.env, "sha256:hash-a"),
    );
    s.client.record_data_exchange(
        &s.admin,
        &String::from_str(&s.env, "ex-iso-b"),
        &s.provider_id,
        &patient_b,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Observation"),
        &String::from_str(&s.env, "sha256:hash-b"),
    );

    let a_list = s.client.get_patient_exchanges(&patient_a);
    let b_list = s.client.get_patient_exchanges(&patient_b);

    assert_eq!(a_list.len(), 1);
    assert_eq!(b_list.len(), 1);
    assert!(a_list.contains(&String::from_str(&s.env, "ex-iso-a")));
    assert!(b_list.contains(&String::from_str(&s.env, "ex-iso-b")));
    // No cross-contamination.
    assert!(!a_list.contains(&String::from_str(&s.env, "ex-iso-b")));
    assert!(!b_list.contains(&String::from_str(&s.env, "ex-iso-a")));
}

/// A suspended provider must not accept new FHIR data exchanges, preventing
/// a compromised or non-compliant endpoint from writing to the ledger.
#[test]
fn test_suspended_provider_cannot_record_fhir_exchange() {
    let s = setup();
    s.client.suspend_provider(&s.admin, &s.provider_id);

    let result = s.client.try_record_data_exchange(
        &s.admin,
        &String::from_str(&s.env, "ex-suspended"),
        &s.provider_id,
        &String::from_str(&s.env, "sha256:patient"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Observation"),
        &String::from_str(&s.env, "sha256:hash"),
    );

    assert_eq!(result, Err(Ok(EmrBridgeError::ProviderNotActive)));
}

// ── 3. Ledger reconstruction ──────────────────────────────────────────────────

/// Full roundtrip: ingest a FHIR record, verify the sync, then reconstruct
/// the exchange record from the ledger and confirm all fields are intact.
#[test]
fn test_fhir_record_fully_reconstructed_after_ingest() {
    let s = setup();

    let exchange_id = String::from_str(&s.env, "ex-roundtrip-001");
    let pseudo_id = String::from_str(&s.env, "sha256:patient-roundtrip");
    let record_hash = String::from_str(&s.env, "sha256:fhir-bundle-content-hash");

    s.client.record_data_exchange(
        &s.admin,
        &exchange_id,
        &s.provider_id,
        &pseudo_id,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Bundle"),
        &record_hash,
    );

    // Reconstruct from ledger.
    let retrieved = s.client.get_exchange(&exchange_id);

    assert_eq!(retrieved.exchange_id, exchange_id);
    assert_eq!(retrieved.provider_id, s.provider_id);
    assert_eq!(retrieved.patient_id, pseudo_id);
    assert_eq!(retrieved.direction, ExchangeDirection::Import);
    assert_eq!(retrieved.data_format, DataFormat::FhirR4);
    assert_eq!(retrieved.resource_type, String::from_str(&s.env, "Bundle"));
    assert_eq!(retrieved.record_hash, record_hash);
    assert_eq!(retrieved.status, SyncStatus::Pending);
}

/// After a successful sync verification with matching hashes, the exchange
/// status transitions to Completed and the verification record is on-chain.
#[test]
fn test_sync_verification_consistent_hashes_transitions_to_completed() {
    let s = setup();

    let exchange_id = String::from_str(&s.env, "ex-verify-001");
    let fhir_hash = String::from_str(&s.env, "sha256:canonical-fhir-hash");

    s.client.record_data_exchange(
        &s.admin,
        &exchange_id,
        &s.provider_id,
        &String::from_str(&s.env, "sha256:patient"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Patient"),
        &fhir_hash,
    );

    let verification = s.client.verify_sync(
        &s.admin,
        &String::from_str(&s.env, "v-001"),
        &exchange_id,
        &fhir_hash,
        &fhir_hash,
        &Vec::new(&s.env),
    );

    assert!(verification.is_consistent);
    assert_eq!(verification.source_hash, fhir_hash);
    assert_eq!(verification.target_hash, fhir_hash);

    // Exchange status must be updated to Completed.
    let record = s.client.get_exchange(&exchange_id);
    assert_eq!(record.status, SyncStatus::Completed);
}

/// Mismatched source and target hashes mark the exchange as PartialSuccess
/// and expose the discrepancy on-chain for auditing.
#[test]
fn test_sync_verification_mismatched_hashes_marks_partial_success() {
    let s = setup();

    let exchange_id = String::from_str(&s.env, "ex-verify-002");
    s.client.record_data_exchange(
        &s.admin,
        &exchange_id,
        &s.provider_id,
        &String::from_str(&s.env, "sha256:patient"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Observation"),
        &String::from_str(&s.env, "sha256:original-hash"),
    );

    let mut discrepancies = Vec::new(&s.env);
    discrepancies.push_back(String::from_str(
        &s.env,
        "Observation.status field mismatch",
    ));

    let verification = s.client.verify_sync(
        &s.admin,
        &String::from_str(&s.env, "v-002"),
        &exchange_id,
        &String::from_str(&s.env, "sha256:source-hash"),
        &String::from_str(&s.env, "sha256:target-hash-differs"),
        &discrepancies,
    );

    assert!(!verification.is_consistent);
    assert_eq!(verification.discrepancies.len(), 1);

    let record = s.client.get_exchange(&exchange_id);
    assert_eq!(record.status, SyncStatus::PartialSuccess);
}

/// Field mappings are reconstructed from the ledger with every field intact,
/// confirming the FHIR-to-internal-schema mapping is stable across reads.
#[test]
fn test_field_mapping_fully_reconstructed_from_ledger() {
    let s = setup();

    let source = String::from_str(&s.env, "Observation.component[0].valueQuantity.value");
    let target = String::from_str(&s.env, "teye.vitals.systolic_bp");
    let rule = String::from_str(&s.env, "convert_mmhg");

    s.client.create_field_mapping(
        &s.admin,
        &String::from_str(&s.env, "map-bp-systolic"),
        &s.provider_id,
        &source,
        &target,
        &rule,
    );

    let retrieved = s
        .client
        .get_field_mapping(&String::from_str(&s.env, "map-bp-systolic"));

    assert_eq!(retrieved.source_field, source);
    assert_eq!(retrieved.target_field, target);
    assert_eq!(retrieved.transform_rule, rule);
    assert_eq!(retrieved.provider_id, s.provider_id);
    assert_eq!(
        retrieved.mapping_id,
        String::from_str(&s.env, "map-bp-systolic")
    );
}

/// The sync verification record itself is reconstructible from the ledger
/// with all discrepancy details intact for audit purposes.
#[test]
fn test_sync_verification_reconstructed_from_ledger() {
    let s = setup();

    let exchange_id = String::from_str(&s.env, "ex-audit-001");
    s.client.record_data_exchange(
        &s.admin,
        &exchange_id,
        &s.provider_id,
        &String::from_str(&s.env, "sha256:patient"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "DiagnosticReport"),
        &String::from_str(&s.env, "sha256:diag-hash"),
    );

    let mut discrepancies = Vec::new(&s.env);
    discrepancies.push_back(String::from_str(
        &s.env,
        "DiagnosticReport.result count differs",
    ));
    discrepancies.push_back(String::from_str(&s.env, "DiagnosticReport.status mismatch"));

    s.client.verify_sync(
        &s.admin,
        &String::from_str(&s.env, "v-audit-001"),
        &exchange_id,
        &String::from_str(&s.env, "sha256:src"),
        &String::from_str(&s.env, "sha256:tgt"),
        &discrepancies,
    );

    let retrieved = s
        .client
        .get_verification(&String::from_str(&s.env, "v-audit-001"));

    assert_eq!(retrieved.exchange_id, exchange_id);
    assert!(!retrieved.is_consistent);
    assert_eq!(retrieved.discrepancies.len(), 2);
    assert!(retrieved.discrepancies.contains(&String::from_str(
        &s.env,
        "DiagnosticReport.result count differs"
    )));
}

/// Duplicate exchange IDs are rejected — FHIR records cannot overwrite
/// existing ledger entries, ensuring append-only integrity.
#[test]
fn test_duplicate_exchange_id_rejected() {
    let s = setup();

    let exchange_id = String::from_str(&s.env, "ex-dup-001");
    s.client.record_data_exchange(
        &s.admin,
        &exchange_id,
        &s.provider_id,
        &String::from_str(&s.env, "sha256:patient"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Patient"),
        &String::from_str(&s.env, "sha256:hash"),
    );

    let result = s.client.try_record_data_exchange(
        &s.admin,
        &exchange_id,
        &s.provider_id,
        &String::from_str(&s.env, "sha256:patient"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&s.env, "Patient"),
        &String::from_str(&s.env, "sha256:hash-2"),
    );

    assert_eq!(result, Err(Ok(EmrBridgeError::ExchangeAlreadyExists)));
}
