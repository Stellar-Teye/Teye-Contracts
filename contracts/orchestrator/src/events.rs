#![allow(deprecated)] // events().publish migration to #[contractevent] tracked separately

use common::transaction::{ContractType, DeadlockInfo, TransactionLog, TransactionPhase};
use soroban_sdk::{symbol_short, Address, Env, String, Vec};

/// Event publisher for orchestrator events.
/// All symbol_short! values must be ≤9 characters.
pub struct EventPublisher;

#[allow(deprecated)]
impl EventPublisher {
    /// Publish transaction started event
    pub fn transaction_started(env: &Env, log: &TransactionLog) {
        env.events().publish(
            (symbol_short!("TX_START"), log.transaction_id),
            (log.initiator.clone(), log.created_at, log.timeout_seconds),
        );
    }

    /// Publish transaction prepared event
    pub fn transaction_prepared(env: &Env, log: &TransactionLog) {
        env.events().publish(
            (symbol_short!("TX_PREP"), log.transaction_id),
            (log.updated_at, log.operations.len()),
        );
    }

    /// Publish transaction committed event
    pub fn transaction_committed(env: &Env, log: &TransactionLog) {
        env.events().publish(
            (symbol_short!("TX_COMIT"), log.transaction_id),
            (log.updated_at, log.operations.len()),
        );
    }

    /// Publish transaction rolled back event
    pub fn transaction_rolled_back(env: &Env, log: &TransactionLog) {
        env.events().publish(
            (symbol_short!("TX_RBACK"), log.transaction_id),
            (log.updated_at, log.phase.clone(), log.error.clone()),
        );
    }

    /// Publish transaction timed out event
    pub fn transaction_timed_out(env: &Env, log: &TransactionLog) {
        env.events().publish(
            (symbol_short!("TX_TMOUT"), log.transaction_id),
            (log.updated_at, log.created_at + log.timeout_seconds),
        );
    }

    /// Publish operation prepared event
    pub fn operation_prepared(
        env: &Env,
        transaction_id: u64,
        operation_id: u64,
        contract_type: &ContractType,
    ) {
        env.events().publish(
            (symbol_short!("OP_PREP"), transaction_id, operation_id),
            (contract_type.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish operation committed event
    pub fn operation_committed(
        env: &Env,
        transaction_id: u64,
        operation_id: u64,
        contract_type: &ContractType,
    ) {
        env.events().publish(
            (symbol_short!("OP_COMIT"), transaction_id, operation_id),
            (contract_type.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish operation failed event
    pub fn operation_failed(
        env: &Env,
        transaction_id: u64,
        operation_id: u64,
        contract_type: &ContractType,
        error: &String,
    ) {
        env.events().publish(
            (symbol_short!("OP_FAIL"), transaction_id, operation_id),
            (
                contract_type.clone(),
                error.clone(),
                env.ledger().timestamp(),
            ),
        );
    }

    /// Publish operation rolled back event
    pub fn operation_rolled_back(
        env: &Env,
        transaction_id: u64,
        operation_id: u64,
        contract_type: &ContractType,
    ) {
        env.events().publish(
            (symbol_short!("OP_RBACK"), transaction_id, operation_id),
            (contract_type.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish rollback failed event
    pub fn rollback_failed(
        env: &Env,
        transaction_id: u64,
        operation_id: u64,
        contract_type: &ContractType,
        error: &String,
    ) {
        env.events().publish(
            (symbol_short!("RB_FAIL"), transaction_id, operation_id),
            (
                contract_type.clone(),
                error.clone(),
                env.ledger().timestamp(),
            ),
        );
    }

    /// Publish deadlock detected event
    pub fn deadlock_detected(env: &Env, deadlock_info: &DeadlockInfo) {
        env.events().publish(
            (symbol_short!("DEADLOCK"), deadlock_info.transaction_id),
            (
                deadlock_info.conflicting_transactions.clone(),
                deadlock_info.conflicting_resources.clone(),
                deadlock_info.detected_at,
            ),
        );
    }

    /// Publish resource locked event
    pub fn resource_locked(
        env: &Env,
        transaction_id: u64,
        resource: &String,
        contract_address: &Address,
    ) {
        env.events().publish(
            (symbol_short!("RES_LOCK"), transaction_id),
            (
                resource.clone(),
                contract_address.clone(),
                env.ledger().timestamp(),
            ),
        );
    }

    /// Publish resource unlocked event
    pub fn resource_unlocked(env: &Env, transaction_id: u64, resource: &String) {
        env.events().publish(
            (symbol_short!("RES_UNLK"), transaction_id),
            (resource.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish phase transition event
    pub fn phase_transition(
        env: &Env,
        transaction_id: u64,
        from_phase: &TransactionPhase,
        to_phase: &TransactionPhase,
    ) {
        env.events().publish(
            (symbol_short!("PH_TRANS"), transaction_id),
            (
                from_phase.clone(),
                to_phase.clone(),
                env.ledger().timestamp(),
            ),
        );
    }

    /// Publish performance metrics event
    pub fn performance_metrics(
        env: &Env,
        transaction_id: u64,
        prepare_time: u64,
        commit_time: u64,
        total_time: u64,
    ) {
        env.events().publish(
            (symbol_short!("PERF"), transaction_id),
            (prepare_time, commit_time, total_time),
        );
    }

    /// Publish gas consumption event
    pub fn gas_consumption(env: &Env, transaction_id: u64, operation_id: u64, gas_used: u64) {
        env.events().publish(
            (symbol_short!("GAS"), transaction_id, operation_id),
            (gas_used, env.ledger().timestamp()),
        );
    }

    /// Publish health check event
    pub fn health_check(
        env: &Env,
        active_transactions: u64,
        locked_resources: u64,
        pending_timeouts: u64,
    ) {
        env.events().publish(
            (symbol_short!("HEALTH"),),
            (
                active_transactions,
                locked_resources,
                pending_timeouts,
                env.ledger().timestamp(),
            ),
        );
    }

    /// Publish audit trail event
    pub fn audit_trail(
        env: &Env,
        transaction_id: u64,
        action: &String,
        actor: &Address,
        details: Vec<String>,
    ) {
        env.events().publish(
            (symbol_short!("AUDIT"), transaction_id, action.clone()),
            (actor.clone(), details, env.ledger().timestamp()),
        );
    }

    /// Publish security event
    pub fn security_event(env: &Env, event_type: &String, severity: &String, details: Vec<String>) {
        env.events().publish(
            (
                symbol_short!("SECURITY"),
                event_type.clone(),
                severity.clone(),
            ),
            (details, env.ledger().timestamp()),
        );
    }

    /// Publish monitoring event
    pub fn monitoring_event(
        env: &Env,
        metric_name: &String,
        metric_value: u64,
        threshold: Option<u64>,
    ) {
        env.events().publish(
            (symbol_short!("MONITOR"), metric_name.clone()),
            (metric_value, threshold, env.ledger().timestamp()),
        );
    }
}
