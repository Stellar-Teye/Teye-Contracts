#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects
)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Env, String, Vec};
use teye_common::concurrency::{
    FieldChange, ResolutionStrategy, UpdateOutcome, VersionStamp,
};

fn setup_env() -> (Env, Address, VisionRecordsContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    // Leak the env so the client lifetime works in tests.
    let env: &'static Env = Box::leak(Box::new(env));
    let client = VisionRecordsContractClient::new(env, &contract_id);
    (env.clone(), admin, client)
}

fn register_provider(
    client: &VisionRecordsContractClient,
    env: &Env,
    admin: &Address,
) -> Address {
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
    client: &VisionRecordsContractClient,
    env: &Env,
    admin: &Address,
) -> Address {
    let patient = Address::generate(env);
    client.register_user(
        admin,
        &patient,
        &Role::Patient,
        &String::from_str(env, "Patient"),
    );
    patient
}

fn add_exam_record(
    client: &VisionRecordsContractClient,
    env: &Env,
    admin: &Address,
    patient: &Address,
    provider: &Address,
) -> u64 {
    let data_hash = String::from_str(env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    client.add_record(
        admin,
        patient,
        provider,
        &RecordType::Examination,
        &data_hash,
    )
}

#[test]
fn test_version_stamp_initialised_on_add_record() {
    let (env, admin, client) = setup_env();
    let provider = register_provider(&client, &env, &admin);
    let patient = register_patient(&client, &env, &admin);

    let record_id = add_exam_record(&client, &env, &admin, &patient, &provider);

    let stamp = client.get_record_version_stamp(&record_id);
    assert_eq!(stamp.version, 1);
}

#[test]
fn test_clean_versioned_examination_update() {
    let (env, admin, client) = setup_env();
    let provider = register_provider(&client, &env, &admin);
    let patient = register_patient(&client, &env, &admin);

    let record_id = add_exam_record(&client, &env, &admin, &patient, &provider);

    // Grant consent & access so provider can read the record.
    client.grant_consent(
        &patient,
        &provider,
        &ConsentType::Treatment,
        &157_680_000u64,
    );
    client.grant_access(
        &patient,
        &patient,
        &provider,
        &AccessLevel::Full,
        &157_680_000u64,
    );

    let stamp = client.get_record_version_stamp(&record_id);

    let va = VisualAcuity {
        uncorrected: examination::PhysicalMeasurement {
            left_eye: String::from_str(&env, "20/20"),
            right_eye: String::from_str(&env, "20/25"),
        },
        corrected: examination::OptPhysicalMeasurement::None,
    };
    let iop = IntraocularPressure {
        left_eye: 14,
        right_eye: 15,
        method: String::from_str(&env, "Goldmann"),
        timestamp: 1000,
    };
    let slit = SlitLampFindings {
        cornea: String::from_str(&env, "clear"),
        anterior_chamber: String::from_str(&env, "deep"),
        iris: String::from_str(&env, "normal"),
        lens: String::from_str(&env, "clear"),
    };

    let mut changed = Vec::new(&env);
    changed.push_back(FieldChange {
        field_name: String::from_str(&env, "visual_acuity"),
        old_hash: String::from_str(&env, "none"),
        new_hash: String::from_str(&env, "va_hash_1"),
    });

    let outcome = client.update_examination_versioned(
        &provider,
        &record_id,
        &stamp.version,
        &1u32,
        &va,
        &iop,
        &slit,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "Initial exam"),
        &changed,
    );

    // The outcome should be Applied with version bumped to 2.
    match outcome {
        UpdateOutcome::Applied(s) => {
            assert_eq!(s.version, 2);
        }
        other => panic!("Expected Applied, got {:?}", other),
    }

    // Version stamp should now be 2.
    let new_stamp = client.get_record_version_stamp(&record_id);
    assert_eq!(new_stamp.version, 2);
}

