#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects
)]

use super::{
    AccessLevel, BatchGrantInput, BatchRecordInput, ContractError, RecordType, Role,
    VisionRecordsContract, VisionRecordsContractClient,
};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, String, Vec};

// ── Helpers ──────────────────────────────────────────────────────

fn setup() -> (Env, VisionRecordsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

fn register_provider(env: &Env, client: &VisionRecordsContractClient, admin: &Address) -> Address {
    let provider = Address::generate(env);
    client.register_user(
        admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(env, "Dr. Provider"),
    );
    provider
}

fn register_patient(
    env: &Env,
    client: &VisionRecordsContractClient,
    admin: &Address,
    name: &str,
) -> Address {
    let patient = Address::generate(env);
    client.register_user(
        admin,
        &patient,
        &Role::Patient,
        &String::from_str(env, name),
    );
    patient
}

// ======================== Batch Record Creation ========================

#[test]
fn test_batch_add_records_single() {
    let (env, client, admin) = setup();
    let provider = register_provider(&env, &client, &admin);
    let patient = register_patient(&env, &client, &admin, "Alice");

    let mut inputs = Vec::new(&env);
    inputs.push_back(BatchRecordInput {
        patient: patient.clone(),
        record_type: RecordType::Examination,
        data_hash: String::from_str(&env, "hash_a"),
    });

    let ids = client.add_records(&provider, &inputs);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids.get(0).unwrap(), 1);

    let record = client.get_record(&1);
    assert_eq!(record.patient, patient);
    assert_eq!(record.provider, provider);
    assert_eq!(record.record_type, RecordType::Examination);
    assert_eq!(client.get_record_count(), 1);
}

#[test]
fn test_batch_add_records_multiple() {
    let (env, client, admin) = setup();
    let provider = register_provider(&env, &client, &admin);
    let patient_a = register_patient(&env, &client, &admin, "Alice");
    let patient_b = register_patient(&env, &client, &admin, "Bob");

    let mut inputs = Vec::new(&env);
    inputs.push_back(BatchRecordInput {
        patient: patient_a.clone(),
        record_type: RecordType::Examination,
        data_hash: String::from_str(&env, "hash_1"),
    });
    inputs.push_back(BatchRecordInput {
        patient: patient_b.clone(),
        record_type: RecordType::Prescription,
        data_hash: String::from_str(&env, "hash_2"),
    });
    inputs.push_back(BatchRecordInput {
        patient: patient_a.clone(),
        record_type: RecordType::LabResult,
        data_hash: String::from_str(&env, "hash_3"),
    });

    let ids = client.add_records(&provider, &inputs);
    assert_eq!(ids.len(), 3);
    assert_eq!(ids.get(0).unwrap(), 1);
    assert_eq!(ids.get(1).unwrap(), 2);
    assert_eq!(ids.get(2).unwrap(), 3);

    // Verify records stored correctly
    let rec1 = client.get_record(&1);
    assert_eq!(rec1.patient, patient_a);
    assert_eq!(rec1.record_type, RecordType::Examination);

    let rec2 = client.get_record(&2);
    assert_eq!(rec2.patient, patient_b);
    assert_eq!(rec2.record_type, RecordType::Prescription);

    let rec3 = client.get_record(&3);
    assert_eq!(rec3.patient, patient_a);
    assert_eq!(rec3.record_type, RecordType::LabResult);

    // Verify patient record indices
    let a_recs = client.get_patient_records(&patient_a);
    assert_eq!(a_recs.len(), 2);
    assert_eq!(a_recs.get(0).unwrap(), 1);
    assert_eq!(a_recs.get(1).unwrap(), 3);

    let b_recs = client.get_patient_records(&patient_b);
    assert_eq!(b_recs.len(), 1);
    assert_eq!(b_recs.get(0).unwrap(), 2);

    assert_eq!(client.get_record_count(), 3);
}

#[test]
fn test_batch_add_records_unauthorized() {
    let (env, client, admin) = setup();
    let patient = register_patient(&env, &client, &admin, "Alice");

    // Patient does not have WriteRecord permission
    let mut inputs = Vec::new(&env);
    inputs.push_back(BatchRecordInput {
        patient: patient.clone(),
        record_type: RecordType::Examination,
        data_hash: String::from_str(&env, "hash"),
    });

    let result = client.try_add_records(&patient, &inputs);
    assert_eq!(result.err().unwrap().unwrap(), ContractError::Unauthorized);
}

#[test]
fn test_batch_add_records_empty_input() {
    let (env, client, admin) = setup();
    let provider = register_provider(&env, &client, &admin);

    let inputs: Vec<BatchRecordInput> = Vec::new(&env);
    let result = client.try_add_records(&provider, &inputs);
    assert_eq!(result.err().unwrap().unwrap(), ContractError::InvalidInput);
}

