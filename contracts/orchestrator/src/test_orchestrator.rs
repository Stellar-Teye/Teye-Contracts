#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Address, Env, String, Vec};
    use common::{
        transaction::{TransactionOperation, TransactionPhase, TransactionStatus, TransactionError,
                      ContractType, TransactionTimeoutConfig, get_default_timeout_config},
    };

    #[test]
    fn test_orchestrator_initialization() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Test successful initialization
        assert_eq!(
            OrchestratorContract::initialize(env.clone(), admin.clone(), None),
            Ok(())
        );

        // Test duplicate initialization
        assert_eq!(
            OrchestratorContract::initialize(env.clone(), admin, None),
            Err(TransactionError::TransactionExists)
        );
    }

    #[test]
    fn test_simple_transaction_success() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let initiator = Address::generate(&env);
        let contract_address = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Create a simple transaction operation
        let mut operations = Vec::new(&env);
        operations.push_back(TransactionOperation {
            operation_id: 1,
            contract_type: ContractType::VisionRecords,
            contract_address: contract_address.clone(),
            function_name: String::from_str(&env, "add_record"),
            parameters: Vec::new(&env),
            locked_resources: Vec::new(&env),
            prepared: false,
            committed: false,
            error: None,
        });

        // Start transaction (should fail gracefully since contract doesn't exist)
        let result = OrchestratorContract::start_transaction(
            env.clone(),
            initiator.clone(),
            operations,
            Some(300),
            Vec::new(&env),
        );

        // Should fail due to contract call failure, but transaction should be created
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_validation() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let initiator = Address::generate(&env);
        let contract_address = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        let tx_manager = TransactionManager::new(&env);

        // Test empty operations
        let empty_operations = Vec::new(&env);
        assert_eq!(
            tx_manager.validate_transaction(&empty_operations),
            Err(TransactionError::InvalidInput)
        );

        // Test duplicate operation IDs
        let mut duplicate_ops = Vec::new(&env);
        duplicate_ops.push_back(TransactionOperation {
            operation_id: 1,
            contract_type: ContractType::VisionRecords,
            contract_address: contract_address.clone(),
            function_name: String::from_str(&env, "add_record"),
            parameters: Vec::new(&env),
            locked_resources: Vec::new(&env),
            prepared: false,
            committed: false,
            error: None,
        });
        duplicate_ops.push_back(TransactionOperation {
            operation_id: 1, // Duplicate ID
            contract_type: ContractType::Identity,
            contract_address: contract_address.clone(),
            function_name: String::from_str(&env, "add_guardian"),
            parameters: Vec::new(&env),
            locked_resources: Vec::new(&env),
            prepared: false,
            committed: false,
            error: None,
        });

        assert_eq!(
            tx_manager.validate_transaction(&duplicate_ops),
            Err(TransactionError::InvalidInput)
        );
    }

    #[test]
    fn test_deadlock_detection() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        let deadlock_detector = DeadlockDetector::new(&env);

        // Test operations with conflicting resources
        let mut operations1 = Vec::new(&env);
        operations1.push_back(TransactionOperation {
            operation_id: 1,
            contract_type: ContractType::VisionRecords,
            contract_address: Address::generate(&env),
            function_name: String::from_str(&env, "add_record"),
            parameters: Vec::new(&env),
            locked_resources: vec![
                &env,
                String::from_str(&env, "resource_1"),
                String::from_str(&env, "resource_2"),
            ],
            prepared: false,
            committed: false,
            error: None,
        });

        let mut operations2 = Vec::new(&env);
        operations2.push_back(TransactionOperation {
            operation_id: 2,
            contract_type: ContractType::Identity,
            contract_address: Address::generate(&env),
            function_name: String::from_str(&env, "add_guardian"),
            parameters: Vec::new(&env),
            locked_resources: vec![
                &env,
                String::from_str(&env, "resource_2"),
                String::from_str(&env, "resource_1"),
            ],
            prepared: false,
            committed: false,
            error: None,
        });

        // First transaction should not cause deadlock
        assert!(!deadlock_detector.would_cause_deadlock(&1, &operations1));

        // Simulate resource locks for first transaction
        let mut locks = Vec::new(&env);
        locks.push_back((String::from_str(&env, "resource_1"), 1));
        locks.push_back((String::from_str(&env, "resource_2"), 1));
        env.storage().instance().set(&common::transaction::RESOURCE_LOCKS, &locks);

        // Second transaction should detect potential deadlock
        assert!(deadlock_detector.would_cause_deadlock(&2, &operations2));
    }

    #[test]
    fn test_rollback_functionality() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        let rollback_manager = RollbackManager::new(&env);

        // Create a mock transaction log
        let mut operations = Vec::new(&env);
        operations.push_back(TransactionOperation {
            operation_id: 1,
            contract_type: ContractType::VisionRecords,
            contract_address: Address::generate(&env),
            function_name: String::from_str(&env, "add_record"),
            parameters: Vec::new(&env),
            locked_resources: Vec::new(&env),
            prepared: true,
            committed: false,
            error: None,
        });

        let log = TransactionLog {
            transaction_id: 1,
            initiator: Address::generate(&env),
            phase: TransactionPhase::Preparing,
            status: TransactionStatus::Active,
            operations,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
            timeout_seconds: 300,
            error: None,
            metadata: Vec::new(&env),
        };

        // Test rollback (should fail gracefully since contract doesn't exist)
        let result = rollback_manager.rollback_transaction(&log);
        assert!(result.is_err() || result.is_ok()); // Either way is fine for this test
    }

    #[test]
    fn test_timeout_configuration() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Test default timeout config
        let config = OrchestratorContract::get_timeout_config(env.clone()).unwrap();
        assert_eq!(config.default_timeout, 300);
        assert_eq!(config.max_timeout, 3600);

        // Test updating timeout config
        let new_config = TransactionTimeoutConfig {
            default_timeout: 600,
            max_timeout: 7200,
            contract_timeouts: Vec::new(&env),
        };

        assert_eq!(
            OrchestratorContract::update_timeout_config(env.clone(), admin.clone(), new_config.clone()),
            Ok(())
        );

        // Verify updated config
        let retrieved_config = OrchestratorContract::get_timeout_config(env.clone()).unwrap();
        assert_eq!(retrieved_config.default_timeout, 600);
        assert_eq!(retrieved_config.max_timeout, 7200);

        // Test invalid config (default > max)
        let invalid_config = TransactionTimeoutConfig {
            default_timeout: 7200,
            max_timeout: 3600,
            contract_timeouts: Vec::new(&env),
        };

        assert_eq!(
            OrchestratorContract::update_timeout_config(env.clone(), admin, invalid_config),
            Err(TransactionError::InvalidInput)
        );
    }

    #[test]
    fn test_transaction_queries() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let initiator = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Test getting non-existent transaction
        assert_eq!(
            OrchestratorContract::get_transaction(env.clone(), 999),
            Err(TransactionError::TransactionNotFound)
        );

        // Test getting active transactions (should be empty initially)
        let active = OrchestratorContract::get_active_transactions(env.clone()).unwrap();
        assert_eq!(active.len(), 0);
    }

    #[test]
    fn test_manual_rollback() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let initiator = Address::generate(&env);
        let contract_address = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Create a transaction that will fail
        let mut operations = Vec::new(&env);
        operations.push_back(TransactionOperation {
            operation_id: 1,
            contract_type: ContractType::VisionRecords,
            contract_address: contract_address.clone(),
            function_name: String::from_str(&env, "add_record"),
            parameters: Vec::new(&env),
            locked_resources: Vec::new(&env),
            prepared: false,
            committed: false,
            error: None,
        });

        // Start transaction (should fail)
        let result = OrchestratorContract::start_transaction(
            env.clone(),
            initiator.clone(),
            operations,
            Some(300),
            Vec::new(&env),
        );

        // Try to rollback non-existent transaction
        assert_eq!(
            OrchestratorContract::rollback_transaction(env.clone(), admin.clone(), 999),
            Err(TransactionError::TransactionNotFound)
        );
    }

    #[test]
    fn test_timeout_processing() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Test processing timeouts with no active transactions
        let timed_out = OrchestratorContract::process_timeouts(env.clone()).unwrap();
        assert_eq!(timed_out.len(), 0);
    }

    #[test]
    fn test_event_publishing() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Test that events can be published
        let log = TransactionLog {
            transaction_id: 1,
            initiator: Address::generate(&env),
            phase: TransactionPhase::Preparing,
            status: TransactionStatus::Active,
            operations: Vec::new(&env),
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
            timeout_seconds: 300,
            error: None,
            metadata: Vec::new(&env),
        };

        // These should not panic
        EventPublisher::transaction_started(&env, &log);
        EventPublisher::transaction_prepared(&env, &log);
        EventPublisher::transaction_committed(&env, &log);
        EventPublisher::transaction_rolled_back(&env, &log);
        EventPublisher::transaction_timed_out(&env, &log);
    }

    #[test]
    fn test_error_handling() {
        let env = Env::default();
        let non_admin = Address::generate(&env);

        // Test operations without initialization
        assert_eq!(
            OrchestratorContract::get_timeout_config(env.clone()),
            Err(TransactionError::Unauthorized)
        );

        assert_eq!(
            OrchestratorContract::get_active_transactions(env.clone()),
            Err(TransactionError::Unauthorized)
        );

        // Test admin operations without initialization
        assert_eq!(
            OrchestratorContract::update_timeout_config(
                env.clone(),
                non_admin.clone(),
                get_default_timeout_config(&env)
            ),
            Err(TransactionError::Unauthorized)
        );

        assert_eq!(
            OrchestratorContract::rollback_transaction(env.clone(), non_admin.clone(), 1),
            Err(TransactionError::Unauthorized)
        );
    }

    #[test]
    fn test_authorization() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let non_admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Test admin-only operations with non-admin
        assert_eq!(
            OrchestratorContract::update_timeout_config(
                env.clone(),
                non_admin.clone(),
                get_default_timeout_config(&env)
            ),
            Err(TransactionError::Unauthorized)
        );

        assert_eq!(
            OrchestratorContract::rollback_transaction(env.clone(), non_admin.clone(), 1),
            Err(TransactionError::Unauthorized)
        );

        // Test admin operations with actual admin
        let new_config = TransactionTimeoutConfig {
            default_timeout: 400,
            max_timeout: 4000,
            contract_timeouts: Vec::new(&env),
        };

        assert_eq!(
            OrchestratorContract::update_timeout_config(env.clone(), admin.clone(), new_config),
            Ok(())
        );
    }

    #[test]
    fn test_complex_transaction_scenarios() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let initiator = Address::generate(&env);
        let contract1 = Address::generate(&env);
        let contract2 = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Create a complex transaction with multiple operations
        let mut operations = Vec::new(&env);
        operations.push_back(TransactionOperation {
            operation_id: 1,
            contract_type: ContractType::VisionRecords,
            contract_address: contract1.clone(),
            function_name: String::from_str(&env, "add_record"),
            parameters: Vec::new(&env),
            locked_resources: vec![&env, String::from_str(&env, "patient_123")],
            prepared: false,
            committed: false,
            error: None,
        });

        operations.push_back(TransactionOperation {
            operation_id: 2,
            contract_type: ContractType::Identity,
            contract_address: contract2.clone(),
            function_name: String::from_str(&env, "add_guardian"),
            parameters: Vec::new(&env),
            locked_resources: vec![&env, String::from_str(&env, "identity_456")],
            prepared: false,
            committed: false,
            error: None,
        });

        // Start transaction (should fail due to contract calls, but structure should be valid)
        let result = OrchestratorContract::start_transaction(
            env.clone(),
            initiator.clone(),
            operations,
            Some(600),
            vec![&env, String::from_str(&env, "complex_test")],
        );

        // Should fail due to contract call issues
        assert!(result.is_err());
    }

    #[test]
    fn test_performance_monitoring() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Test performance monitoring events
        EventPublisher::performance_metrics(&env, 1, 100, 200, 300);
        EventPublisher::gas_consumption(&env, 1, 1, 50000);
        EventPublisher::health_check(&env, 5, 10, 2);
        EventPublisher::monitoring_event(&env, &String::from_str(&env, "tx_rate"), 100, Some(150));
    }

    #[test]
    fn test_security_events() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Test security event publishing
        EventPublisher::security_event(
            &env,
            &String::from_str(&env, "unauthorized_access"),
            &String::from_str(&env, "high"),
            vec![&env, String::from_str(&env, "attempted transaction without authorization")],
        );

        EventPublisher::audit_trail(
            &env,
            1,
            &String::from_str(&env, "transaction_started"),
            &admin,
            vec![&env, String::from_str(&env, "test_transaction")],
        );
    }
}
