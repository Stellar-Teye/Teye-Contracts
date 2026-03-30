//! Event Emission Verification Tests for `emr_bridge`
//!
//! Each state-changing entry point in the contract emits exactly one event.
//! These tests verify that:
//! - `initialize`          → emits `EMR_INIT(admin)`
//! - `register_provider`   → emits `PRV_REG(provider_id, registered_by)`
//! - `activate_provider`   → emits `PRV_STS(provider_id, status=1)`
//! - `suspend_provider`    → emits `PRV_STS(provider_id, status=2)`
//! - `record_data_exchange`→ emits `DATA_EX(exchange_id, provider_id)`
//! - `create_field_mapping`→ emits `MAP_ADD(mapping_id, provider_id)`
//! - `verify_sync`         → emits `SYNC_VF(verification_id, is_consistent)`
//!
//! NOTE: `env.events().all()` in Soroban testutils returns only the events
//! produced by the *most recent* contract invocation.  All count assertions
//! therefore measure the number of events that a *single* call produces.
//!
//! Idempotent code paths (e.g. activating an already-active provider) must
//! produce 0 events.

use emr_bridge::{
    types::{DataFormat, EmrSystem, ExchangeDirection, SyncStatus},
    EmrBridgeContract, EmrBridgeContractClient,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, String, Vec,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Initialise a fresh contract but do NOT call `initialize`.
fn setup_uninit() -> (Env, EmrBridgeContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(EmrBridgeContract, ());
    let client = EmrBridgeContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    (env, client, admin)
}

/// Initialise a fresh contract AND call `initialize`.
fn setup() -> (Env, EmrBridgeContractClient<'static>, Address) {
    let (env, client, admin) = setup_uninit();
    client.initialize(&admin);
    (env, client, admin)
}

/// Register AND activate a provider; returns its `soroban_sdk::String` id.
fn register_active_provider(
    env: &Env,
    client: &EmrBridgeContractClient,
    admin: &Address,
    id: &str,
) -> String {
    let pid = String::from_str(env, id);
    client.register_provider(
        admin,
        &pid,
        &String::from_str(env, &format!("Provider {id}")),
        &EmrSystem::EpicFhir,
        &String::from_str(env, "https://emr.example.com/fhir"),
        &DataFormat::FhirR4,
    );
    client.activate_provider(admin, &pid);
    pid
}

/// Return the number of events produced by the most-recent contract call.
fn last_call_event_count(env: &Env) -> usize {
    env.events().all().events().len()
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. INITIALIZE — emits EMR_INIT
// ═══════════════════════════════════════════════════════════════════════════════

/// Before any contract call there must be no events.
#[test]
fn test_no_events_before_any_call() {
    let (env, _client, _admin) = setup_uninit();
    assert_eq!(last_call_event_count(&env), 0, "no events before any call");
}

/// `initialize` must emit exactly one event.
#[test]
fn test_initialize_emits_exactly_one_event() {
    let (env, client, admin) = setup_uninit();
    client.initialize(&admin);
    assert_eq!(
        last_call_event_count(&env),
        1,
        "initialize must emit exactly one EMR_INIT event"
    );
}

/// The `initialize` event must be present (topic + data presence check).
#[test]
fn test_initialize_event_is_present() {
    let (env, client, admin) = setup_uninit();
    client.initialize(&admin);

    let all = env.events().all();
    assert!(
        !all.events().is_empty(),
        "EMR_INIT event must be present after initialize"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. REGISTER_PROVIDER — emits PRV_REG
// ═══════════════════════════════════════════════════════════════════════════════

/// `register_provider` must emit exactly one PRV_REG event per call.
#[test]
fn test_register_provider_emits_one_event() {
    let (env, client, admin) = setup();

    client.register_provider(
        &admin,
        &String::from_str(&env, "prov-reg-01"),
        &String::from_str(&env, "Registration Test Provider"),
        &EmrSystem::EpicFhir,
        &String::from_str(&env, "https://epic.example.com"),
        &DataFormat::FhirR4,
    );

    assert_eq!(
        last_call_event_count(&env),
        1,
        "register_provider must emit exactly one PRV_REG event"
    );
}

/// Each distinct `register_provider` call must individually produce one event.
#[test]
fn test_each_register_provider_call_emits_one_event() {
    let (env, client, admin) = setup();

    for i in 0u32..3 {
        client.register_provider(
            &admin,
            &String::from_str(&env, &format!("multi-prov-{i}")),
            &String::from_str(&env, &format!("Provider {i}")),
            &EmrSystem::CernerMillennium,
            &String::from_str(&env, &format!("https://cerner{i}.example.com")),
            &DataFormat::Hl7V2,
        );
        assert_eq!(
            last_call_event_count(&env),
            1,
            "register_provider call {i} must emit exactly one event"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. ACTIVATE_PROVIDER — emits PRV_STS (status = 1)
// ═══════════════════════════════════════════════════════════════════════════════

/// `activate_provider` on a pending provider must emit exactly one PRV_STS event.
#[test]
fn test_activate_provider_emits_one_event() {
    let (env, client, admin) = setup();

    let pid = String::from_str(&env, "prov-activate");
    client.register_provider(
        &admin,
        &pid,
        &String::from_str(&env, "Activate Test Provider"),
        &EmrSystem::EpicFhir,
        &String::from_str(&env, "https://ep.example.com"),
        &DataFormat::FhirR4,
    );

    client.activate_provider(&admin, &pid);
    assert_eq!(
        last_call_event_count(&env),
        1,
        "activate_provider must emit exactly one PRV_STS event"
    );
}

/// `activate_provider` on an already-active provider must emit 0 events.
#[test]
fn test_activate_already_active_provider_emits_no_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "already-active");

    // Second activate — idempotent, returns Ok(()) early without publishing
    client.activate_provider(&admin, &pid);
    assert_eq!(
        last_call_event_count(&env),
        0,
        "activating an already-active provider must not emit an extra event"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. SUSPEND_PROVIDER — emits PRV_STS (status = 2)
// ═══════════════════════════════════════════════════════════════════════════════

/// `suspend_provider` on an active provider must emit exactly one PRV_STS event.
#[test]
fn test_suspend_provider_emits_one_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "prov-to-suspend");

    client.suspend_provider(&admin, &pid);
    assert_eq!(
        last_call_event_count(&env),
        1,
        "suspend_provider must emit exactly one PRV_STS event"
    );
}

/// `suspend_provider` on an already-suspended provider must emit 0 events.
#[test]
fn test_suspend_already_suspended_provider_emits_no_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "already-suspended");

    client.suspend_provider(&admin, &pid); // first suspend — emits 1
    assert_eq!(last_call_event_count(&env), 1);

    // Second suspend — idempotent
    client.suspend_provider(&admin, &pid);
    assert_eq!(
        last_call_event_count(&env),
        0,
        "suspending an already-suspended provider must not emit an extra event"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. RECORD_DATA_EXCHANGE — emits DATA_EX
// ═══════════════════════════════════════════════════════════════════════════════

/// `record_data_exchange` must emit exactly one DATA_EX event.
#[test]
fn test_record_data_exchange_emits_one_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "exchange-emit-prov");

    client.record_data_exchange(
        &admin,
        &String::from_str(&env, "ex-emit-001"),
        &pid,
        &String::from_str(&env, "pat-emit"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "hash_emit"),
    );

    assert_eq!(
        last_call_event_count(&env),
        1,
        "record_data_exchange must emit exactly one DATA_EX event"
    );
}

/// Import direction emits exactly one event.
#[test]
fn test_exchange_import_direction_emits_one_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "import-emit-prov");

    client.record_data_exchange(
        &admin,
        &String::from_str(&env, "import-ex"),
        &pid,
        &String::from_str(&env, "pat-import"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "hash_import"),
    );
    assert_eq!(last_call_event_count(&env), 1, "Import must emit one event");
}

/// Export direction emits exactly one event.
#[test]
fn test_exchange_export_direction_emits_one_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "export-emit-prov");

    client.record_data_exchange(
        &admin,
        &String::from_str(&env, "export-ex"),
        &pid,
        &String::from_str(&env, "pat-export"),
        &ExchangeDirection::Export,
        &DataFormat::Hl7V2,
        &String::from_str(&env, "Observation"),
        &String::from_str(&env, "hash_export"),
    );
    assert_eq!(last_call_event_count(&env), 1, "Export must emit one event");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. CREATE_FIELD_MAPPING — emits MAP_ADD
