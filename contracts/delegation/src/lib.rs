#![no_std]

pub mod events;
pub mod executor;
pub mod task_queue;
pub mod verification;

use crate::task_queue::TaskStatus;
use soroban_sdk::{contract, contractimpl, symbol_short, Address, BytesN, Env, Symbol};

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
        events::publish_initialized(&env, admin);
    }

    pub fn submit_task(
        env: Env,
        creator: Address,
        input_data: BytesN<32>,
        priority: u32,
        deadline: u64,
    ) -> u64 {
        creator.require_auth();
        let task_id =
            task_queue::create_task(&env, creator.clone(), input_data, priority, deadline);
        events::publish_task_submitted(&env, task_id, creator);
        task_id
    }

    pub fn register_executor(env: Env, executor: Address) {
        executor.require_auth();
        executor::register_executor(&env, executor.clone());
        events::publish_executor_registered(&env, executor);
    }

    pub fn assign_task(env: Env, executor: Address, task_id: u64) {
        executor.require_auth();

        // Enforce executor registration invariants before assignment.
        if executor::get_executor(&env, executor.clone()).is_none() {
            panic!("Executor not registered");
        }

        let mut task = task_queue::get_task(&env, task_id).expect("Task not found");
        if task.status != TaskStatus::Pending {
            panic!("Task not pending");
        }
        task.executor = Some(executor.clone());
        task.status = TaskStatus::Assigned;
        task_queue::update_task(&env, task);

        events::publish_task_assigned(&env, task_id, executor);
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
        if task.status != TaskStatus::Assigned {
            panic!("Task not in assigned state");
        }

        let is_valid = verification::verify_execution_proof(
            &env,
            task.input_data.clone(),
            result.clone(),
            proof.clone(),
        );
        if is_valid {
            task.result = Some(result);
            task.proof = Some(proof);
            task.status = TaskStatus::Completed;

            if let Some(mut info) = executor::get_executor(&env, executor.clone()) {
                info.tasks_completed += 1;
                info.reputation += 1;
                executor::update_executor(&env, info);
            }
        } else {
            task.status = TaskStatus::Failed;
            executor::slash_executor(&env, executor.clone(), 10);
        }
        task_queue::update_task(&env, task);

        events::publish_task_result_submitted(&env, task_id, executor, is_valid);
    }

    pub fn get_task(env: Env, task_id: u64) -> Option<task_queue::Task> {
        task_queue::get_task(&env, task_id)
    }

    pub fn get_executor_info(env: Env, executor: Address) -> Option<executor::ExecutorInfo> {
        executor::get_executor(&env, executor)
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&ADMIN)
    }
}

#[cfg(test)]
mod test;