#[test]
fn test_batch_add_records_admin_can_create() {
    let (env, client, admin) = setup();
    let patient = register_patient(&env, &client, &admin, "Alice");

    // Admin has SystemAdmin permission, so can batch-create records
    let mut inputs = Vec::new(&env);
    inputs.push_back(BatchRecordInput {
        patient: patient.clone(),
        record_type: RecordType::Surgery,
        data_hash: String::from_str(&env, "surgery_hash"),
    });

    let ids = client.add_records(&admin, &inputs);
    assert_eq!(ids.len(), 1);
    let record = client.get_record(&ids.get(0).unwrap());
    assert_eq!(record.record_type, RecordType::Surgery);
}

#[test]
fn test_batch_add_records_counter_continuity() {
    let (env, client, admin) = setup();
    let provider = register_provider(&env, &client, &admin);
    let patient = register_patient(&env, &client, &admin, "Alice");

    // First add a single record via add_record
    client.add_record(
        &provider,
        &patient,
        &provider,
        &RecordType::Examination,
        &String::from_str(&env, "single_hash"),
    );
    assert_eq!(client.get_record_count(), 1);

    // Now batch-add more records — IDs should continue from 2
    let mut inputs = Vec::new(&env);
    inputs.push_back(BatchRecordInput {
        patient: patient.clone(),
        record_type: RecordType::Diagnosis,
        data_hash: String::from_str(&env, "batch_hash_1"),
    });
    inputs.push_back(BatchRecordInput {
        patient: patient.clone(),
        record_type: RecordType::Treatment,
        data_hash: String::from_str(&env, "batch_hash_2"),
    });

    let ids = client.add_records(&provider, &inputs);
    assert_eq!(ids.get(0).unwrap(), 2);
    assert_eq!(ids.get(1).unwrap(), 3);
    assert_eq!(client.get_record_count(), 3);
}

// ======================== Batch Record Retrieval ========================

#[test]
fn test_batch_get_records() {
    let (env, client, admin) = setup();
    let provider = register_provider(&env, &client, &admin);
    let patient = register_patient(&env, &client, &admin, "Alice");

    let hashes = [
        String::from_str(&env, "hash_0"),
        String::from_str(&env, "hash_1"),
        String::from_str(&env, "hash_2"),
        String::from_str(&env, "hash_3"),
    ];

    let mut inputs = Vec::new(&env);
    for hash in hashes.iter() {
        inputs.push_back(BatchRecordInput {
            patient: patient.clone(),
            record_type: RecordType::Examination,
            data_hash: hash.clone(),
        });
    }

    let ids = client.add_records(&provider, &inputs);
    assert_eq!(ids.len(), 4);

    // Retrieve a subset
    let mut subset = Vec::new(&env);
    subset.push_back(1u64);
    subset.push_back(3u64);

    let records = client.get_records(&subset);
    assert_eq!(records.len(), 2);
    assert_eq!(records.get(0).unwrap().id, 1);
    assert_eq!(records.get(1).unwrap().id, 3);
}

#[test]
fn test_batch_get_records_not_found() {
    let (env, client, _admin) = setup();

    let mut ids = Vec::new(&env);
    ids.push_back(999u64);

    let result = client.try_get_records(&ids);
    assert_eq!(
        result.err().unwrap().unwrap(),
        ContractError::RecordNotFound
    );
}

#[test]
fn test_batch_get_records_partial_not_found() {
    let (env, client, admin) = setup();
    let provider = register_provider(&env, &client, &admin);
    let patient = register_patient(&env, &client, &admin, "Alice");

    let mut inputs = Vec::new(&env);
    inputs.push_back(BatchRecordInput {
        patient: patient.clone(),
        record_type: RecordType::Examination,
        data_hash: String::from_str(&env, "hash_1"),
    });
    client.add_records(&provider, &inputs);

    // Try to get existing + non-existing record
    let mut ids = Vec::new(&env);
    ids.push_back(1u64);
    ids.push_back(999u64);

    let result = client.try_get_records(&ids);
    assert_eq!(
        result.err().unwrap().unwrap(),
        ContractError::RecordNotFound
    );
}

// ======================== Batch Access Grants ========================