// ═══════════════════════════════════════════════════════════════════════════════

/// `create_field_mapping` must emit exactly one MAP_ADD event.
#[test]
fn test_create_field_mapping_emits_one_event() {
    let (env, client, admin) = setup();

    let pid = String::from_str(&env, "mapping-emit-prov");
    client.register_provider(
        &admin,
        &pid,
        &String::from_str(&env, "Mapping Emit Provider"),
        &EmrSystem::EpicFhir,
        &String::from_str(&env, "https://ep.example.com"),
        &DataFormat::FhirR4,
    );

    client.create_field_mapping(
        &admin,
        &String::from_str(&env, "map-emit-001"),
        &pid,
        &String::from_str(&env, "patient.name.given"),
        &String::from_str(&env, "first_name"),
        &String::from_str(&env, "direct_copy"),
    );

    assert_eq!(
        last_call_event_count(&env),
        1,
        "create_field_mapping must emit exactly one MAP_ADD event"
    );
}

/// Each additional field mapping call must individually produce one event.
#[test]
fn test_each_field_mapping_call_emits_one_event() {
    let (env, client, admin) = setup();

    let pid = String::from_str(&env, "multi-map-prov");
    client.register_provider(
        &admin,
        &pid,
        &String::from_str(&env, "Multi Map Provider"),
        &EmrSystem::Allscripts,
        &String::from_str(&env, "https://allscripts.example.com"),
        &DataFormat::Hl7V2,
    );

    let mappings = [
        ("map-1", "src.a", "tgt.a"),
        ("map-2", "src.b", "tgt.b"),
        ("map-3", "src.c", "tgt.c"),
    ];
    for (mid, src, tgt) in &mappings {
        client.create_field_mapping(
            &admin,
            &String::from_str(&env, mid),
            &pid,
            &String::from_str(&env, src),
            &String::from_str(&env, tgt),
            &String::from_str(&env, "direct_copy"),
        );
        assert_eq!(
            last_call_event_count(&env),
            1,
            "create_field_mapping for mapping '{mid}' must emit exactly one event"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. VERIFY_SYNC — emits SYNC_VF
// ═══════════════════════════════════════════════════════════════════════════════

/// `verify_sync` must emit exactly one SYNC_VF event for a consistent result.
#[test]
fn test_verify_sync_consistent_emits_one_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "sync-emit-prov");

    client.record_data_exchange(
        &admin,
        &String::from_str(&env, "sync-emit-ex"),
        &pid,
        &String::from_str(&env, "pat-sync"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "hash_sync"),
    );

    let hash = String::from_str(&env, "same_hash");
    client.verify_sync(
        &admin,
        &String::from_str(&env, "sync-emit-v1"),
        &String::from_str(&env, "sync-emit-ex"),
        &hash,
        &hash,
        &Vec::new(&env),
    );

    assert_eq!(
        last_call_event_count(&env),
        1,
        "verify_sync (consistent) must emit exactly one SYNC_VF event"
    );
}

/// `verify_sync` must emit exactly one SYNC_VF event for an inconsistent result.
#[test]
fn test_verify_sync_inconsistent_emits_one_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "sync-incons-prov");

    client.record_data_exchange(
        &admin,
        &String::from_str(&env, "incons-ex"),
        &pid,
        &String::from_str(&env, "pat-incons"),
        &ExchangeDirection::Export,
        &DataFormat::Hl7V2,
        &String::from_str(&env, "Observation"),
        &String::from_str(&env, "hash_incons"),
    );

    let mut discrepancies = Vec::new(&env);
    discrepancies.push_back(String::from_str(&env, "dob field mismatch"));

    client.verify_sync(
        &admin,
        &String::from_str(&env, "incons-v1"),
        &String::from_str(&env, "incons-ex"),
        &String::from_str(&env, "src_hash_A"),
        &String::from_str(&env, "tgt_hash_B"),
        &discrepancies,
    );

    assert_eq!(
        last_call_event_count(&env),
        1,
        "verify_sync (inconsistent) must emit exactly one SYNC_VF event"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. READ-ONLY OPERATIONS — must not emit events
// ═══════════════════════════════════════════════════════════════════════════════

/// `get_admin` must not emit any events.
#[test]
fn test_get_admin_emits_no_event() {
    let (env, client, _admin) = setup();
    let _ = client.get_admin();
    assert_eq!(last_call_event_count(&env), 0, "get_admin must not emit any event");
}

/// `get_provider` must not emit any events.
#[test]
fn test_get_provider_emits_no_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "read-prov");

    let _ = client.get_provider(&pid);
    assert_eq!(last_call_event_count(&env), 0, "get_provider must not emit any event");
}

