use soroban_sdk::{contracttype, Address, Env, String, Vec};
use teye_common::concurrency::{self, FieldChange, UpdateOutcome, VersionStamp};
use teye_common::lineage::{self, RelationshipKind};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LensType {
    Glasses,
    ContactLens,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrescriptionData {
    pub sphere: String,   // SPH
    pub cylinder: String, // CYL
    pub axis: String,     // AXIS
    pub add: String,      // ADD
    pub pd: String,       // Pupillary Distance
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContactLensData {
    pub base_curve: String,
    pub diameter: String,
    pub brand: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum OptionalContactLensData {
    None,
    Some(ContactLensData),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Prescription {
    pub id: u64,
    pub patient: Address,
    pub provider: Address,
    pub lens_type: LensType,
    pub left_eye: PrescriptionData,
    pub right_eye: PrescriptionData,
    pub contact_data: OptionalContactLensData,
    pub issued_at: u64,
    pub expires_at: u64,
    pub verified: bool,
    pub metadata_hash: String,
}

/// Persists a prescription and initialises its lineage node.
///
/// If `derived_from_exam_id` is `Some(exam_id)`, a `DerivedFrom` edge is
/// added linking this prescription back to the source examination.  This is
/// the canonical exam → prescription provenance link required by the issue.
///
/// # Parameters
/// - `exam_record_id` — optional examination record that this prescription
///   was derived from.  Pass `None` for standalone prescriptions.
pub fn save_prescription(
    env: &Env,
    prescription: &Prescription,
    exam_record_id: Option<u64>,
) {
    let key = (soroban_sdk::symbol_short!("RX"), prescription.id);
    env.storage().persistent().set(&key, prescription);

    // Track patient history
    let history_key = (
        soroban_sdk::symbol_short!("RX_HIST"),
        prescription.patient.clone(),
    );
    let mut history: Vec<u64> = env
        .storage()
        .persistent()
        .get(&history_key)
        .unwrap_or(Vec::new(env));
    history.push_back(prescription.id);
    env.storage().persistent().set(&history_key, &history);

    // Lineage: create node and edges.
    lineage::create_node(
        env,
        prescription.id,
        prescription.provider.clone(),
        "Prescription",
        None,
    );

    // Genesis Created edge.
    lineage::add_edge(
        env,
        prescription.id,
        prescription.id,
        RelationshipKind::Created,
        prescription.provider.clone(),
        None,
    );

    // If derived from an examination, add a DerivedFrom edge.
    if let Some(exam_id) = exam_record_id {
        lineage::add_edge(
            env,
            exam_id,          // source: examination
            prescription.id,  // target: prescription
            RelationshipKind::DerivedFrom,
            prescription.provider.clone(),
            None,
        );
    }
}

pub fn get_prescription(env: &Env, id: u64) -> Option<Prescription> {
    let key = (soroban_sdk::symbol_short!("RX"), id);
    env.storage().persistent().get(&key)
}

pub fn get_patient_history(env: &Env, patient: Address) -> Vec<u64> {
    let history_key = (soroban_sdk::symbol_short!("RX_HIST"), patient);
    env.storage()
        .persistent()
        .get(&history_key)
        .unwrap_or(Vec::new(env))
}

pub fn verify_prescription(env: &Env, id: u64, verifier: Address) -> bool {
    if let Some(mut rx) = get_prescription(env, id) {
        verifier.require_auth();
        rx.verified = true;
        let key = (soroban_sdk::symbol_short!("RX"), id);
        env.storage().persistent().set(&key, &rx);
        return true;
    }
    false
}

/// Performs a versioned (OCC) update of a prescription record.
///
/// The caller supplies the `expected_version` they read before making edits,
/// a `node_id` for the vector clock, and a list of field-level changes.
/// A `ModifiedBy` lineage edge is recorded for every successful update.
///
/// Returns [`UpdateOutcome`] indicating whether the update was applied,
/// merged, or queued as a conflict.
pub fn versioned_save_prescription(
    env: &Env,
    prescription: &Prescription,
    expected_version: u64,
    node_id: u32,
    provider: &Address,
    changed_fields: &Vec<FieldChange>,
) -> UpdateOutcome {
    let outcome = concurrency::compare_and_swap(
        env,
        prescription.id,
        expected_version,
        node_id,
        provider,
        changed_fields,
    );

    match &outcome {
        UpdateOutcome::Applied(_) | UpdateOutcome::Merged(_) => {
            let key = (soroban_sdk::symbol_short!("RX"), prescription.id);
            env.storage().persistent().set(&key, prescription);
            concurrency::save_field_snapshot(env, prescription.id, changed_fields);

            // Lineage: record a ModifiedBy edge for each successful mutation.
            lineage::add_edge(
                env,
                prescription.id,
                prescription.id,
                RelationshipKind::ModifiedBy,
                provider.clone(),
                None,
            );
        }
        UpdateOutcome::Conflicted(_) => {
            // Prescription is not updated — conflict must be resolved first.
        }
    }

    outcome
}

/// Retrieves the current OCC version stamp for a prescription.
pub fn get_prescription_version(env: &Env, id: u64) -> VersionStamp {
    concurrency::get_version_stamp(env, id)
}
