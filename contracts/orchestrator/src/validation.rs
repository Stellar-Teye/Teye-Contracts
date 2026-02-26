use common::transaction::{
    TransactionError, TransactionOperation, TransactionPhase, TransactionTimeoutConfig,
};
use soroban_sdk::{String, Vec};

const MIN_TIMEOUT_SECONDS: u64 = 30;
const MAX_TIMEOUT_SECONDS: u64 = 86400 * 7; // 7 days
const MAX_OPERATIONS_PER_TRANSACTION: u32 = 50;
const MAX_METADATA_ITEMS: u32 = 20;
const MAX_PARAMETERS_PER_OPERATION: u32 = 10;

/// Validates timeout value
pub fn validate_timeout(timeout_seconds: u64) -> Result<(), TransactionError> {
    if !(MIN_TIMEOUT_SECONDS..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds) {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates operation count
pub fn validate_operation_count(count: u32) -> Result<(), TransactionError> {
    if count == 0 || count > MAX_OPERATIONS_PER_TRANSACTION {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates metadata size
pub fn validate_metadata(metadata: &Vec<String>) -> Result<(), TransactionError> {
    if metadata.len() > MAX_METADATA_ITEMS {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates operation parameters
pub fn validate_operation_parameters(parameters: &Vec<String>) -> Result<(), TransactionError> {
    if parameters.len() > MAX_PARAMETERS_PER_OPERATION {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates function name
pub fn validate_function_name(name: &String) -> Result<(), TransactionError> {
    let len = name.len();
    if len == 0 || len > 64 {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates resource identifier
pub fn validate_resource_id(resource_id: &String) -> Result<(), TransactionError> {
    let len = resource_id.len();
    if len == 0 || len > 128 {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates transaction metadata
pub fn validate_transaction_metadata(
    operations_count: u32,
    timeout_seconds: u64,
    metadata: &Vec<String>,
) -> Result<(), TransactionError> {
    validate_operation_count(operations_count)?;
    validate_timeout(timeout_seconds)?;
    validate_metadata(metadata)?;
    Ok(())
}

/// Validates transaction operation
pub fn validate_transaction_operation(
    operation_id: u64,
    function_name: &String,
    parameters: &Vec<String>,
    locked_resources: &Vec<String>,
) -> Result<(), TransactionError> {
    if operation_id == 0 {
        return Err(TransactionError::InvalidInput);
    }
    validate_function_name(function_name)?;
    validate_operation_parameters(parameters)?;

    for i in 0..locked_resources.len() {
        let resource = locked_resources.get(i).unwrap();
        validate_resource_id(&resource)?;
    }

    Ok(())
}

/// Validates transaction phase transition
pub fn validate_phase_transition(
    from_phase: &TransactionPhase,
    to_phase: &TransactionPhase,
) -> Result<(), TransactionError> {
    match (from_phase, to_phase) {
        (TransactionPhase::Preparing, TransactionPhase::Prepared)
        | (TransactionPhase::Prepared, TransactionPhase::Committed)
        | (TransactionPhase::Prepared, TransactionPhase::RolledBack)
        | (TransactionPhase::Preparing, TransactionPhase::RolledBack)
        | (TransactionPhase::Preparing, TransactionPhase::TimedOut)
        | (TransactionPhase::Prepared, TransactionPhase::TimedOut) => Ok(()),
        _ => Err(TransactionError::InvalidPhase),
    }
}

/// Validates rollback operation
pub fn validate_rollback_operation(
    prepared: bool,
    committed: bool,
) -> Result<(), TransactionError> {
    if !prepared || committed {
        Err(TransactionError::InvalidPhase)
    } else {
        Ok(())
    }
}

/// Validates deadlock detection parameters
pub fn validate_deadlock_detection(
    transaction_id: u64,
    operations: &Vec<TransactionOperation>,
) -> Result<(), TransactionError> {
    if transaction_id == 0 {
        return Err(TransactionError::InvalidInput);
    }
    if operations.is_empty() {
        return Err(TransactionError::InvalidInput);
    }

    for i in 0..operations.len() {
        let operation = operations.get(i).unwrap();
        for j in 0..operation.locked_resources.len() {
            let resource = operation.locked_resources.get(j).unwrap();
            validate_resource_id(&resource)?;
        }
    }

    Ok(())
}

/// Validates timeout configuration
pub fn validate_timeout_config(config: &TransactionTimeoutConfig) -> Result<(), TransactionError> {
    validate_timeout(config.default_timeout)?;
    if config.max_timeout < config.default_timeout {
        return Err(TransactionError::InvalidInput);
    }
    for i in 0..config.contract_timeouts.len() {
        let (_, timeout) = config.contract_timeouts.get(i).unwrap();
        validate_timeout(timeout)?;
    }
    Ok(())
}

/// Checks if a transaction is expired
pub fn is_transaction_expired_check(
    created_at: u64,
    timeout_seconds: u64,
    current_timestamp: u64,
) -> bool {
    let deadline = created_at.saturating_add(timeout_seconds);
    current_timestamp > deadline
}
