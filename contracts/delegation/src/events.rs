#![allow(deprecated)]

use soroban_sdk::{symbol_short, Address, Env};

pub fn publish_initialized(env: &Env, admin: Address) {
    env.events().publish((symbol_short!("INIT"),), admin);
}

pub fn publish_task_submitted(env: &Env, task_id: u64, creator: Address) {
    env.events()
        .publish((symbol_short!("TSK_SUB"), task_id), creator);
}

pub fn publish_executor_registered(env: &Env, executor: Address) {
    env.events().publish((symbol_short!("EX_REG"),), executor);
}

pub fn publish_task_assigned(env: &Env, task_id: u64, executor: Address) {
    env.events()
        .publish((symbol_short!("TSK_ASS"), task_id), executor);
}

pub fn publish_task_result_submitted(env: &Env, task_id: u64, executor: Address, success: bool) {
    env.events()
        .publish((symbol_short!("TSK_RES"), task_id, success), executor);
}
