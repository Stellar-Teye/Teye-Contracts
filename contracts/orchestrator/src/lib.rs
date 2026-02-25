#![no_std]

pub mod transaction;
pub mod rollback;
pub mod deadlock;
pub mod events;
pub mod errors;
pub mod validation;

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, String, Vec, Symbol};
use common::transaction::{
    TransactionLog, TransactionPhase, TransactionStatus, TransactionOperation, TransactionError,
    TransactionTimeoutConfig, generate_transaction_id, get_transaction_log, set_transaction_log,
    is_transaction_expired, get_default_timeout_config,
    TRANSACTION_COUNTER, TIMEOUT_CONFIG, ACTIVE_TRANSACTIONS, RESOURCE_LOCKS,
};

use transaction::TransactionManager;
use rollback::RollbackManager;
use deadlock::DeadlockDetector;
use events::EventPublisher;

/// Storage keys for the orchestrator contract
const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");

/// Main orchestrator contract for cross-contract atomic transactions
#[contract]
pub struct OrchestratorContract;

#[contractimpl]
impl OrchestratorContract {
    /// Initialize the orchestrator with admin address and timeout configuration
    pub fn initialize(env: Env, admin: Address, timeout_config: Option<TransactionTimeoutConfig>) -> Result<(), TransactionError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(TransactionError::TransactionExists);
        }

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);
        
        let config = timeout_config.unwrap_or_else(|| get_default_timeout_config(&env));
        env.storage().instance().set(&TIMEOUT_CONFIG, &config);
        
        // Initialize transaction counter
        env.storage().instance().set(&TRANSACTION_COUNTER, &0u64);
        
        Ok(())
    }

    /// Start a new orchestrated transaction with multiple operations
    pub fn start_transaction(
        env: Env,
        initiator: Address,
        operations: Vec<TransactionOperation>,
        timeout_seconds: Option<u64>,
        metadata: Vec<String>,
    ) -> Result<u64, TransactionError> {
        Self::require_initialized(&env)?;
        
        let transaction_id = generate_transaction_id(&env);
        let now = env.ledger().timestamp();
        
        // Get timeout configuration
        let config: TransactionTimeoutConfig = env.storage().instance().get(&TIMEOUT_CONFIG)
            .unwrap_or_else(|| get_default_timeout_config(&env));
        
        let timeout = timeout_seconds.unwrap_or(config.default_timeout);
        if timeout > config.max_timeout {
            return Err(TransactionError::InvalidInput);
        }

        // Check for potential deadlocks before starting
        let deadlock_detector = DeadlockDetector::new(&env);
        if deadlock_detector.would_cause_deadlock(&transaction_id, &operations) {
            return Err(TransactionError::DeadlockDetected);
        }

        // Create transaction log
        let mut log = TransactionLog {
            transaction_id,
            initiator: initiator.clone(),
            phase: TransactionPhase::Preparing,
            status: TransactionStatus::Active,
            operations: operations.clone(),
            created_at: now,
            updated_at: now,
            timeout_seconds: timeout,
            error: None,
            metadata,
        };

        // Store transaction log
        set_transaction_log(&env, &log);

        // Acquire resource locks
        Self::acquire_resource_locks(&env, &transaction_id, &operations)?;

        // Publish transaction started event
        EventPublisher::transaction_started(&env, &log);

        // Start two-phase commit
        let tx_manager = TransactionManager::new(&env);
        match tx_manager.prepare_phase(&mut log) {
            Ok(()) => {
                // All operations prepared successfully, commit them
                match tx_manager.commit_phase(&mut log) {
                    Ok(()) => {
                        log.phase = TransactionPhase::Committed;
                        log.status = TransactionStatus::Completed;
                        log.updated_at = env.ledger().timestamp();
                        
                        set_transaction_log(&env, &log);
                        Self::release_resource_locks(&env, transaction_id)?;
                        
                        EventPublisher::transaction_committed(&env, &log);
                        Ok(transaction_id)
                    }
                    Err(e) => {
                        // Commit failed, rollback
                        let rollback_manager = RollbackManager::new(&env);
                        if let Err(_rollback_err) = rollback_manager.rollback_transaction(&log) {
                            log.error = Some(String::from_str(&env, "Commit failed; rollback also failed"));
                        } else {
                            log.error = Some(String::from_str(&env, "Commit failed; rolled back"));
                        }
                        
                        log.phase = TransactionPhase::RolledBack;
                        log.status = TransactionStatus::Failed;
                        log.updated_at = env.ledger().timestamp();
                        
                        set_transaction_log(&env, &log);
                        Self::release_resource_locks(&env, transaction_id)?;
                        
                        EventPublisher::transaction_rolled_back(&env, &log);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                // Prepare failed, rollback
                let rollback_manager = RollbackManager::new(&env);
                if let Err(_rollback_err) = rollback_manager.rollback_transaction(&log) {
                    log.error = Some(String::from_str(&env, "Prepare failed; rollback also failed"));
                } else {
                    log.error = Some(String::from_str(&env, "Prepare failed; rolled back"));
                }
                
                log.phase = TransactionPhase::RolledBack;
                log.status = TransactionStatus::Failed;
                log.updated_at = env.ledger().timestamp();
                
                set_transaction_log(&env, &log);
                Self::release_resource_locks(&env, transaction_id)?;
                
                EventPublisher::transaction_rolled_back(&env, &log);
                Err(e)
            }
        }
    }

    /// Get transaction details by ID
    pub fn get_transaction(env: Env, transaction_id: u64) -> Result<TransactionLog, TransactionError> {
        Self::require_initialized(&env)?;
        
        get_transaction_log(&env, transaction_id)
            .ok_or(TransactionError::TransactionNotFound)
    }

    /// Get all active transactions
    pub fn get_active_transactions(env: Env) -> Result<Vec<u64>, TransactionError> {
        Self::require_initialized(&env)?;
        
        let active: Vec<u64> = env.storage().instance().get(&ACTIVE_TRANSACTIONS)
            .unwrap_or(Vec::new(&env));
        Ok(active)
    }

    /// Manually rollback a transaction (admin only)
    pub fn rollback_transaction(env: Env, admin: Address, transaction_id: u64) -> Result<(), TransactionError> {
        Self::require_admin(&env, &admin)?;
        Self::require_initialized(&env)?;
        
        let mut log = get_transaction_log(&env, transaction_id)
            .ok_or(TransactionError::TransactionNotFound)?;
        
        if log.phase == TransactionPhase::Committed {
            return Err(TransactionError::InvalidPhase);
        }

        let rollback_manager = RollbackManager::new(&env);
        rollback_manager.rollback_transaction(&log)?;
        
        log.phase = TransactionPhase::RolledBack;
        log.status = TransactionStatus::Cancelled;
        log.updated_at = env.ledger().timestamp();
        
        set_transaction_log(&env, &log);
        Self::release_resource_locks(&env, transaction_id)?;
        
        EventPublisher::transaction_rolled_back(&env, &log);
        Ok(())
    }

    /// Check and timeout expired transactions
    pub fn process_timeouts(env: Env) -> Result<Vec<u64>, TransactionError> {
        Self::require_initialized(&env)?;
        
        let active: Vec<u64> = env.storage().instance().get(&ACTIVE_TRANSACTIONS)
            .unwrap_or(Vec::new(&env));
        
        let mut timed_out = Vec::new(&env);
        let rollback_manager = RollbackManager::new(&env);
        
        for i in 0..active.len() {
            let transaction_id = active.get(i).unwrap();
            if let Some(log) = get_transaction_log(&env, transaction_id) {
                if is_transaction_expired(&env, &log) && log.phase != TransactionPhase::Committed {
                    if rollback_manager.rollback_transaction(&log).is_ok() {
                        let mut updated_log = log;
                        updated_log.phase = TransactionPhase::TimedOut;
                        updated_log.status = TransactionStatus::Failed;
                        updated_log.updated_at = env.ledger().timestamp();
                        updated_log.error = Some(String::from_str(&env, "Transaction timed out"));

                        set_transaction_log(&env, &updated_log);
                        let _ = Self::release_resource_locks(&env, transaction_id);

                        timed_out.push_back(transaction_id);
                        EventPublisher::transaction_timed_out(&env, &updated_log);
                    }
                }
            }
        }
        
        Ok(timed_out)
    }

    /// Update timeout configuration (admin only)
    pub fn update_timeout_config(env: Env, admin: Address, config: TransactionTimeoutConfig) -> Result<(), TransactionError> {
        Self::require_admin(&env, &admin)?;
        Self::require_initialized(&env)?;
        
        if config.default_timeout > config.max_timeout {
            return Err(TransactionError::InvalidInput);
        }
        
        env.storage().instance().set(&TIMEOUT_CONFIG, &config);
        Ok(())
    }

    /// Get current timeout configuration
    pub fn get_timeout_config(env: Env) -> Result<TransactionTimeoutConfig, TransactionError> {
        Self::require_initialized(&env)?;
        
        let config: TransactionTimeoutConfig = env.storage().instance().get(&TIMEOUT_CONFIG)
            .unwrap_or_else(|| get_default_timeout_config(&env));
        Ok(config)
    }

    // Helper functions
    
    fn require_initialized(env: &Env) -> Result<(), TransactionError> {
        if !env.storage().instance().has(&INITIALIZED) {
            Err(TransactionError::Unauthorized)
        } else {
            Ok(())
        }
    }

    fn require_admin(env: &Env, caller: &Address) -> Result<(), TransactionError> {
        let admin: Address = env.storage().instance().get(&ADMIN)
            .ok_or(TransactionError::Unauthorized)?;
        
        if admin == *caller {
            Ok(())
        } else {
            Err(TransactionError::Unauthorized)
        }
    }

    fn acquire_resource_locks(env: &Env, transaction_id: &u64, operations: &Vec<TransactionOperation>) -> Result<(), TransactionError> {
        let mut locks: Vec<(String, u64)> = env.storage().instance().get(&RESOURCE_LOCKS)
            .unwrap_or(Vec::new(env));

        for op_idx in 0..operations.len() {
            let operation = operations.get(op_idx).unwrap();
            for res_idx in 0..operation.locked_resources.len() {
                let resource = operation.locked_resources.get(res_idx).unwrap();
                // Check if resource is already locked
                for i in 0..locks.len() {
                    let (locked_resource, _locked_tx) = locks.get(i).unwrap();
                    if locked_resource == resource {
                        return Err(TransactionError::ResourceLocked);
                    }
                }
                // Acquire lock
                locks.push_back((resource.clone(), *transaction_id));
            }
        }

        env.storage().instance().set(&RESOURCE_LOCKS, &locks);
        Ok(())
    }

    fn release_resource_locks(env: &Env, transaction_id: u64) -> Result<(), TransactionError> {
        let locks: Vec<(String, u64)> = env.storage().instance().get(&RESOURCE_LOCKS)
            .unwrap_or(Vec::new(env));

        let mut new_locks: Vec<(String, u64)> = Vec::new(env);
        for i in 0..locks.len() {
            let (resource, locked_tx_id) = locks.get(i).unwrap();
            if locked_tx_id != transaction_id {
                new_locks.push_back((resource, locked_tx_id));
            }
        }

        env.storage().instance().set(&RESOURCE_LOCKS, &new_locks);
        Ok(())
    }
}

#[cfg(test)]
mod test_orchestrator;

#[cfg(test)]
mod test_gas_benchmarks;