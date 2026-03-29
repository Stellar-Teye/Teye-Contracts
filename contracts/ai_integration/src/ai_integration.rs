use soroban_sdk::{Env, Symbol, String, Vec};
use crate::model_registry::{get_model};

#[derive(Clone)]
pub struct InferenceRecord {
    pub model_name: Symbol,
    pub model_version: u32,
    pub result: String,
}

pub fn commit_inference(
    env: Env,
    model_name: Symbol,
    version: u32,
    result: String,
) {
    let model = get_model(&env, model_name.clone(), version)
        .expect("Model not found");

    if !model.approved {
        panic!("Model not approved");
    }

    if model.deprecated {
        panic!("Model deprecated or insecure");
    }

    let mut logs: Vec<InferenceRecord> =
        env.storage().instance().get(&Symbol::short("LOGS"))
        .unwrap_or(Vec::new(&env));

    logs.push_back(InferenceRecord {
        model_name,
        model_version: version,
        result,
    });

    env.storage().instance().set(&Symbol::short("LOGS"), &logs);
}