use soroban_sdk::{Env, BytesN, Symbol, Vec};
use crate::queue::{Tx, get_queue, set_queue};

const DELAY: u64 = 30;

pub fn queue_tx(env: Env, id: BytesN<32>, delay: u64) {
    let mut queue = get_queue(&env);

    let execute_after = env.ledger().timestamp() + delay;

    queue.push_back(Tx {
        id,
        execute_after,
        priority: false,
    });

    set_queue(&env, queue);
}

pub fn execute_tx(env: Env, id: BytesN<32>) {
    let mut queue = get_queue(&env);
    let now = env.ledger().timestamp();

    let mut found = false;

    for tx in queue.iter() {
        if tx.id == id {
            if !tx.priority && now < tx.execute_after {
                panic!("Too early to execute");
            }
            found = true;
        }
    }

    if !found {
        panic!("Tx not found");
    }

    // Remove after execution
    let filtered: Vec<Tx> = queue.into_iter().filter(|t| t.id != id).collect();
    set_queue(&env, filtered);
}

pub fn prioritize_tx(env: Env, id: BytesN<32>) {
    let mut queue = get_queue(&env);

    for mut tx in queue.iter_mut() {
        if tx.id == id {
            tx.priority = true;
        }
    }

    set_queue(&env, queue);

    // Log event
    env.storage().instance().set(&Symbol::short("LAST_ACTION"), &"PRIORITIZED");
}