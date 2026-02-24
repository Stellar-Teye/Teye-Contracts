use soroban_sdk::{contracttype, Address, Env, String, Vec};

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

pub fn save_prescription(env: &Env, prescription: &Prescription) {
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