#[test]
fn test_stale_version_triggers_conflict_manual_review() {
    let (env, admin, client) = setup_env();
    let provider = register_provider(&client, &env, &admin);
    let patient = register_patient(&client, &env, &admin);

    let record_id = add_exam_record(&client, &env, &admin, &patient, &provider);

    client.grant_consent(
        &patient,
        &provider,
        &ConsentType::Treatment,
        &157_680_000u64,
    );
    client.grant_access(
        &patient,
        &patient,
        &provider,
        &AccessLevel::Full,
        &157_680_000u64,
    );

    // Set strategy to ManualReview.
    client.set_record_resolution_strategy(
        &provider,
        &record_id,
        &ResolutionStrategy::ManualReview,
    );

    let stamp_v1 = client.get_record_version_stamp(&record_id);

    let va = VisualAcuity {
        uncorrected: examination::PhysicalMeasurement {
            left_eye: String::from_str(&env, "20/20"),
            right_eye: String::from_str(&env, "20/25"),
        },
        corrected: examination::OptPhysicalMeasurement::None,
    };
    let iop = IntraocularPressure {
        left_eye: 14,
        right_eye: 15,
        method: String::from_str(&env, "Goldmann"),
        timestamp: 1000,
    };
    let slit = SlitLampFindings {
        cornea: String::from_str(&env, "clear"),
        anterior_chamber: String::from_str(&env, "deep"),
        iris: String::from_str(&env, "normal"),
        lens: String::from_str(&env, "clear"),
    };

    let mut changed_a = Vec::new(&env);
    changed_a.push_back(FieldChange {
        field_name: String::from_str(&env, "visual_acuity"),
        old_hash: String::from_str(&env, "none"),
        new_hash: String::from_str(&env, "va_hash_1"),
    });

    // First update succeeds (version 1 -> 2).
    let first = client.update_examination_versioned(
        &provider,
        &record_id,
        &stamp_v1.version,
        &1u32,
        &va,
        &iop,
        &slit,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "First update"),
        &changed_a,
    );
    assert!(matches!(first, UpdateOutcome::Applied(_)));

    // Second update uses stale version 1 -> should detect conflict.
    let mut changed_b = Vec::new(&env);
    changed_b.push_back(FieldChange {
        field_name: String::from_str(&env, "iop"),
        old_hash: String::from_str(&env, "none"),
        new_hash: String::from_str(&env, "iop_hash_1"),
    });

    let second = client.update_examination_versioned(
        &provider,
        &record_id,
        &stamp_v1.version,
        &2u32,
        &va,
        &iop,
        &slit,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "Second update (stale)"),
        &changed_b,
    );

    match second {
        UpdateOutcome::Conflicted(cid) => {
            assert!(cid > 0);
            // Verify it's in the pending conflicts.
            let pending = client.get_pending_conflicts();
            assert!(!pending.is_empty());
        }
        other => panic!("Expected Conflicted, got {:?}", other),
    }
}

#[test]
fn test_last_writer_wins_strategy() {
    let (env, admin, client) = setup_env();
    let provider = register_provider(&client, &env, &admin);
    let patient = register_patient(&client, &env, &admin);

    let record_id = add_exam_record(&client, &env, &admin, &patient, &provider);

    client.grant_consent(
        &patient,
        &provider,
        &ConsentType::Treatment,
        &157_680_000u64,
    );
    client.grant_access(
        &patient,
        &patient,
        &provider,
        &AccessLevel::Full,
        &157_680_000u64,
    );

    // Set LWW strategy.
    client.set_record_resolution_strategy(
        &provider,
        &record_id,
        &ResolutionStrategy::LastWriterWins,
    );

    let stamp_v1 = client.get_record_version_stamp(&record_id);

    let va = VisualAcuity {
        uncorrected: examination::PhysicalMeasurement {
            left_eye: String::from_str(&env, "20/20"),
            right_eye: String::from_str(&env, "20/25"),
        },
        corrected: examination::OptPhysicalMeasurement::None,
    };
    let iop = IntraocularPressure {
        left_eye: 14,
        right_eye: 15,
        method: String::from_str(&env, "Goldmann"),
        timestamp: 1000,
    };
    let slit = SlitLampFindings {
        cornea: String::from_str(&env, "clear"),
        anterior_chamber: String::from_str(&env, "deep"),
        iris: String::from_str(&env, "normal"),
        lens: String::from_str(&env, "clear"),
    };

    let changed = Vec::new(&env);

    // First update.
    client.update_examination_versioned(
        &provider,
        &record_id,
        &stamp_v1.version,
        &1u32,
        &va,
        &iop,
        &slit,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "First"),
        &changed,
    );

    // Second update with stale version — LWW should still apply.
    let outcome = client.update_examination_versioned(
        &provider,
        &record_id,
        &stamp_v1.version,
        &2u32,
        &va,
        &iop,
        &slit,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "LWW override"),
        &changed,
    );

    assert!(matches!(outcome, UpdateOutcome::Applied(_)));
}

