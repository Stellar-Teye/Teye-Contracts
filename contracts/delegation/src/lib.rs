#![no_std]

pub mod task_queue;
pub mod executor;
pub mod verification;

use soroban_sdk::{contract, contractimpl, Address, Env, BytesN, symbol_short, Symbol};
use crate::task_queue::{TaskStatus};

#[contract]
pub struct DelegationContract;

const ADMIN: Symbol = symbol_short!("ADMIN");

#[contractimpl]
impl DelegationContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&ADMIN) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&ADMIN, &admin);
    }

    pub fn submit_task(
        env: Env,
        creator: Address,
        input_data: BytesN<32>,
        priority: u32,
        deadline: u64,
    ) -> u64 {
        creator.require_auth();
        task_queue::create_task(&env, creator, input_data, priority, deadline)
    }

    pub fn register_executor(env: Env, executor: Address) {
        executor.require_auth();
        executor::register_executor(&env, executor);
    }

    pub fn assign_task(env: Env, executor: Address, task_id: u64) {
        executor.require_auth();
        let mut task = task_queue::get_task(&env, task_id).expect("Task not found");
        if task.status != TaskStatus::Pending {
            panic!("Task not pending");
        }
        task.executor = Some(executor);
        task.status = TaskStatus::Assigned;
        task_queue::update_task(&env, task);
    }

    pub fn submit_result(
        env: Env,
        executor: Address,
        task_id: u64,
        result: BytesN<32>,
        proof: BytesN<32>,
    ) {
        executor.require_auth();
        let mut task = task_queue::get_task(&env, task_id).expect("Task not found");
        if task.executor != Some(executor.clone()) {
            panic!("Not assigned executor");
        }

        if verification::verify_execution_proof(&env, task.input_data.clone(), result.clone(), proof.clone()) {
            task.result = Some(result);
            task.proof = Some(proof);
            task.status = TaskStatus::Completed;
            
            if let Some(mut info) = executor::get_executor(&env, executor) {
                info.tasks_completed += 1;
                info.reputation += 1;
                executor::update_executor(&env, info);
            }
        } else {
            task.status = TaskStatus::Failed;
            executor::slash_executor(&env, executor, 10);
        }
        task_queue::update_task(&env, task);
    }
}
