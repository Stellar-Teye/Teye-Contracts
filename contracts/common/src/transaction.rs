use soroban_sdk::{contracttype, Address, Symbol, String, Vec, Env, symbol_short};

/// Transaction phases for two-phase commit protocol
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransactionPhase {
    /// Transaction is being prepared
    Preparing,
    /// All participants prepared, ready to commit
    Prepared,
    /// Transaction committed successfully
    Committed,
    /// Transaction rolled back due to failure
    RolledBack,
    /// Transaction timed out
    TimedOut,
}

/// Transaction status with additional context
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransactionStatus {
    /// Transaction is active and in progress
    Active,
    /// Transaction completed successfully
    Completed,
    /// Transaction failed
    Failed,
    /// Transaction was cancelled
    Cancelled,
}

/// Types of contracts that can participate in orchestrated transactions
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractType {
    VisionRecords,
    Identity,
    Staking,
    ZkVerifier,
    Treasury,
    Compliance,
}

/// Individual operation within a transaction
#[contracttype]
#[derive(Clone, Debug)]
pub struct TransactionOperation {
    /// Unique identifier for this operation
    pub operation_id: u64,
    /// Contract type this operation targets
    pub contract_type: ContractType,
    /// Contract address
    pub contract_address: Address,
    /// Function name to call
    pub function_name: String,
    /// Serialized function parameters
    pub parameters: Vec<String>,
    /// Resources this operation locks (for deadlock detection)
    pub locked_resources: Vec<String>,
    /// Whether this operation is prepared
    pub prepared: bool,
    /// Whether this operation is committed
    pub committed: bool,
    /// Error message if operation failed
    pub error: Option<String>,
}

/// Transaction log entry for tracking orchestrated transactions
#[contracttype]
#[derive(Clone, Debug)]
pub struct TransactionLog {
    /// Unique transaction identifier
    pub transaction_id: u64,
    /// Transaction initiator
    pub initiator: Address,
    /// Current phase of the transaction
    pub phase: TransactionPhase,
    /// Overall transaction status
    pub status: TransactionStatus,
    /// List of operations in this transaction
    pub operations: Vec<TransactionOperation>,
    /// Timestamp when transaction was created
    pub created_at: u64,
    /// Timestamp when transaction was last updated
    pub updated_at: u64,
    /// Timeout for this transaction (seconds since creation)
    pub timeout_seconds: u64,
    /// Error message if transaction failed
    pub error: Option<String>,
    /// Metadata for transaction
    pub metadata: Vec<String>,
}

/// Deadlock detection information
#[contracttype]
#[derive(Clone, Debug)]
pub struct DeadlockInfo {
    /// Transaction ID that detected the deadlock
    pub transaction_id: u64,
    /// List of conflicting transactions
    pub conflicting_transactions: Vec<u64>,
    /// Resources causing the deadlock
    pub conflicting_resources: Vec<String>,
    /// Timestamp when deadlock was detected
    pub detected_at: u64,
}

/// Rollback information for failed operations
#[contracttype]
#[derive(Clone, Debug)]
pub struct RollbackInfo {
    /// Transaction ID being rolled back
    pub transaction_id: u64,
    /// Operation ID being rolled back
    pub operation_id: u64,
    /// Contract address for rollback
    pub contract_address: Address,
    /// Rollback function name
    pub rollback_function: String,
    /// Rollback parameters
    pub rollback_parameters: Vec<String>,
    /// Whether rollback was successful
    pub rollback_successful: bool,
    /// Rollback error if any
    pub rollback_error: Option<String>,
}

/// Configuration for transaction timeouts
#[contracttype]
#[derive(Clone, Debug)]
pub struct TransactionTimeoutConfig {
    /// Default timeout for all transactions (seconds)
    pub default_timeout: u64,
    /// Maximum allowed timeout (seconds)
    pub max_timeout: u64,
    /// Timeout for specific contract types
    pub contract_timeouts: Vec<(ContractType, u64)>,
}

/// Event data for transaction phase transitions
#[contracttype]
#[derive(Clone, Debug)]
pub struct TransactionPhaseEvent {
    pub transaction_id: u64,
    pub phase: TransactionPhase,
    pub timestamp: u64,
    pub contract_address: Option<Address>,
}