#[test]
fn test_resolve_conflict() {
    let (env, admin, client) = setup_env();
    let provider = register_provider(&client, &env, &admin);
    let patient = register_patient(&client, &env, &admin);

    let record_id = add_exam_record(&client, &env, &admin, &patient, &provider);

    client.grant_consent(
        &patient,
        &provider,
        &ConsentType::Treatment,
        &157_680_000u64,
    );
    client.grant_access(
        &patient,
        &patient,
        &provider,
        &AccessLevel::Full,
        &157_680_000u64,
    );

    client.set_record_resolution_strategy(
        &provider,
        &record_id,
        &ResolutionStrategy::ManualReview,
    );

    let stamp_v1 = client.get_record_version_stamp(&record_id);

    let va = VisualAcuity {
        uncorrected: examination::PhysicalMeasurement {
            left_eye: String::from_str(&env, "20/20"),
            right_eye: String::from_str(&env, "20/25"),
        },
        corrected: examination::OptPhysicalMeasurement::None,
    };
    let iop = IntraocularPressure {
        left_eye: 14,
        right_eye: 15,
        method: String::from_str(&env, "Goldmann"),
        timestamp: 1000,
    };
    let slit = SlitLampFindings {
        cornea: String::from_str(&env, "clear"),
        anterior_chamber: String::from_str(&env, "deep"),
        iris: String::from_str(&env, "normal"),
        lens: String::from_str(&env, "clear"),
    };

    let changed = Vec::new(&env);

    // First update succeeds.
    client.update_examination_versioned(
        &provider,
        &record_id,
        &stamp_v1.version,
        &1u32,
        &va,
        &iop,
        &slit,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "First"),
        &changed,
    );

    // Second with stale version triggers conflict.
    let outcome = client.update_examination_versioned(
        &provider,
        &record_id,
        &stamp_v1.version,
        &2u32,
        &va,
        &iop,
        &slit,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "Stale"),
        &changed,
    );

    let conflict_id = match outcome {
        UpdateOutcome::Conflicted(cid) => cid,
        other => panic!("Expected Conflicted, got {:?}", other),
    };

    // Admin resolves the conflict.
    client.resolve_conflict(&admin, &conflict_id, &record_id);

    // Pending conflicts should be empty now.
    let pending = client.get_pending_conflicts();
    assert!(pending.is_empty());
}

