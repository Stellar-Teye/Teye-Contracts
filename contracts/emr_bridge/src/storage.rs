use soroban_sdk::{Env, Symbol, Map, Vec, BytesN, String};

#[derive(Clone)]
pub struct Record {
    pub data: String,
    pub version: u32,
}

pub fn get_records(env: &Env) -> Map<BytesN<32>, Vec<Record>> {
    env.storage()
        .instance()
        .get(&Symbol::short("RECORDS"))
        .unwrap_or(Map::new(env))
}

pub fn set_records(env: &Env, records: Map<BytesN<32>, Vec<Record>>) {
    env.storage().instance().set(&Symbol::short("RECORDS"), &records);
}