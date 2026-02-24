#![allow(clippy::arithmetic_side_effects)]
use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol, Vec};

pub const ERROR_LOG_KEY: Symbol = symbol_short!("ERR_LOG");
pub const ERROR_COUNT_KEY: Symbol = symbol_short!("ERR_CNT");
pub const MAX_ERROR_LOG_SIZE: u32 = 100;

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

/// Extends the time-to-live (TTL) for instance storage.
/// Instance storage TTL applies to all keys in the instance storage.
/// This ensures the data remains accessible for the extended period.
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

#[contracttype]
#[derive(Clone, Debug)]
pub struct ErrorContext {
    pub category: ErrorCategory,
    pub severity: ErrorSeverity,
    pub message: String,
    pub user: Option<Address>,
    pub resource_id: Option<String>,
    pub timestamp: u64,
    pub retryable: bool,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ErrorLogEntry {
    pub error_code: u32,
    pub context: ErrorContext,
    pub stack_trace: Option<String>,
}

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    UserNotFound = 4,
    RecordNotFound = 5,
    InvalidInput = 6,
    AccessDenied = 7,
    Paused = 8,
    ProviderNotFound = 9,
    ProviderAlreadyRegistered = 10,
    InvalidVerificationStatus = 11,
    InvalidAddress = 12,
    InvalidTimestamp = 13,
    StorageError = 14,
    RateLimitExceeded = 15,
    ExpiredAccess = 16,
    InvalidRole = 17,
    InvalidPermission = 18,
    DelegationExpired = 19,
    InvalidDataHash = 20,
    DuplicateRecord = 21,
    InvalidRecordType = 22,
    ContractPaused = 23,
    InsufficientPermissions = 24,
    TransientFailure = 25,
    MetaTxExpired = 26,
    NonceAlreadyUsed = 27,
}

impl ContractError {
    /// Returns the error category for this error.
    /// Categories help classify errors for better error handling and monitoring.
    pub fn category(&self) -> ErrorCategory {
        match self {
            ContractError::NotInitialized
            | ContractError::AlreadyInitialized
            | ContractError::InvalidInput
            | ContractError::InvalidAddress
            | ContractError::InvalidTimestamp
            | ContractError::InvalidRole
            | ContractError::InvalidPermission
            | ContractError::InvalidDataHash
            | ContractError::InvalidRecordType
            | ContractError::InvalidVerificationStatus
            | ContractError::MetaTxExpired => ErrorCategory::Validation,
            ContractError::Unauthorized
            | ContractError::AccessDenied
            | ContractError::InsufficientPermissions
            | ContractError::ExpiredAccess => ErrorCategory::Authorization,
            ContractError::UserNotFound
            | ContractError::RecordNotFound
            | ContractError::ProviderNotFound => ErrorCategory::NotFound,
            ContractError::ProviderAlreadyRegistered
            | ContractError::DuplicateRecord
            | ContractError::DelegationExpired
            | ContractError::NonceAlreadyUsed => ErrorCategory::StateConflict,
            ContractError::StorageError => ErrorCategory::Storage,
            ContractError::TransientFailure | ContractError::RateLimitExceeded => {
                ErrorCategory::Transient
            }
            ContractError::Paused | ContractError::ContractPaused => ErrorCategory::System,
        }
    }

    /// Returns the severity level for this error.
    /// Severity levels indicate the impact and urgency of the error.
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            ContractError::NotInitialized
            | ContractError::AlreadyInitialized
            | ContractError::InvalidInput
            | ContractError::InvalidAddress
            | ContractError::InvalidTimestamp
            | ContractError::InvalidRole
            | ContractError::InvalidPermission
            | ContractError::InvalidDataHash
            | ContractError::InvalidRecordType
            | ContractError::InvalidVerificationStatus
            | ContractError::UserNotFound
            | ContractError::RecordNotFound
            | ContractError::ProviderNotFound
            | ContractError::DuplicateRecord
            | ContractError::MetaTxExpired => ErrorSeverity::Low,
            ContractError::Unauthorized
            | ContractError::AccessDenied
            | ContractError::InsufficientPermissions
            | ContractError::ExpiredAccess
            | ContractError::ProviderAlreadyRegistered
            | ContractError::DelegationExpired
            | ContractError::RateLimitExceeded
            | ContractError::NonceAlreadyUsed => ErrorSeverity::Medium,
            ContractError::StorageError | ContractError::TransientFailure => ErrorSeverity::High,
            ContractError::Paused | ContractError::ContractPaused => ErrorSeverity::Critical,
        }
    }

    /// Returns whether this error is retryable.
    /// Retryable errors indicate transient failures that may succeed on retry.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            ContractError::TransientFailure
                | ContractError::RateLimitExceeded
                | ContractError::StorageError
        )
    }

    /// Returns a human-readable error message for this error.
    /// Messages provide context about what went wrong.
    pub fn message(&self) -> &'static str {
        match self {
            ContractError::NotInitialized => "Contract has not been initialized",
            ContractError::AlreadyInitialized => "Contract is already initialized",
            ContractError::Unauthorized => "Caller is not authorized for this operation",
            ContractError::UserNotFound => "User not found in the system",
            ContractError::RecordNotFound => "Record not found",
            ContractError::InvalidInput => "Invalid input parameters provided",
            ContractError::AccessDenied => "Access denied to the requested resource",
            ContractError::Paused => "Contract operations are currently paused",
            ContractError::ProviderNotFound => "Provider not found in the system",
            ContractError::ProviderAlreadyRegistered => "Provider is already registered",
            ContractError::InvalidVerificationStatus => "Invalid verification status provided",
            ContractError::InvalidAddress => "Invalid address format",
            ContractError::InvalidTimestamp => "Invalid timestamp value",
            ContractError::StorageError => "Storage operation failed",
            ContractError::RateLimitExceeded => "Rate limit exceeded, please retry later",
            ContractError::ExpiredAccess => "Access grant has expired",
            ContractError::InvalidRole => "Invalid role specified",
            ContractError::InvalidPermission => "Invalid permission specified",
            ContractError::DelegationExpired => "Role delegation has expired",
            ContractError::InvalidDataHash => "Invalid data hash format",
            ContractError::DuplicateRecord => "Record with this ID already exists",
            ContractError::InvalidRecordType => "Invalid record type specified",
            ContractError::ContractPaused => "Contract is paused",
            ContractError::InsufficientPermissions => "Insufficient permissions for operation",
            ContractError::TransientFailure => "Transient failure, operation may succeed on retry",
            ContractError::MetaTxExpired => "Meta-transaction has expired",
            ContractError::NonceAlreadyUsed => "Nonce has already been used",
        }
    }
}

