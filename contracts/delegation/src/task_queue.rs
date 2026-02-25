use soroban_sdk::{contracttype, Address, BytesN, Env, Symbol, Vec, symbol_short};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskStatus {
    Pending = 0,
    Assigned = 1,
    Completed = 2,
    Failed = 3,
    Slushed = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    pub id: u64,
    pub creator: Address,
    pub executor: Option<Address>,
    pub input_data: BytesN<32>, // Hash of input data or small input
    pub result: Option<BytesN<32>>, // Hash of result
    pub proof: Option<BytesN<32>>, // Hash of execution proof
    pub status: TaskStatus,
    pub priority: u32,
    pub deadline: u64,
}

const TASK_COUNTER: Symbol = symbol_short!("TASK_CTR");
const TASKS: Symbol = symbol_short!("TASKS");

pub fn next_task_id(env: &Env) -> u64 {
    let mut id: u64 = env.storage().instance().get(&TASK_COUNTER).unwrap_or(0);
    id += 1;
    env.storage().instance().set(&TASK_COUNTER, &id);
    id
}

pub fn create_task(
    env: &Env,
    creator: Address,
    input_data: BytesN<32>,
    priority: u32,
    deadline: u64,
) -> u64 {
    let id = next_task_id(env);
    let task = Task {
        id,
        creator,
        executor: None,
        input_data,
        result: None,
        proof: None,
        status: TaskStatus::Pending,
        priority,
        deadline,
    };
    env.storage().persistent().set(&(TASKS, id), &task);
    id
}

pub fn get_task(env: &Env, id: u64) -> Option<Task> {
    env.storage().persistent().get(&(TASKS, id))
}

pub fn update_task(env: &Env, task: Task) {
    env.storage().persistent().set(&(TASKS, task.id), &task);
}
