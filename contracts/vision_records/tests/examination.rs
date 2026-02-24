mod common;

use common::{create_test_record, create_test_user, setup_test_env};
use soroban_sdk::String;
use vision_records::{
    AccessLevel, IntraocularPressure, OptFundusPhotography, OptPhysicalMeasurement,
    OptRetinalImaging, OptVisualField, PhysicalMeasurement, RecordType, Role, SlitLampFindings,
    VisualAcuity,
};

#[test]
fn test_add_and_get_eye_examination() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");

    let record_id = create_test_record(
        &ctx,
        &provider,
        &patient,
        &provider,
        RecordType::Examination,
        "e3b0c44298fc1c149afbf4c8996fb924",
    );

    let visual_acuity = VisualAcuity {
        uncorrected: PhysicalMeasurement {
            left_eye: String::from_str(&ctx.env, "20/20"),
            right_eye: String::from_str(&ctx.env, "20/20"),
        },
        corrected: OptPhysicalMeasurement::None,
    };

    let iop = IntraocularPressure {
        left_eye: 15,
        right_eye: 16,
        method: String::from_str(&ctx.env, "Goldmann"),
        timestamp: ctx.env.ledger().timestamp(),
    };

    let slit_lamp = SlitLampFindings {
        cornea: String::from_str(&ctx.env, "Clear"),
        anterior_chamber: String::from_str(&ctx.env, "Deep and quiet"),
        iris: String::from_str(&ctx.env, "Normal"),
        lens: String::from_str(&ctx.env, "Clear"),
    };

    let clinical_notes = String::from_str(&ctx.env, "Regular checkup, all normal.");

    ctx.client.add_eye_examination(
        &provider,
        &record_id,
        &visual_acuity,
        &iop,
        &slit_lamp,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &clinical_notes,
    );

    // Test get by provider
    let exam = ctx.client.get_eye_examination(&provider, &record_id);
    assert_eq!(
        exam.visual_acuity.uncorrected.left_eye,
        String::from_str(&ctx.env, "20/20")
    );
    assert_eq!(exam.iop.left_eye, 15);
    assert_eq!(exam.slit_lamp.cornea, String::from_str(&ctx.env, "Clear"));
    assert_eq!(
        exam.clinical_notes,
        String::from_str(&ctx.env, "Regular checkup, all normal.")
    );
}

#[test]
fn test_eye_examination_access_control() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient2");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider2");
    let other_provider = create_test_user(&ctx, Role::Optometrist, "Provider3");

    let record_id = create_test_record(
        &ctx,
        &provider,
        &patient,
        &provider,
        RecordType::Examination,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );

    let visual_acuity = VisualAcuity {
        uncorrected: PhysicalMeasurement {
            left_eye: String::from_str(&ctx.env, "20/40"),
            right_eye: String::from_str(&ctx.env, "20/40"),
        },
        corrected: OptPhysicalMeasurement::None,
    };

    let iop = IntraocularPressure {
        left_eye: 14,
        right_eye: 14,
        method: String::from_str(&ctx.env, "iCare"),
        timestamp: ctx.env.ledger().timestamp(),
    };

    let slit_lamp = SlitLampFindings {
        cornea: String::from_str(&ctx.env, "Arcus"),
        anterior_chamber: String::from_str(&ctx.env, "Quiet"),
        iris: String::from_str(&ctx.env, "Normal"),
        lens: String::from_str(&ctx.env, "NS 1+"),
    };

    ctx.client.add_eye_examination(
        &provider,
        &record_id,
        &visual_acuity,
        &iop,
        &slit_lamp,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&ctx.env, "Notes"),
    );

    // patient can read their own
    let exam_res_patient = ctx.client.try_get_eye_examination(&patient, &record_id);
    assert!(exam_res_patient.is_ok());

    // other provider cannot read without access
    let exam_res_other = ctx
        .client
        .try_get_eye_examination(&other_provider, &record_id);
    assert!(exam_res_other.is_err());

    // patient grants access to other provider
    ctx.client.grant_access(
        &patient,
        &patient,
        &other_provider,
        &AccessLevel::Read,
        &86400,
    );

    // other provider can read now
    let exam_res_other_after = ctx
        .client
        .try_get_eye_examination(&other_provider, &record_id);
    assert!(exam_res_other_after.is_ok());
}
