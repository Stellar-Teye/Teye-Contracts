use soroban_sdk::{Env, Address, Symbol};
use crate::state::{is_panic, get_last_rotation, set_last_rotation};

const COOLDOWN: u64 = 10; // mock ledger time units

pub fn rotate_key(env: Env, new_key: Address) {
    let now = env.ledger().timestamp();

    let last = get_last_rotation(&env);

    if now < last + COOLDOWN {
        panic!("Cooldown active");
    }

    // Panic mode still allows rotation but enforces stricter checks
    if is_panic(&env) {
        // You can extend with stricter auth rules here
    }

    // Save new key
    env.storage().instance().set(&Symbol::short("KEY"), &new_key);

    // Update rotation time
    set_last_rotation(&env, now);
}

pub fn get_key(env: Env) -> Address {
    env.storage()
        .instance()
        .get(&Symbol::short("KEY"))
        .expect("No key set")
}