/// Event data for operation status changes
#[contracttype]
#[derive(Clone, Debug)]
pub struct OperationStatusEvent {
    pub transaction_id: u64,
    pub operation_id: u64,
    pub contract_type: ContractType,
    pub status: String,
    pub timestamp: u64,
}

/// Event data for deadlock detection
#[contracttype]
#[derive(Clone, Debug)]
pub struct DeadlockEvent {
    pub transaction_id: u64,
    pub conflicting_transactions: Vec<u64>,
    pub conflicting_resources: Vec<String>,
    pub timestamp: u64,
}

/// Storage keys for transaction management
pub const TRANSACTION_LOG: Symbol = symbol_short!("TX_LOG");
pub const TRANSACTION_COUNTER: Symbol = symbol_short!("TX_CTR");
pub const DEADLOCK_TRACKER: Symbol = symbol_short!("DL_TRK");
pub const TIMEOUT_CONFIG: Symbol = symbol_short!("TO_CFG");
pub const ACTIVE_TRANSACTIONS: Symbol = symbol_short!("ACT_TX");
pub const RESOURCE_LOCKS: Symbol = symbol_short!("RES_LK");

/// Default timeout values (in seconds)
pub const DEFAULT_TRANSACTION_TIMEOUT: u64 = 300; // 5 minutes
pub const MAX_TRANSACTION_TIMEOUT: u64 = 3600; // 1 hour
pub const DEADLOCK_CHECK_INTERVAL: u64 = 30; // 30 seconds

/// Error codes for transaction operations
#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TransactionError {
    /// Transaction not found
    TransactionNotFound = 1001,
    /// Invalid transaction phase
    InvalidPhase = 1002,
    /// Transaction already exists
    TransactionExists = 1003,
    /// Operation not found
    OperationNotFound = 1004,
    /// Insufficient permissions
    Unauthorized = 1005,
    /// Transaction timed out
    TransactionTimeout = 1006,
    /// Deadlock detected
    DeadlockDetected = 1007,
    /// Rollback failed
    RollbackFailed = 1008,
    /// Invalid input parameters
    InvalidInput = 1009,
    /// Contract call failed
    ContractCallFailed = 1010,
    /// Resource already locked
    ResourceLocked = 1011,
}

/// Helper functions for transaction management
pub fn generate_transaction_id(env: &Env) -> u64 {
    let counter: u64 = env.storage().instance().get(&TRANSACTION_COUNTER).unwrap_or(0);
    let new_id = counter + 1;
    env.storage().instance().set(&TRANSACTION_COUNTER, &new_id);
    new_id
}

pub fn is_transaction_expired(env: &Env, log: &TransactionLog) -> bool {
    let now = env.ledger().timestamp();
    let deadline = log.created_at + log.timeout_seconds;
    now > deadline
}

pub fn get_transaction_log(env: &Env, transaction_id: u64) -> Option<TransactionLog> {
    let key = (TRANSACTION_LOG, transaction_id);
    env.storage().persistent().get(&key)
}

pub fn set_transaction_log(env: &Env, log: &TransactionLog) {
    let key = (TRANSACTION_LOG, log.transaction_id);
    env.storage().persistent().set(&key, log);
    
    // Update the active transactions list
    let mut active: Vec<u64> = env.storage().instance().get(&ACTIVE_TRANSACTIONS).unwrap_or(Vec::new(env));
    if !active.contains(&log.transaction_id) {
        active.push_back(log.transaction_id);
        env.storage().instance().set(&ACTIVE_TRANSACTIONS, &active);
    }
}

pub fn remove_transaction_log(env: &Env, transaction_id: u64) {
    let key = (TRANSACTION_LOG, transaction_id);
    env.storage().persistent().remove(&key);
    
    // Remove from active transactions list
    let active: Vec<u64> = env.storage().instance().get(&ACTIVE_TRANSACTIONS).unwrap_or(Vec::new(env));
    let mut new_active = Vec::new(env);
    for i in 0..active.len() {
        if let Some(id) = active.get(i) {
            if id != transaction_id {
                new_active.push_back(id);
            }
        }
    }
    env.storage().instance().set(&ACTIVE_TRANSACTIONS, &new_active);
}

pub fn get_default_timeout_config(env: &Env) -> TransactionTimeoutConfig {
    TransactionTimeoutConfig {
        default_timeout: DEFAULT_TRANSACTION_TIMEOUT,
        max_timeout: MAX_TRANSACTION_TIMEOUT,
        contract_timeouts: Vec::new(env),
    }
}
