#[cfg(test)]
mod gas_benchmarks {
    use super::*;
    use common::transaction::{ContractType, TransactionOperation, TransactionTimeoutConfig};
    use soroban_sdk::{Address, Env, String, Vec};

    #[test]
    #[ignore]
    fn benchmark_direct_vs_orchestrated_calls() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let initiator = Address::generate(&env);
        let contract_address = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Benchmark single operation
        let single_operation = TransactionOperation {
            operation_id: 1,
            contract_type: ContractType::VisionRecords,
            contract_address: contract_address.clone(),
            function_name: String::from_str(&env, "add_record"),
            parameters: Vec::new(&env),
            locked_resources: Vec::new(&env),
            prepared: false,
            committed: false,
            error: None,
        };

        let operations = vec![&env, single_operation];

        // Measure gas for orchestrated transaction
        let start_gas = env.budget().consumed();
        let _result = OrchestratorContract::start_transaction(
            env.clone(),
            initiator.clone(),
            operations,
            Some(300),
            Vec::new(&env),
        );
        let orchestrated_gas = env.budget().consumed() - start_gas;

        println!("Orchestrated transaction gas: {}", orchestrated_gas);

        // Note: Direct call benchmarking would require deploying actual contracts
        // This is a placeholder for the benchmarking structure
    }

    #[test]
    #[ignore]
    fn benchmark_transaction_complexity() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let initiator = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Test with increasing number of operations
        for op_count in [1, 3, 5, 10, 20] {
            let mut operations = Vec::new(&env);

            for i in 1..=op_count {
                operations.push_back(TransactionOperation {
                    operation_id: i,
                    contract_type: match i % 4 {
                        0 => ContractType::VisionRecords,
                        1 => ContractType::Identity,
                        2 => ContractType::Staking,
                        _ => ContractType::ZkVerifier,
                    },
                    contract_address: Address::generate(&env),
                    function_name: String::from_str(&env, "test_function"),
                    parameters: Vec::new(&env),
                    locked_resources: vec![
                        &env,
                        String::from_str(&env, &format!("resource_{}", i)),
                    ],
                    prepared: false,
                    committed: false,
                    error: None,
                });
            }

            let start_gas = env.budget().consumed();
            let _result = OrchestratorContract::start_transaction(
                env.clone(),
                initiator.clone(),
                operations,
                Some(300),
                Vec::new(&env),
            );
            let total_gas = env.budget().consumed() - start_gas;

            println!(
                "Operations: {}, Gas: {}, Gas per op: {}",
                op_count,
                total_gas,
                total_gas / op_count as u64
            );
        }
    }

    #[test]
    #[ignore]
    fn benchmark_deadlock_detection_overhead() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        let deadlock_detector = DeadlockDetector::new(&env);

        // Test deadlock detection with increasing resource contention
        for resource_count in [5, 10, 20, 50] {
            let mut operations = Vec::new(&env);

            for i in 1..=resource_count {
                operations.push_back(TransactionOperation {
                    operation_id: i,
                    contract_type: ContractType::VisionRecords,
                    contract_address: Address::generate(&env),
                    function_name: String::from_str(&env, "test_function"),
                    parameters: Vec::new(&env),
                    locked_resources: vec![
                        &env,
                        String::from_str(&env, &format!("resource_{}", i)),
                        String::from_str(&env, &format!("resource_{}", (i + 1) % resource_count)),
                    ],
                    prepared: false,
                    committed: false,
                    error: None,
                });
            }

            let start_gas = env.budget().consumed();
            let _would_deadlock = deadlock_detector.would_cause_deadlock(&1, &operations);
            let detection_gas = env.budget().consumed() - start_gas;

            println!(
                "Resources: {}, Deadlock detection gas: {}",
                resource_count, detection_gas
            );
        }
    }

    #[test]
    #[ignore]
    fn benchmark_rollback_overhead() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        let rollback_manager = RollbackManager::new(&env);

        // Test rollback with increasing number of operations
        for op_count in [1, 3, 5, 10, 20] {
            let mut operations = Vec::new(&env);

            for i in 1..=op_count {
                operations.push_back(TransactionOperation {
                    operation_id: i,
                    contract_type: ContractType::VisionRecords,
                    contract_address: Address::generate(&env),
                    function_name: String::from_str(&env, "test_function"),
                    parameters: Vec::new(&env),
                    locked_resources: Vec::new(&env),
                    prepared: true,
                    committed: false,
                    error: None,
                });
            }

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

            let start_gas = env.budget().consumed();
            let _result = rollback_manager.rollback_transaction(&log);
            let rollback_gas = env.budget().consumed() - start_gas;

            println!(
                "Operations: {}, Rollback gas: {}, Gas per op: {}",
                op_count,
                rollback_gas,
                rollback_gas / op_count as u64
            );
        }
    }

    #[test]
    #[ignore]
    fn benchmark_event_publishing_overhead() {
        let env = Env::default();

        // Test event publishing overhead
        let event_count = 100;
        let start_gas = env.budget().consumed();

        for i in 1..=event_count {
            EventPublisher::operation_prepared(&env, i, i, &ContractType::VisionRecords);
            EventPublisher::operation_committed(&env, i, i, &ContractType::Identity);
            EventPublisher::gas_consumption(&env, i, i, 1000);
        }

        let event_gas = env.budget().consumed() - start_gas;
        println!(
            "Events: {}, Total gas: {}, Gas per event: {}",
            event_count * 3,
            event_gas,
            event_gas / (event_count * 3) as u64
        );
    }

    #[test]
    #[ignore]
    fn benchmark_storage_operations() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Benchmark transaction log storage and retrieval
        let transaction_count = 50;
        let mut transaction_ids = Vec::new(&env);

        // Create transactions
        for i in 1..=transaction_count {
            let mut operations = Vec::new(&env);
            operations.push_back(TransactionOperation {
                operation_id: i,
                contract_type: ContractType::VisionRecords,
                contract_address: Address::generate(&env),
                function_name: String::from_str(&env, "test_function"),
                parameters: Vec::new(&env),
                locked_resources: Vec::new(&env),
                prepared: false,
                committed: false,
                error: None,
            });

            let log = TransactionLog {
                transaction_id: i,
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

            let start_gas = env.budget().consumed();
            common::transaction::set_transaction_log(&env, &log);
            let storage_gas = env.budget().consumed() - start_gas;

            transaction_ids.push_back(i);

            if i <= 5 {
                println!("Transaction {} storage gas: {}", i, storage_gas);
            }
        }

        // Benchmark retrieval
        let start_gas = env.budget().consumed();
        for tx_id in transaction_ids {
            let _log = common::transaction::get_transaction_log(&env, tx_id);
        }
        let retrieval_gas = env.budget().consumed() - start_gas;

        println!(
            "Retrieved {} transactions, Total gas: {}, Gas per retrieval: {}",
            transaction_count,
            retrieval_gas,
            retrieval_gas / transaction_count as u64
        );
    }

    #[test]
    #[ignore]
    fn benchmark_timeout_processing() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Initialize orchestrator
        OrchestratorContract::initialize(env.clone(), admin.clone(), None).unwrap();

        // Create expired transactions
        let transaction_count = 20;
        for i in 1..=transaction_count {
            let mut operations = Vec::new(&env);
            operations.push_back(TransactionOperation {
                operation_id: i,
                contract_type: ContractType::VisionRecords,
                contract_address: Address::generate(&env),
                function_name: String::from_str(&env, "test_function"),
                parameters: Vec::new(&env),
                locked_resources: Vec::new(&env),
                prepared: false,
                committed: false,
                error: None,
            });

            let log = TransactionLog {
                transaction_id: i,
                initiator: Address::generate(&env),
                phase: TransactionPhase::Preparing,
                status: TransactionStatus::Active,
                operations,
                created_at: env.ledger().timestamp() - 1000, // Created in the past
                updated_at: env.ledger().timestamp() - 1000,
                timeout_seconds: 100, // Very short timeout
                error: None,
                metadata: Vec::new(&env),
            };

            common::transaction::set_transaction_log(&env, &log);
        }

        // Benchmark timeout processing
        let start_gas = env.budget().consumed();
        let _timed_out = OrchestratorContract::process_timeouts(env.clone()).unwrap();
        let timeout_gas = env.budget().consumed() - start_gas;

        println!(
            "Processed {} transactions for timeouts, Gas: {}, Gas per tx: {}",
            transaction_count,
            timeout_gas,
            timeout_gas / transaction_count as u64
        );
    }

    #[test]
    fn benchmark_summary() {
        println!("=== Gas Benchmark Summary ===");
        println!("Run benchmarks with: cargo test --release -- --ignored benchmark");
        println!("");
        println!("Benchmarks available:");
        println!("- benchmark_direct_vs_orchestrated_calls");
        println!("- benchmark_transaction_complexity");
        println!("- benchmark_deadlock_detection_overhead");
        println!("- benchmark_rollback_overhead");
        println!("- benchmark_event_publishing_overhead");
        println!("- benchmark_storage_operations");
        println!("- benchmark_timeout_processing");
        println!("");
        println!("Expected results:");
        println!("- Orchestrated transactions should have higher gas cost than direct calls");
        println!("- Gas cost should scale linearly with operation count");
        println!("- Deadlock detection should be efficient for typical resource counts");
        println!("- Rollback should be comparable in cost to commit operations");
        println!("- Event publishing should have minimal overhead");
    }
}
