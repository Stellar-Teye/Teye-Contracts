#![allow(clippy::arithmetic_side_effects)]
use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol, Vec};
use teye_common::concurrency::{self, FieldChange, UpdateOutcome, VersionStamp};
use teye_common::lineage::{self, RelationshipKind};

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
#[allow(clippy::large_enum_variant)]
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
#[allow(clippy::large_enum_variant)]
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
#[allow(clippy::large_enum_variant)]
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

/// Stores an eye examination record and initialises its lineage node.
///
/// On first write the node is created and a `Created` edge is recorded from
/// the provider address.  Subsequent writes (detected by the node already
/// existing) record a `ModifiedBy` edge instead.  Use
/// [`versioned_set_examination`] for OCC-controlled updates.
pub fn set_examination(env: &Env, exam: &EyeExamination, provider: &Address) {
    let key = exam_key(exam.record_id);
    env.storage().persistent().set(&key, exam);
    extend_ttl_exam_key(env, &key);

    // Lineage: create or extend the provenance node for this examination.
    let (_, is_new) = lineage::create_node(
        env,
        exam.record_id,
        provider.clone(),
        "Examination",
        None,
    );

    if is_new {
        // Genesis edge: Created(provider → record)
        lineage::add_edge(
            env,
            exam.record_id, // self-referential source for genesis
            exam.record_id,
            RelationshipKind::Created,
            provider.clone(),
            None,
        );
    }
}

pub fn remove_examination(env: &Env, record_id: u64) {
    let key = exam_key(record_id);
    env.storage().persistent().remove(&key);
}

/// Performs a versioned (OCC) update of an eye examination record.
///
/// The caller must supply the `expected_version` they read before making
/// modifications, along with the `node_id` that identifies the provider in
/// the vector clock and the list of field-level changes.
///
/// Returns the [`UpdateOutcome`] so the contract layer can decide whether
/// the update was applied, merged, or queued as a conflict.
pub fn versioned_set_examination(
    env: &Env,
    exam: &EyeExamination,
    expected_version: u64,
    node_id: u32,
    provider: &soroban_sdk::Address,
    changed_fields: &Vec<FieldChange>,
) -> UpdateOutcome {
    let outcome = concurrency::compare_and_swap(
        env,
        exam.record_id,
        expected_version,
        node_id,
        provider,
        changed_fields,
    );

    match &outcome {
        UpdateOutcome::Applied(_) | UpdateOutcome::Merged(_) => {
            let key = exam_key(exam.record_id);
            env.storage().persistent().set(&key, exam);
            extend_ttl_exam_key(env, &key);
            concurrency::save_field_snapshot(env, exam.record_id, changed_fields);

            // Lineage: record a ModifiedBy edge for each successful mutation.
            lineage::add_edge(
                env,
                exam.record_id,
                exam.record_id,
                RelationshipKind::ModifiedBy,
                provider.clone(),
                None,
            );
        }
        UpdateOutcome::Conflicted(_) => {
            // Record not updated — conflict must be resolved first.
        }
    }

    outcome
}

/// Retrieves the current OCC version stamp for an examination record.
pub fn get_exam_version(env: &Env, record_id: u64) -> VersionStamp {
    concurrency::get_version_stamp(env, record_id)
}