/// Logs an error to the contract's error log.
/// Errors are stored with full context including category, severity, message, user, resource ID, and timestamp.
/// The error log is limited to the most recent 100 entries.
pub fn log_error(
    env: &Env,
    error: ContractError,
    user: Option<Address>,
    resource_id: Option<String>,
    stack_trace: Option<String>,
) {
    let context = ErrorContext {
        category: error.category(),
        severity: error.severity(),
        message: String::from_str(env, error.message()),
        user,
        resource_id,
        timestamp: env.ledger().timestamp(),
        retryable: error.retryable(),
    };

    let log_entry = ErrorLogEntry {
        error_code: error as u32,
        context,
        stack_trace,
    };

    let mut error_log: Vec<ErrorLogEntry> = env
        .storage()
        .instance()
        .get(&ERROR_LOG_KEY)
        .unwrap_or(Vec::new(env));

    error_log.push_back(log_entry);

    if error_log.len() > MAX_ERROR_LOG_SIZE {
        let mut new_log = Vec::new(env);
        for i in 1..error_log.len() {
            if let Some(entry) = error_log.get(i) {
                new_log.push_back(entry);
            }
        }
        error_log = new_log;
    }

    env.storage().instance().set(&ERROR_LOG_KEY, &error_log);

    let error_count: u64 = env.storage().instance().get(&ERROR_COUNT_KEY).unwrap_or(0);
    env.storage()
        .instance()
        .set(&ERROR_COUNT_KEY, &(error_count + 1));

    // Extend TTL for instance storage (applies to all instance keys)
    extend_ttl_instance(env);
}

/// Retrieves the complete error log containing all logged errors.
/// Returns an empty vector if no errors have been logged.
pub fn get_error_log(env: &Env) -> Vec<ErrorLogEntry> {
    env.storage()
        .instance()
        .get(&ERROR_LOG_KEY)
        .unwrap_or(Vec::new(env))
}

/// Returns the total count of errors that have been logged.
/// This count persists even if the error log is cleared.
pub fn get_error_count(env: &Env) -> u64 {
    env.storage().instance().get(&ERROR_COUNT_KEY).unwrap_or(0)
}

/// Clears the error log and resets the error count to zero.
/// This operation cannot be undone.
pub fn clear_error_log(env: &Env) {
    env.storage().instance().remove(&ERROR_LOG_KEY);
    env.storage().instance().set(&ERROR_COUNT_KEY, &0u64);
    // Extend TTL for instance storage (applies to all instance keys)
    extend_ttl_instance(env);
}

/// Creates an ErrorContext structure from an error and optional user/resource information.
/// The context includes automatically determined category, severity, message, and retryable flag.
pub fn create_error_context(
    env: &Env,
    error: ContractError,
    user: Option<Address>,
    resource_id: Option<String>,
) -> ErrorContext {
    ErrorContext {
        category: error.category(),
        severity: error.severity(),
        message: String::from_str(env, error.message()),
        user,
        resource_id,
        timestamp: env.ledger().timestamp(),
        retryable: error.retryable(),
    }
}

const RETRY_COUNT_KEY: Symbol = symbol_short!("RETRY_CNT");

/// Checks if an operation can be retried based on the current retry count.
/// Returns true if the operation can be retried, false if max retries have been reached.
/// Increments the retry count for the caller and operation combination.
pub fn retry_operation(env: &Env, caller: &Address, operation: &String, max_retries: u32) -> bool {
    let key = (RETRY_COUNT_KEY, caller.clone(), operation.clone());
    let count: u32 = env.storage().instance().get(&key).unwrap_or(0);

    if count >= max_retries {
        return false;
    }

    env.storage().instance().set(&key, &(count + 1));
    // Note: TTL extension for composite keys requires persistent storage
    // This function uses instance storage, so TTL is managed automatically
    true
}

/// Resets the retry count for a specific caller and operation.
/// This allows the operation to be retried from the beginning.
pub fn reset_retry_count(env: &Env, caller: &Address, operation: &String) {
    let key = (RETRY_COUNT_KEY, caller.clone(), operation.clone());
    env.storage().instance().remove(&key);
}
