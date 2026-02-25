use soroban_sdk::{Env, Vec, String, Symbol, Val, IntoVal};
use common::transaction::{
    TransactionLog, TransactionOperation, TransactionError, RollbackInfo,
    get_transaction_log,
};

use super::events::EventPublisher;

/// Rollback manager for handling transaction rollback operations
pub struct RollbackManager<'a> {
    env: &'a Env,
}

impl<'a> RollbackManager<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }

    /// Rollback an entire transaction in reverse order (LIFO)
    pub fn rollback_transaction(&self, log: &TransactionLog) -> Result<(), TransactionError> {
        let mut rollback_failed = false;

        // Rollback operations in reverse order (LIFO principle)
        for i in (0..log.operations.len()).rev() {
            let operation = log.operations.get(i).unwrap();

            // Only rollback operations that were prepared but not committed
            if operation.prepared && !operation.committed {
                match self.rollback_operation(&operation) {
                    Ok(_) => {
                        EventPublisher::operation_rolled_back(
                            self.env, log.transaction_id, operation.operation_id, &operation.contract_type,
                        );
                    }
                    Err(_) => {
                        rollback_failed = true;
                        EventPublisher::rollback_failed(
                            self.env, log.transaction_id, operation.operation_id,
                            &operation.contract_type,
                            &String::from_str(self.env, "Rollback failed"),
                        );
                    }
                }
            }
        }

        if rollback_failed {
            Err(TransactionError::RollbackFailed)
        } else {
            Ok(())
        }
    }

    /// Rollback a single operation via cross-contract invocation
    pub fn rollback_operation(&self, operation: &TransactionOperation) -> Result<RollbackInfo, TransactionError> {
        let func_sym = Symbol::new(self.env, "rollback_");

        let mut rollback_info = RollbackInfo {
            transaction_id: 0,
            operation_id: operation.operation_id,
            contract_address: operation.contract_address.clone(),
            rollback_function: String::from_str(self.env, "rollback_"),
            rollback_parameters: operation.parameters.clone(),
            rollback_successful: false,
            rollback_error: None,
        };

        let mut args: Vec<Val> = Vec::new(self.env);
        for i in 0..operation.parameters.len() {
            let param = operation.parameters.get(i).unwrap();
            args.push_back(param.into_val(self.env));
        }

        // Invoke rollback — panics on failure which Soroban runtime catches
        let _result: Val = self.env.invoke_contract(
            &operation.contract_address,
            &func_sym,
            args,
        );

        rollback_info.rollback_successful = true;
        Ok(rollback_info)
    }

    /// Check if an operation can be rolled back
    pub fn can_rollback(&self, operation: &TransactionOperation) -> bool {
        operation.prepared && !operation.committed
    }

    /// Get rollback status for a transaction
    pub fn get_rollback_status(&self, transaction_id: u64) -> Result<Vec<RollbackInfo>, TransactionError> {
        let log = get_transaction_log(self.env, transaction_id)
            .ok_or(TransactionError::TransactionNotFound)?;

        let mut rollback_status: Vec<RollbackInfo> = Vec::new(self.env);

        for i in 0..log.operations.len() {
            let operation = log.operations.get(i).unwrap();
            let rollback_info = RollbackInfo {
                transaction_id,
                operation_id: operation.operation_id,
                contract_address: operation.contract_address.clone(),
                rollback_function: String::from_str(self.env, "rollback_"),
                rollback_parameters: operation.parameters.clone(),
                rollback_successful: false,
                rollback_error: None,
            };
            rollback_status.push_back(rollback_info);
        }

        Ok(rollback_status)
    }

    /// Perform a partial rollback of specific operations
    pub fn partial_rollback(&self, transaction_id: u64, operation_ids: Vec<u64>) -> Result<(), TransactionError> {
        let log = get_transaction_log(self.env, transaction_id)
            .ok_or(TransactionError::TransactionNotFound)?;

        for k in 0..operation_ids.len() {
            let operation_id = operation_ids.get(k).unwrap();
            let mut found = false;

            for i in 0..log.operations.len() {
                let operation = log.operations.get(i).unwrap();
                if operation.operation_id == operation_id {
                    found = true;

                    if !self.can_rollback(&operation) {
                        return Err(TransactionError::InvalidPhase);
                    }

                    match self.rollback_operation(&operation) {
                        Ok(_) => {
                            EventPublisher::operation_rolled_back(
                                self.env, transaction_id, operation_id, &operation.contract_type,
                            );
                        }
                        Err(_) => {
                            EventPublisher::rollback_failed(
                                self.env, transaction_id, operation_id,
                                &operation.contract_type,
                                &String::from_str(self.env, "Partial rollback failed"),
                            );
                            return Err(TransactionError::RollbackFailed);
                        }
                    }
                    break;
                }
            }

            if !found {
                return Err(TransactionError::OperationNotFound);
            }
        }

        Ok(())
    }
}