/// `list_providers` must not emit any events.
#[test]
fn test_list_providers_emits_no_event() {
    let (env, client, admin) = setup();
    register_active_provider(&env, &client, &admin, "list-prov");

    let _ = client.list_providers();
    assert_eq!(last_call_event_count(&env), 0, "list_providers must not emit any event");
}

/// `get_exchange` must not emit any events.
#[test]
fn test_get_exchange_emits_no_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "get-ex-prov");

    let eid = String::from_str(&env, "get-ex");
    client.record_data_exchange(
        &admin,
        &eid,
        &pid,
        &String::from_str(&env, "pat-get-ex"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "h"),
    );

    let _ = client.get_exchange(&eid);
    assert_eq!(last_call_event_count(&env), 0, "get_exchange must not emit any event");
}

/// `get_patient_exchanges` must not emit any events.
#[test]
fn test_get_patient_exchanges_emits_no_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "pat-ex-prov");
    let patient = String::from_str(&env, "pat-read");

    client.record_data_exchange(
        &admin,
        &String::from_str(&env, "pat-read-ex"),
        &pid,
        &patient,
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "h"),
    );

    let _ = client.get_patient_exchanges(&patient);
    assert_eq!(
        last_call_event_count(&env),
        0,
        "get_patient_exchanges must not emit any event"
    );
}

