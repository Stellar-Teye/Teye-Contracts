use soroban_sdk::{Env, Symbol, Vec, BytesN};

#[derive(Clone)]
pub struct Tx {
    pub id: BytesN<32>,
    pub execute_after: u64,
    pub priority: bool,
}

pub fn get_queue(env: &Env) -> Vec<Tx> {
    env.storage()
        .instance()
        .get(&Symbol::short("QUEUE"))
        .unwrap_or(Vec::new(env))
}

pub fn set_queue(env: &Env, queue: Vec<Tx>) {
    env.storage().instance().set(&Symbol::short("QUEUE"), &queue);
}