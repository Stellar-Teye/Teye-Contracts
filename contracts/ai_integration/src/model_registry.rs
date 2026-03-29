use soroban_sdk::{Env, Symbol, Map};

#[derive(Clone)]
pub struct ModelInfo {
    pub approved: bool,
    pub deprecated: bool,
}

pub fn set_model(env: &Env, name: Symbol, version: u32, info: ModelInfo) {
    let mut registry: Map<(Symbol, u32), ModelInfo> =
        env.storage().instance().get(&Symbol::short("MODELS"))
        .unwrap_or(Map::new(env));

    registry.set((name, version), info);
    env.storage().instance().set(&Symbol::short("MODELS"), &registry);
}

pub fn get_model(env: &Env, name: Symbol, version: u32) -> Option<ModelInfo> {
    let registry: Map<(Symbol, u32), ModelInfo> =
        env.storage().instance().get(&Symbol::short("MODELS"))
        .unwrap_or(Map::new(env));

    registry.get((name, version))
}