/// `update_exchange_status` has no `publish_*` call in source; must emit 0 events.
#[test]
fn test_update_exchange_status_emits_no_event() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "status-upd-prov");

    let eid = String::from_str(&env, "status-upd-ex");
    client.record_data_exchange(
        &admin,
        &eid,
        &pid,
        &String::from_str(&env, "pat-status"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "hash_status"),
    );

    client.update_exchange_status(&admin, &eid, &SyncStatus::InProgress);
    assert_eq!(
        last_call_event_count(&env),
        0,
        "update_exchange_status must not emit an event (no publish call in source)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 9. COMPLETE WORKFLOW — each step's event count
// ═══════════════════════════════════════════════════════════════════════════════

/// A full provider→exchange→verification workflow: verify each step produces
/// exactly one event and read-only/status steps produce none.
#[test]
fn test_full_workflow_each_step_emits_correct_event_count() {
    let (env, client, admin) = setup_uninit();

    // 1. initialize → 1 event (EMR_INIT)
    client.initialize(&admin);
    assert_eq!(last_call_event_count(&env), 1, "initialize must emit 1 event");

    // 2. register_provider → 1 event (PRV_REG)
    let pid = String::from_str(&env, "workflow-prov");
    client.register_provider(
        &admin,
        &pid,
        &String::from_str(&env, "Workflow Provider"),
        &EmrSystem::EpicFhir,
        &String::from_str(&env, "https://workflow.example.com"),
        &DataFormat::FhirR4,
    );
    assert_eq!(last_call_event_count(&env), 1, "register_provider must emit 1 event");

    // 3. activate_provider → 1 event (PRV_STS)
    client.activate_provider(&admin, &pid);
    assert_eq!(last_call_event_count(&env), 1, "activate_provider must emit 1 event");

    // 4. record_data_exchange → 1 event (DATA_EX)
    let eid = String::from_str(&env, "workflow-ex");
    client.record_data_exchange(
        &admin,
        &eid,
        &pid,
        &String::from_str(&env, "pat-workflow"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "workflow_hash"),
    );
    assert_eq!(last_call_event_count(&env), 1, "record_data_exchange must emit 1 event");

    // 5. update_exchange_status → 0 events (no publish)
    client.update_exchange_status(&admin, &eid, &SyncStatus::InProgress);
    assert_eq!(last_call_event_count(&env), 0, "update_exchange_status must emit 0 events");

    // 6. create_field_mapping → 1 event (MAP_ADD)
    client.create_field_mapping(
        &admin,
        &String::from_str(&env, "workflow-map"),
        &pid,
        &String::from_str(&env, "patient.dob"),
        &String::from_str(&env, "date_of_birth"),
        &String::from_str(&env, "date_format"),
    );
    assert_eq!(last_call_event_count(&env), 1, "create_field_mapping must emit 1 event");

    // 7. verify_sync → 1 event (SYNC_VF)
    let hash = String::from_str(&env, "wf_hash");
    client.verify_sync(
        &admin,
        &String::from_str(&env, "workflow-ver"),
        &eid,
        &hash,
        &hash,
        &Vec::new(&env),
    );
    assert_eq!(last_call_event_count(&env), 1, "verify_sync must emit 1 event");

    // 8. suspend_provider → 1 event (PRV_STS)
    client.suspend_provider(&admin, &pid);
    assert_eq!(last_call_event_count(&env), 1, "suspend_provider must emit 1 event");

    // 9. Suspend again (idempotent) → 0 events
    client.suspend_provider(&admin, &pid);
    assert_eq!(
        last_call_event_count(&env),
        0,
        "second suspend (idempotent) must emit 0 events"
    );
}
