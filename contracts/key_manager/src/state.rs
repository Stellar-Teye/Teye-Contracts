use soroban_sdk::{Env, Symbol};

pub fn set_panic(env: &Env, value: bool) {
    env.storage().instance().set(&Symbol::short("PANIC"), &value);
}

pub fn is_panic(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&Symbol::short("PANIC"))
        .unwrap_or(false)
}

pub fn set_last_rotation(env: &Env, timestamp: u64) {
    env.storage().instance().set(&Symbol::short("LAST_ROT"), &timestamp);
}

pub fn get_last_rotation(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&Symbol::short("LAST_ROT"))
        .unwrap_or(0)
}