#[test]
fn test_batch_grant_access_multiple() {
    let (env, client, admin) = setup();
    let patient = register_patient(&env, &client, &admin, "Alice");
    let doc1 = register_provider(&env, &client, &admin);
    let doc2 = register_provider(&env, &client, &admin);

    env.ledger().set_timestamp(1000);

    let mut grants = Vec::new(&env);
    grants.push_back(BatchGrantInput {
        grantee: doc1.clone(),
        level: AccessLevel::Read,
        duration_seconds: 3600,
    });
    grants.push_back(BatchGrantInput {
        grantee: doc2.clone(),
        level: AccessLevel::Full,
        duration_seconds: 7200,
    });

    client.grant_access_batch(&patient, &grants);

    assert_eq!(client.check_access(&patient, &doc1), AccessLevel::Read);
    assert_eq!(client.check_access(&patient, &doc2), AccessLevel::Full);
}

#[test]
fn test_batch_grant_access_empty_input() {
    let (env, client, admin) = setup();
    let patient = register_patient(&env, &client, &admin, "Alice");

    let grants: Vec<BatchGrantInput> = Vec::new(&env);
    let result = client.try_grant_access_batch(&patient, &grants);
    assert_eq!(result.err().unwrap().unwrap(), ContractError::InvalidInput);
}

#[test]
fn test_batch_grant_access_expiration() {
    let (env, client, admin) = setup();
    let patient = register_patient(&env, &client, &admin, "Alice");
    let doc = register_provider(&env, &client, &admin);

    env.ledger().set_timestamp(1000);

    let mut grants = Vec::new(&env);
    grants.push_back(BatchGrantInput {
        grantee: doc.clone(),
        level: AccessLevel::Read,
        duration_seconds: 500, // expires at 1500
    });

    client.grant_access_batch(&patient, &grants);
    assert_eq!(client.check_access(&patient, &doc), AccessLevel::Read);

    // Advance time past expiration
    env.ledger().set_timestamp(1501);
    assert_eq!(client.check_access(&patient, &doc), AccessLevel::None);
}

#[test]
fn test_batch_grant_access_overwrite() {
    let (env, client, admin) = setup();
    let patient = register_patient(&env, &client, &admin, "Alice");
    let doc = register_provider(&env, &client, &admin);

    env.ledger().set_timestamp(1000);

    // First grant Read access
    let mut grants1 = Vec::new(&env);
    grants1.push_back(BatchGrantInput {
        grantee: doc.clone(),
        level: AccessLevel::Read,
        duration_seconds: 3600,
    });
    client.grant_access_batch(&patient, &grants1);
    assert_eq!(client.check_access(&patient, &doc), AccessLevel::Read);

    // Overwrite with Full access via batch
    let mut grants2 = Vec::new(&env);
    grants2.push_back(BatchGrantInput {
        grantee: doc.clone(),
        level: AccessLevel::Full,
        duration_seconds: 7200,
    });
    client.grant_access_batch(&patient, &grants2);
    assert_eq!(client.check_access(&patient, &doc), AccessLevel::Full);
}

// ======================== Atomicity / Gas Optimization ========================

#[test]
fn test_batch_records_atomic_counter() {
    // Verifies the counter is updated once at the end (gas optimization),
    // not per-record, ensuring atomicity.
    let (env, client, admin) = setup();
    let provider = register_provider(&env, &client, &admin);
    let patient = register_patient(&env, &client, &admin, "Alice");

    let mut inputs = Vec::new(&env);
    for _ in 0..5u32 {
        inputs.push_back(BatchRecordInput {
            patient: patient.clone(),
            record_type: RecordType::Examination,
            data_hash: String::from_str(&env, "h"),
        });
    }

    let ids = client.add_records(&provider, &inputs);
    assert_eq!(ids.len(), 5);

    // Counter should reflect all 5 records
    assert_eq!(client.get_record_count(), 5);

    // All IDs are sequential
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(id, (i as u64) + 1);
    }
}

#[test]
fn test_batch_add_and_retrieve_round_trip() {
    // Full round trip: batch create → batch retrieve → verify all data intact
    let (env, client, admin) = setup();
    let provider = register_provider(&env, &client, &admin);
    let patient = register_patient(&env, &client, &admin, "Alice");

    let mut inputs = Vec::new(&env);
    inputs.push_back(BatchRecordInput {
        patient: patient.clone(),
        record_type: RecordType::Examination,
        data_hash: String::from_str(&env, "exam_data"),
    });
    inputs.push_back(BatchRecordInput {
        patient: patient.clone(),
        record_type: RecordType::Prescription,
        data_hash: String::from_str(&env, "rx_data"),
    });

    let ids = client.add_records(&provider, &inputs);

    // Retrieve all via batch
    let records = client.get_records(&ids);
    assert_eq!(records.len(), 2);

    assert_eq!(records.get(0).unwrap().record_type, RecordType::Examination);
    assert_eq!(
        records.get(1).unwrap().record_type,
        RecordType::Prescription
    );
    assert_eq!(records.get(0).unwrap().provider, provider);
    assert_eq!(records.get(1).unwrap().provider, provider);
}
