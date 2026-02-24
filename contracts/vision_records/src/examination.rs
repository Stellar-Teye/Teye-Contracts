#![allow(clippy::arithmetic_side_effects)]
use soroban_sdk::{contracttype, symbol_short, Env, String, Symbol};

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

fn extend_ttl_exam_key(env: &Env, key: &(Symbol, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalMeasurement {
    pub left_eye: String,
    pub right_eye: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptPhysicalMeasurement {
    None,
    Some(PhysicalMeasurement),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualAcuity {
    pub uncorrected: PhysicalMeasurement,
    pub corrected: OptPhysicalMeasurement,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntraocularPressure {
    pub left_eye: u32,
    pub right_eye: u32,
    pub method: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlitLampFindings {
    pub cornea: String,
    pub anterior_chamber: String,
    pub iris: String,
    pub lens: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualField {
    pub left_eye_reliability: String,
    pub right_eye_reliability: String,
    pub left_eye_defects: String,
    pub right_eye_defects: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptVisualField {
    None,
    Some(VisualField),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetinalImaging {
    pub image_url: String,
    pub image_hash: String,
    pub findings: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptRetinalImaging {
    None,
    Some(RetinalImaging),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundusPhotography {
    pub image_url: String,
    pub image_hash: String,
    pub cup_to_disc_ratio_left: String,
    pub cup_to_disc_ratio_right: String,
    pub macula_status: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptFundusPhotography {
    None,
    Some(FundusPhotography),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EyeExamination {
    pub record_id: u64,
    pub visual_acuity: VisualAcuity,
    pub iop: IntraocularPressure,
    pub slit_lamp: SlitLampFindings,
    pub visual_field: OptVisualField,
    pub retina_imaging: OptRetinalImaging,
    pub fundus_photo: OptFundusPhotography,
    pub clinical_notes: String,
}

pub fn exam_key(record_id: u64) -> (Symbol, u64) {
    (symbol_short!("EXAM"), record_id)
}

pub fn get_examination(env: &Env, record_id: u64) -> Option<EyeExamination> {
    let key = exam_key(record_id);
    env.storage().persistent().get(&key)
}

pub fn set_examination(env: &Env, exam: &EyeExamination) {
    let key = exam_key(exam.record_id);
    env.storage().persistent().set(&key, exam);
    extend_ttl_exam_key(env, &key);
}

pub fn remove_examination(env: &Env, record_id: u64) {
    let key = exam_key(record_id);
    env.storage().persistent().remove(&key);
}
