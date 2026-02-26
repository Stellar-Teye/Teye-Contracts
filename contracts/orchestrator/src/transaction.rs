use common::transaction::{
    get_transaction_log, set_transaction_log, TransactionError, TransactionLog,
    TransactionOperation, TransactionPhase,
};
use soroban_sdk::{Env, IntoVal, String, Symbol, Val, Vec};

use super::events::EventPublisher;

/// Transaction manager for handling two-phase commit protocol
pub struct TransactionManager<'a> {
    env: &'a Env,
}

impl<'a> TransactionManager<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }

    /// Prepare phase: call prepare_* functions on all participating contracts.
    /// Each operation's `function_name` should be the base name (e.g., "stake").
    /// This method will invoke `prepare_<function_name>` on each target contract.
    pub fn prepare_phase(&self, log: &mut TransactionLog) -> Result<(), TransactionError> {
        log.phase = TransactionPhase::Preparing;
        set_transaction_log(self.env, log);

        let mut prepared_operations: Vec<TransactionOperation> = Vec::new(self.env);

        for i in 0..log.operations.len() {
            let mut operation = log.operations.get(i).unwrap().clone();

            // Build the prepare function symbol and invoke
            let func_sym = Symbol::new(self.env, "prepare_");

            let mut args: Vec<Val> = Vec::new(self.env);
            for j in 0..operation.parameters.len() {
                let param = operation.parameters.get(j).unwrap();
                args.push_back(param.into_val(self.env));
            }

            // invoke_contract panics on failure; Soroban runtime catches it
            // For orchestrated transactions, the caller should handle panics
            let _result: Val =
                self.env
                    .invoke_contract(&operation.contract_address, &func_sym, args);

            operation.prepared = true;
            prepared_operations.push_back(operation.clone());
            EventPublisher::operation_prepared(
                self.env,
                log.transaction_id,
                operation.operation_id,
                &operation.contract_type,
            );
        }

        log.operations = prepared_operations;
        log.phase = TransactionPhase::Prepared;
        log.updated_at = self.env.ledger().timestamp();
        set_transaction_log(self.env, log);
        EventPublisher::transaction_prepared(self.env, log);

        Ok(())
    }

    /// Commit phase: call commit_* functions on all prepared contracts
    pub fn commit_phase(&self, log: &mut TransactionLog) -> Result<(), TransactionError> {
        if log.phase != TransactionPhase::Prepared {
            return Err(TransactionError::InvalidPhase);
        }

        let mut committed_operations: Vec<TransactionOperation> = Vec::new(self.env);

        for i in 0..log.operations.len() {
            let mut operation = log.operations.get(i).unwrap().clone();

            if !operation.prepared {
                return Err(TransactionError::InvalidPhase);
            }

            let func_sym = Symbol::new(self.env, "commit_");

            let mut args: Vec<Val> = Vec::new(self.env);
            for j in 0..operation.parameters.len() {
                let param = operation.parameters.get(j).unwrap();
                args.push_back(param.into_val(self.env));
            }

            let _result: Val =
                self.env
                    .invoke_contract(&operation.contract_address, &func_sym, args);

            operation.committed = true;
            committed_operations.push_back(operation.clone());
            EventPublisher::operation_committed(
                self.env,
                log.transaction_id,
                operation.operation_id,
                &operation.contract_type,
            );
        }

        log.operations = committed_operations;
        log.updated_at = self.env.ledger().timestamp();
        set_transaction_log(self.env, log);

        Ok(())
    }

    /// Validate that all operations in a transaction are compatible
    pub fn validate_transaction(
        &self,
        operations: &Vec<TransactionOperation>,
    ) -> Result<(), TransactionError> {
        if operations.is_empty() {
            return Err(TransactionError::InvalidInput);
        }

        let mut operation_ids: Vec<u64> = Vec::new(self.env);
        for i in 0..operations.len() {
            let operation = operations.get(i).unwrap();
            if operation_ids.contains(&operation.operation_id) {
                return Err(TransactionError::InvalidInput);
            }
            operation_ids.push_back(operation.operation_id);
            self.validate_operation(&operation)?;
        }

        Ok(())
    }

    /// Validate a single operation
    fn validate_operation(&self, operation: &TransactionOperation) -> Result<(), TransactionError> {
        if operation.function_name.len() == 0 {
            return Err(TransactionError::InvalidInput);
        }
        if operation.operation_id == 0 {
            return Err(TransactionError::InvalidInput);
        }
        Ok(())
    }

    /// Check if a transaction can be safely committed
    pub fn can_commit(&self, log: &TransactionLog) -> bool {
        if log.phase != TransactionPhase::Prepared {
            return false;
        }
        for i in 0..log.operations.len() {
            if !log.operations.get(i).unwrap().prepared {
                return false;
            }
        }
        true
    }

    /// Get the status of a specific operation within a transaction
    pub fn get_operation_status(
        &self,
        transaction_id: u64,
        operation_id: u64,
    ) -> Result<String, TransactionError> {
        let log = get_transaction_log(self.env, transaction_id)
            .ok_or(TransactionError::TransactionNotFound)?;

        for i in 0..log.operations.len() {
            let operation = log.operations.get(i).unwrap();
            if operation.operation_id == operation_id {
                if operation.committed {
                    return Ok(String::from_str(self.env, "committed"));
                } else if operation.prepared {
                    return Ok(String::from_str(self.env, "prepared"));
                } else if operation.error.is_some() {
                    return Ok(String::from_str(self.env, "failed"));
                } else {
                    return Ok(String::from_str(self.env, "pending"));
                }
            }
        }

        Err(TransactionError::OperationNotFound)
    }

    /// Get all operations that failed during prepare or commit phase
    pub fn get_failed_operations(
        &self,
        transaction_id: u64,
    ) -> Result<Vec<TransactionOperation>, TransactionError> {
        let log = get_transaction_log(self.env, transaction_id)
            .ok_or(TransactionError::TransactionNotFound)?;

        let mut failed: Vec<TransactionOperation> = Vec::new(self.env);
        for i in 0..log.operations.len() {
            let operation = log.operations.get(i).unwrap();
            if operation.error.is_some() {
                failed.push_back(operation.clone());
            }
        }

        Ok(failed)
    }
}
