#![allow(clippy::arithmetic_side_effects)]

use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol, Vec};

pub const ERROR_LOG_KEY: Symbol = symbol_short!("ERR_LOG");
pub const ERROR_COUNT_KEY: Symbol = symbol_short!("ERR_CNT");
pub const MAX_ERROR_LOG_SIZE: u32 = 100;

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

/// Extends the time-to-live (TTL) for instance storage.
fn extend_ttl_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Error categories for classifying different types of errors
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ErrorCategory {
    /// Validation errors: invalid input parameters or format errors
    Validation = 1,
    /// Authorization errors: permission and access control failures
    Authorization = 2,
    /// Not found errors: resource lookup failures
    NotFound = 3,
    /// State conflict errors: duplicate registrations, expired delegations
    StateConflict = 4,
    /// Storage errors: storage operation failures
    Storage = 5,
    /// Transient errors: temporary failures that may succeed on retry
    Transient = 6,
    /// System errors: contract-level issues like pausing
    System = 7,
}

/// Error severity levels indicating the impact and urgency of errors
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ErrorSeverity {
    /// Low severity: non-critical errors, informational
    Low = 1,
    /// Medium severity: important but recoverable errors
    Medium = 2,
    /// High severity: significant errors requiring attention
    High = 3,
    /// Critical severity: system-level failures requiring immediate action
    Critical = 4,
}

/// Error context information for debugging and auditing
#[contracttype]
#[derive(Clone, Debug)]
pub struct ErrorContext {
    pub error_code: u32,
    pub category: ErrorCategory,
    pub severity: ErrorSeverity,
    pub message: String,
    pub user_address: Option<Address>,
    pub resource_id: Option<String>,
    pub timestamp: u64,
    pub contract_function: Option<String>,
}

/// Error log entry for tracking errors over time
#[contracttype]
#[derive(Clone, Debug)]
pub struct ErrorLogEntry {
    pub context: ErrorContext,
    pub stack_trace: Vec<String>,
    pub recovery_attempted: bool,
    pub recovery_successful: bool,
}

/// Orchestrator-specific error codes
#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum OrchestratorError {
    // System errors (1000-1099)
    NotInitialized = 1000,
    AlreadyInitialized = 1001,
    Unauthorized = 1002,
    Paused = 1003,
    
    // Transaction errors (1100-1199)
    TransactionNotFound = 1100,
    TransactionExists = 1101,
    InvalidPhase = 1102,
    TransactionTimeout = 1103,
    DeadlockDetected = 1104,
    RollbackFailed = 1105,
    
    // Operation errors (1200-1299)
    OperationNotFound = 1200,
    InvalidInput = 1201,
    ContractCallFailed = 1202,
    ResourceLocked = 1203,
    
    // Validation errors (1300-1399)
    ValidationError = 1300,
    InvalidAddress = 1301,
    InvalidTimeout = 1302,
    InvalidOperation = 1303,
    
    // Storage errors (1400-1499)
    StorageError = 1400,
    SerializationError = 1401,
    DeserializationError = 1402,
}

impl From<OrchestratorError> for ErrorCategory {
    fn from(error: OrchestratorError) -> Self {
        match error {
            OrchestratorError::NotInitialized |
            OrchestratorError::AlreadyInitialized |
            OrchestratorError::Paused => ErrorCategory::System,
            
            OrchestratorError::Unauthorized => ErrorCategory::Authorization,
            
            OrchestratorError::TransactionNotFound |
            OrchestratorError::TransactionExists |
            OrchestratorError::InvalidPhase |
            OrchestratorError::TransactionTimeout |
            OrchestratorError::DeadlockDetected |
            OrchestratorError::RollbackFailed => ErrorCategory::StateConflict,
            
            OrchestratorError::OperationNotFound |
            OrchestratorError::ContractCallFailed |
            OrchestratorError::ResourceLocked => ErrorCategory::StateConflict,
            
            OrchestratorError::InvalidInput |
            OrchestratorError::InvalidAddress |
            OrchestratorError::InvalidTimeout |
            OrchestratorError::InvalidOperation |
            OrchestratorError::ValidationError => ErrorCategory::Validation,
            
            OrchestratorError::StorageError |
            OrchestratorError::SerializationError |
            OrchestratorError::DeserializationError => ErrorCategory::Storage,
        }
    }
}

impl From<OrchestratorError> for ErrorSeverity {
    fn from(error: OrchestratorError) -> Self {
        match error {
            OrchestratorError::NotInitialized |
            OrchestratorError::AlreadyInitialized |
            OrchestratorError::Unauthorized |
            OrchestratorError::InvalidInput |
            OrchestratorError::InvalidAddress |
            OrchestratorError::InvalidTimeout |
            OrchestratorError::InvalidOperation => ErrorSeverity::Medium,
            
            OrchestratorError::TransactionNotFound |
            OrchestratorError::TransactionExists |
            OrchestratorError::OperationNotFound |
            OrchestratorError::ResourceLocked |
            OrchestratorError::ValidationError |
            OrchestratorError::StorageError |
            OrchestratorError::SerializationError |
            OrchestratorError::DeserializationError => ErrorSeverity::Low,
            
            OrchestratorError::InvalidPhase |
            OrchestratorError::ContractCallFailed |
            OrchestratorError::Paused => ErrorSeverity::High,
            
            OrchestratorError::TransactionTimeout |
            OrchestratorError::DeadlockDetected |
            OrchestratorError::RollbackFailed => ErrorSeverity::Critical,
        }
    }
}

