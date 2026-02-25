use soroban_sdk::{contracttype, Address, Env, Symbol, symbol_short};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutorInfo {
    pub address: Address,
    pub reputation: u32,
    pub tasks_completed: u64,
    pub last_active: u64,
}

const EXECUTORS: Symbol = symbol_short!("EXECS");
const MIN_STAKE: Symbol = symbol_short!("MIN_STK");

pub fn register_executor(env: &Env, executor: Address) {
    let info = ExecutorInfo {
        address: executor.clone(),
        reputation: 100, // Initial reputation
        tasks_completed: 0,
        last_active: env.ledger().timestamp(),
    };
    env.storage().persistent().set(&(EXECUTORS, executor), &info);
}

pub fn get_executor(env: &Env, executor: Address) -> Option<ExecutorInfo> {
    env.storage().persistent().get(&(EXECUTORS, executor))
}

pub fn update_executor(env: &Env, info: ExecutorInfo) {
    env.storage().persistent().set(&(EXECUTORS, info.address.clone()), &info);
}

pub fn set_min_stake(env: &Env, amount: i128) {
    env.storage().instance().set(&MIN_STAKE, &amount);
}

pub fn get_min_stake(env: &Env) -> i128 {
    env.storage().instance().get(&MIN_STAKE).unwrap_or(0)
}

pub fn slash_executor(env: &Env, executor: Address, amount_penalty: u32) {
    if let Some(mut info) = get_executor(env, executor.clone()) {
        info.reputation = info.reputation.saturating_sub(amount_penalty);
        update_executor(env, info);
    }
}