#[test]
fn test_merge_strategy_non_overlapping_fields() {
    let (env, admin, client) = setup_env();
    let provider = register_provider(&client, &env, &admin);
    let patient = register_patient(&client, &env, &admin);

    let record_id = add_exam_record(&client, &env, &admin, &patient, &provider);

    client.grant_consent(
        &patient,
        &provider,
        &ConsentType::Treatment,
        &157_680_000u64,
    );
    client.grant_access(
        &patient,
        &patient,
        &provider,
        &AccessLevel::Full,
        &157_680_000u64,
    );

    client.set_record_resolution_strategy(
        &provider,
        &record_id,
        &ResolutionStrategy::Merge,
    );

    let stamp_v1 = client.get_record_version_stamp(&record_id);

    let va = VisualAcuity {
        uncorrected: examination::PhysicalMeasurement {
            left_eye: String::from_str(&env, "20/20"),
            right_eye: String::from_str(&env, "20/25"),
        },
        corrected: examination::OptPhysicalMeasurement::None,
    };
    let iop = IntraocularPressure {
        left_eye: 14,
        right_eye: 15,
        method: String::from_str(&env, "Goldmann"),
        timestamp: 1000,
    };
    let slit = SlitLampFindings {
        cornea: String::from_str(&env, "clear"),
        anterior_chamber: String::from_str(&env, "deep"),
        iris: String::from_str(&env, "normal"),
        lens: String::from_str(&env, "clear"),
    };

    // First update touches "visual_acuity".
    let mut changed_a = Vec::new(&env);
    changed_a.push_back(FieldChange {
        field_name: String::from_str(&env, "visual_acuity"),
        old_hash: String::from_str(&env, "none"),
        new_hash: String::from_str(&env, "va_hash_1"),
    });

    client.update_examination_versioned(
        &provider,
        &record_id,
        &stamp_v1.version,
        &1u32,
        &va,
        &iop,
        &slit,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "Update VA"),
        &changed_a,
    );

    // Second update with stale version touches a different field ("iop").
    let mut changed_b = Vec::new(&env);
    changed_b.push_back(FieldChange {
        field_name: String::from_str(&env, "iop"),
        old_hash: String::from_str(&env, "none"),
        new_hash: String::from_str(&env, "iop_hash_1"),
    });

    let outcome = client.update_examination_versioned(
        &provider,
        &record_id,
        &stamp_v1.version,
        &2u32,
        &va,
        &iop,
        &slit,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "Update IOP (merge)"),
        &changed_b,
    );

    // Non-overlapping fields should merge.
    assert!(matches!(outcome, UpdateOutcome::Merged(_)));
}

#[test]
fn test_get_record_conflicts_returns_for_specific_record() {
    let (env, admin, client) = setup_env();
    let provider = register_provider(&client, &env, &admin);
    let patient = register_patient(&client, &env, &admin);

    let record_id = add_exam_record(&client, &env, &admin, &patient, &provider);

    client.grant_consent(
        &patient,
        &provider,
        &ConsentType::Treatment,
        &157_680_000u64,
    );
    client.grant_access(
        &patient,
        &patient,
        &provider,
        &AccessLevel::Full,
        &157_680_000u64,
    );

    client.set_record_resolution_strategy(
        &provider,
        &record_id,
        &ResolutionStrategy::ManualReview,
    );

    let stamp_v1 = client.get_record_version_stamp(&record_id);

    let va = VisualAcuity {
        uncorrected: examination::PhysicalMeasurement {
            left_eye: String::from_str(&env, "20/20"),
            right_eye: String::from_str(&env, "20/25"),
        },
        corrected: examination::OptPhysicalMeasurement::None,
    };
    let iop = IntraocularPressure {
        left_eye: 14,
        right_eye: 15,
        method: String::from_str(&env, "Goldmann"),
        timestamp: 1000,
    };
    let slit = SlitLampFindings {
        cornea: String::from_str(&env, "clear"),
        anterior_chamber: String::from_str(&env, "deep"),
        iris: String::from_str(&env, "normal"),
        lens: String::from_str(&env, "clear"),
    };

    let changed = Vec::new(&env);

    // Apply a clean update first.
    client.update_examination_versioned(
        &provider,
        &record_id,
        &stamp_v1.version,
        &1u32,
        &va,
        &iop,
        &slit,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "First"),
        &changed,
    );

    // Stale update creates a conflict.
    client.update_examination_versioned(
        &provider,
        &record_id,
        &stamp_v1.version,
        &2u32,
        &va,
        &iop,
        &slit,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "Stale"),
        &changed,
    );

    let conflicts = client.get_record_conflicts(&record_id);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts.get(0).unwrap().record_id, record_id);
}