/// Convert an OrchestratorError to a human-readable Soroban String
fn error_to_string(env: &Env, error: OrchestratorError) -> String {
    match error {
        OrchestratorError::NotInitialized => String::from_str(env, "NotInitialized"),
        OrchestratorError::AlreadyInitialized => String::from_str(env, "AlreadyInitialized"),
        OrchestratorError::Unauthorized => String::from_str(env, "Unauthorized"),
        OrchestratorError::Paused => String::from_str(env, "Paused"),
        OrchestratorError::TransactionNotFound => String::from_str(env, "TransactionNotFound"),
        OrchestratorError::TransactionExists => String::from_str(env, "TransactionExists"),
        OrchestratorError::InvalidPhase => String::from_str(env, "InvalidPhase"),
        OrchestratorError::TransactionTimeout => String::from_str(env, "TransactionTimeout"),
        OrchestratorError::DeadlockDetected => String::from_str(env, "DeadlockDetected"),
        OrchestratorError::RollbackFailed => String::from_str(env, "RollbackFailed"),
        OrchestratorError::OperationNotFound => String::from_str(env, "OperationNotFound"),
        OrchestratorError::InvalidInput => String::from_str(env, "InvalidInput"),
        OrchestratorError::ContractCallFailed => String::from_str(env, "ContractCallFailed"),
        OrchestratorError::ResourceLocked => String::from_str(env, "ResourceLocked"),
        OrchestratorError::ValidationError => String::from_str(env, "ValidationError"),
        OrchestratorError::InvalidAddress => String::from_str(env, "InvalidAddress"),
        OrchestratorError::InvalidTimeout => String::from_str(env, "InvalidTimeout"),
        OrchestratorError::InvalidOperation => String::from_str(env, "InvalidOperation"),
        OrchestratorError::StorageError => String::from_str(env, "StorageError"),
        OrchestratorError::SerializationError => String::from_str(env, "SerializationError"),
        OrchestratorError::DeserializationError => String::from_str(env, "DeserializationError"),
    }
}

/// Creates an error context for logging and debugging
pub fn create_error_context(
    env: &Env,
    error: OrchestratorError,
    user_address: Option<Address>,
    resource_id: Option<String>,
) -> ErrorContext {
    ErrorContext {
        error_code: error as u32,
        category: error.into(),
        severity: error.into(),
        message: error_to_string(env, error),
        user_address,
        resource_id,
        timestamp: env.ledger().timestamp(),
        contract_function: None,
    }
}

/// Logs an error to persistent storage for audit and debugging
pub fn log_error(
    env: &Env,
    error: OrchestratorError,
    user_address: Option<Address>,
    resource_id: Option<String>,
    contract_function: Option<String>,
) {
    let context = create_error_context(env, error, user_address, resource_id.clone());
    
    let mut final_context = context;
    if let Some(func) = contract_function {
        final_context.contract_function = Some(func);
    }
    
    let log_entry = ErrorLogEntry {
        context: final_context,
        stack_trace: Vec::new(env),
        recovery_attempted: false,
        recovery_successful: false,
    };
    
    // Store error log entry
    let error_count: u32 = env.storage().instance().get(&ERROR_COUNT_KEY).unwrap_or(0);
    let new_count = error_count.saturating_add(1);
    env.storage().instance().set(&ERROR_COUNT_KEY, &new_count);
    
    let error_key = (ERROR_LOG_KEY, new_count);
    env.storage().persistent().set(&error_key, &log_entry);
    
    // Extend TTL for error storage
    env.storage()
        .persistent()
        .extend_ttl(&error_key, TTL_THRESHOLD, TTL_EXTEND_TO);
    
    extend_ttl_instance(env);
    
    // Clean up old error logs if we exceed the maximum
    if new_count > MAX_ERROR_LOG_SIZE {
        let oldest_key = (ERROR_LOG_KEY, new_count.saturating_sub(MAX_ERROR_LOG_SIZE));
        env.storage().persistent().remove(&oldest_key);
    }
}

/// Gets the total number of logged errors
pub fn get_error_count(env: &Env) -> u32 {
    env.storage().instance().get(&ERROR_COUNT_KEY).unwrap_or(0)
}

/// Gets recent error log entries for debugging
pub fn get_recent_errors(env: &Env, count: u32) -> Vec<ErrorLogEntry> {
    let total_count = get_error_count(env);
    let start_index = if total_count > count {
        total_count.saturating_sub(count)
    } else {
        1
    };
    
    let mut errors = Vec::new(env);
    for i in start_index..=total_count {
        let error_key = (ERROR_LOG_KEY, i);
        if let Some(entry) = env.storage().persistent().get::<_, ErrorLogEntry>(&error_key) {
            errors.push_back(entry);
        }
    }
    
    errors
}

/// Clears all error logs (admin only)
pub fn clear_error_logs(env: &Env) {
    let total_count = get_error_count(env);
    for i in 1..=total_count {
        let error_key = (ERROR_LOG_KEY, i);
        env.storage().persistent().remove(&error_key);
    }
    env.storage().instance().remove(&ERROR_COUNT_KEY);
    extend_ttl_instance(env);
}
