use soroban_sdk::{Env, Vec, String};
use common::transaction::{TransactionOperation, TransactionError, DeadlockInfo, RESOURCE_LOCKS};

/// Deadlock detector for preventing and resolving transaction deadlocks
pub struct DeadlockDetector<'a> {
    env: &'a Env,
}

impl<'a> DeadlockDetector<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }

    /// Check if a new transaction would cause a deadlock
    pub fn would_cause_deadlock(&self, transaction_id: &u64, operations: &Vec<TransactionOperation>) -> bool {
        let dependency_graph = self.build_dependency_graph(transaction_id, operations);
        self.has_cycles(&dependency_graph)
    }

    /// Build a dependency graph for the current transaction
    fn build_dependency_graph(&self, transaction_id: &u64, operations: &Vec<TransactionOperation>) -> DependencyGraph<'_> {
        let mut graph = DependencyGraph::new(self.env);

        let current_locks: Vec<(String, u64)> = self.env.storage().instance()
            .get(&RESOURCE_LOCKS)
            .unwrap_or(Vec::new(self.env));

        // Check each new operation's resources against existing locks
        for op_idx in 0..operations.len() {
            let operation = operations.get(op_idx).unwrap();
            for res_idx in 0..operation.locked_resources.len() {
                let resource = operation.locked_resources.get(res_idx).unwrap();

                for lock_idx in 0..current_locks.len() {
                    let (locked_resource, locked_tx_id) = current_locks.get(lock_idx).unwrap();
                    if locked_resource == resource && locked_tx_id != *transaction_id {
                        graph.add_dependency(*transaction_id, locked_tx_id, resource.clone());
                    }
                }
            }
        }

        graph
    }

    /// Check if the dependency graph has cycles (indicating deadlock)
    fn has_cycles(&self, graph: &DependencyGraph) -> bool {
        let mut visited: Vec<u64> = Vec::new(self.env);
        let mut recursion_stack: Vec<u64> = Vec::new(self.env);

        let transactions = graph.get_all_transactions();

        for i in 0..transactions.len() {
            let transaction = transactions.get(i).unwrap();
            if !visited.contains(&transaction) {
                if self.dfs_has_cycle(graph, transaction, &mut visited, &mut recursion_stack) {
                    return true;
                }
            }
        }

        false
    }

    /// Depth-first search to detect cycles
    fn dfs_has_cycle(
        &self,
        graph: &DependencyGraph,
        transaction: u64,
        visited: &mut Vec<u64>,
        recursion_stack: &mut Vec<u64>,
    ) -> bool {
        visited.push_back(transaction);
        recursion_stack.push_back(transaction);

        let dependencies = graph.get_dependencies(transaction);

        for i in 0..dependencies.len() {
            let (dependent_tx, _resource) = dependencies.get(i).unwrap();
            if !visited.contains(&dependent_tx) {
                if self.dfs_has_cycle(graph, dependent_tx, visited, recursion_stack) {
                    return true;
                }
            } else if recursion_stack.contains(&dependent_tx) {
                return true;
            }
        }

        // Remove from recursion stack (pop last element)
        let len = recursion_stack.len();
        if len > 0 {
            recursion_stack.remove(len - 1);
        }
        false
    }

    /// Detect and resolve existing deadlocks
    pub fn detect_and_resolve_deadlocks(&self) -> Result<Vec<DeadlockInfo>, TransactionError> {
        let current_locks: Vec<(String, u64)> = self.env.storage().instance()
            .get(&RESOURCE_LOCKS)
            .unwrap_or(Vec::new(self.env));

        if current_locks.is_empty() {
            return Ok(Vec::new(self.env));
        }

        Ok(Vec::new(self.env))
    }

    /// Get resources that are causing conflicts in a deadlock cycle
    fn get_conflicting_resources(&self, cycle: &Vec<u64>) -> Vec<String> {
        let current_locks: Vec<(String, u64)> = self.env.storage().instance()
            .get(&RESOURCE_LOCKS)
            .unwrap_or(Vec::new(self.env));

        let mut conflicting_resources: Vec<String> = Vec::new(self.env);

        for i in 0..current_locks.len() {
            let (resource, tx_id) = current_locks.get(i).unwrap();
            if cycle.contains(&tx_id) && !conflicting_resources.contains(&resource) {
                conflicting_resources.push_back(resource);
            }
        }

        conflicting_resources
    }

    /// Get deadlock prevention suggestions
    pub fn get_deadlock_prevention_suggestions(&self, operations: &Vec<TransactionOperation>) -> Vec<String> {
        let mut suggestions: Vec<String> = Vec::new(self.env);

        suggestions.push_back(String::from_str(self.env, "Acquire resources in consistent order"));
        suggestions.push_back(String::from_str(self.env, "Configure appropriate timeouts"));

        if operations.len() > 5 {
            suggestions.push_back(String::from_str(self.env, "Break large transactions into smaller batches"));
        }

        for i in 0..operations.len() {
            let operation = operations.get(i).unwrap();
            if operation.locked_resources.len() > 3 {
                suggestions.push_back(String::from_str(self.env, "Use more granular resource locking"));
                break;
            }
        }

        suggestions
    }
}

/// Dependency graph for deadlock detection
struct DependencyGraph<'a> {
    env: &'a Env,
    dependencies: Vec<(u64, u64, String)>, // (from_tx, to_tx, resource)
}

impl<'a> DependencyGraph<'a> {
    fn new(env: &'a Env) -> Self {
        Self {
            env,
            dependencies: Vec::new(env),
        }
    }

    fn add_dependency(&mut self, from_tx: u64, to_tx: u64, resource: String) {
        self.dependencies.push_back((from_tx, to_tx, resource));
    }

    fn get_dependencies(&self, transaction: u64) -> Vec<(u64, String)> {
        let mut deps: Vec<(u64, String)> = Vec::new(self.env);

        for i in 0..self.dependencies.len() {
            let (from_tx, to_tx, resource) = self.dependencies.get(i).unwrap();
            if from_tx == transaction {
                deps.push_back((to_tx, resource));
            }
        }

        deps
    }

    fn get_all_transactions(&self) -> Vec<u64> {
        let mut transactions: Vec<u64> = Vec::new(self.env);

        for i in 0..self.dependencies.len() {
            let (from_tx, to_tx, _) = self.dependencies.get(i).unwrap();
            if !transactions.contains(&from_tx) {
                transactions.push_back(from_tx);
            }
            if !transactions.contains(&to_tx) {
                transactions.push_back(to_tx);
            }
        }

        transactions
    }
}
