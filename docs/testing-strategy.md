# Testing Strategy

This document serves as the definitive guide for all testing practices, quality gates, and coverage requirements across the Teye contract suite.

## Testing Taxonomy

| Test Type | Location | Purpose | Tool | Command |
|-----------|----------|---------|------|---------|
| **Unit Tests** | `contracts/*/src/test.rs` | Individual function logic | cargo test | `cargo test --lib` |
| **Integration Tests** | `contracts/*/tests/core.rs` | Cross-module interactions | cargo test | `cargo test --test` |
| **VisionRecords Integration** | `contracts/vision_records/tests/` | End-to-end patient workflows | cargo test | `cargo test -p vision_records` |
| **Benchmarks** | `contracts/benches/` | Gas usage, performance regression | Criterion | `cargo bench` |
| **Fuzz Tests** | `fuzz/` | Random input exploration | cargo-fuzz/libFuzzer | `cargo fuzz run` |
| **Property Tests** | `contracts/*/tests/property/` | Invariant verification | proptest | `cargo test --features property-testing` |
| **Negative Tests** | `tests/negative/` | Error path validation | cargo test | `cargo test --test negative` |
| **Upgrade Tests** | `tests/upgrade/` | State migration correctness | cargo test | `cargo test --test upgrade` |

## Test Writing Conventions

### Naming Conventions

Follow the pattern: `test_<function>_<scenario>_<expected_result>`

```rust
// ✅ Good
fn test_register_patient_valid_data_success() { }
fn test_register_patient_duplicate_id_error() { }
fn test_grant_access_insufficient_permissions_error() { }

// ❌ Bad
fn test_patient() { }
fn register_test() { }
fn test_grant() { }
```

### Setup Patterns

Use the common test environment setup:

```rust
use soroban_sdk::{testutils::Address as _, Address, Env};
use vision_records::{VisionRecordsContract, VisionRecordsContractClient};

pub fn setup_test_env() -> (Env, Address, VisionRecordsContractClient) {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);
    
    // Initialize contract
    client.initialize(&admin);
    
    (env, admin, client)
}

pub fn create_test_user(env: &Env, name: &str) -> Address {
    let user = Address::generate(env);
    // Additional setup as needed
    user
}
```

### Assertion Patterns

```rust
// ✅ Use specific assertions
assert_eq!(result, expected_value);
assert!(condition, "Error message with context");
assert_ne!(value1, value2, "Values should be different");

// ✅ Use #[should_panic] for expected panics
#[test]
#[should_panic(expected = "InsufficientPermissions")]
fn test_unauthorized_access_should_panic() {
    // Test code that should panic
}

// ❌ Avoid generic assertions
assert!(true); // Doesn't test anything meaningful
```

### Mock Patterns

```rust
// Mock external contract calls
#[cfg(test)]
pub mod mocks {
    use soroban_sdk::{Address, Env};
    
    pub struct MockExternalContract {
        env: Env,
        contract_id: Address,
    }
    
    impl MockExternalContract {
        pub fn new(env: &Env, contract_id: Address) -> Self {
            Self {
                env: env.clone(),
                contract_id,
            }
        }
        
        pub fn mock_response(&self, response: &str) {
            self.env.when_invoked(&self.contract_id, "external_function")
                .returns_with(response.to_val());
        }
    }
}
```

### Snapshot Testing

```rust
#[cfg(test)]
mod snapshots {
    use super::*;
    use insta::assert_debug_snapshot;
    
    #[test]
    fn test_patient_profile_snapshot() {
        let (env, _, client) = setup_test_env();
        let patient = create_test_patient(&env);
        
        let profile = client.get_patient_profile(&patient);
        assert_debug_snapshot!(profile);
    }
}
```

## Coverage Requirements

### Per-Contract Minimum Coverage Targets

| Contract | Minimum Coverage | Critical Paths |
|----------|-----------------|----------------|
| `vision_records` | 85% | Patient registration, record access, audit trail |
| `governor` | 80% | Proposal creation, voting, execution |
| `staking` | 80% | Stake/unstake, reward calculation |
| `treasury` | 85% | Fund transfers, authorization |
| `zk_verifier` | 90% | Proof verification, security checks |
| `compliance` | 85% | HIPAA controls, audit logging |
| `common` | 75% | Utility functions, shared components |

### Running Coverage Locally

```bash
# Install coverage tools
cargo install cargo-tarpaulin

# Run coverage for all contracts
./scripts/run_coverage.sh

# Run coverage for specific contract
cargo tarpaulin --package vision_records --out Html

# Generate detailed report
cargo tarpaulin --all --out Html --output-dir coverage/
```

### Interpreting Coverage Reports

- **Green**: Covered code paths
- **Red**: Uncovered code paths
- **Yellow**: Partially covered branches
- **Exempt**: `#[cfg(test)]` blocks, error handling paths

### Coverage Exemptions

The following code paths are exempt from coverage requirements:

1. `#[cfg(test)]` blocks - Test-only code
2. Panic handlers and error formatting
3. Debug/trace logging statements
4. Unreachable code patterns (marked with `unreachable!()`)

## Benchmarking Guide

### Running Gas Benchmarks

```bash
# Install benchmark dependencies
cargo install cargo-criterion

# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench -- gas_usage

# Run with specific filter
cargo bench -- "gas_sim_*"
```

### Benchmark Files

#### `gas_usage.rs` - Gas Consumption Analysis

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vision_records::VisionRecordsContractClient;

fn bench_patient_registration(c: &mut Criterion) {
    let (env, _, client) = setup_test_env();
    let patient = Address::generate(&env);
    
    c.bench_function("register_patient", |b| {
        b.iter(|| {
            client.register_patient(
                black_box(&patient),
                black_box(&"Test Patient".into_val(&env)),
                black_box(&"1990-01-01".into_val(&env)),
                black_box(&"test@example.com".into_val(&env)),
                black_box(&"Emergency Contact".into_val(&env)),
            );
        });
    });
}

fn bench_vision_record_creation(c: &mut Criterion) {
    let (env, _, client) = setup_test_env();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    
    c.bench_function("add_vision_record", |b| {
        b.iter(|| {
            client.add_vision_record(
                black_box(&patient),
                black_box(&provider),
                black_box(&"exam".into_val(&env)),
                black_box(&[0u8; 32]),
                black_box(&"metadata".into_val(&env)),
            );
        });
    });
}

criterion_group!(
    gas_benches,
    bench_patient_registration,
    bench_vision_record_creation
);
criterion_main!(gas_benches);
```

#### `public_functions.rs` - Public API Performance

```rust
fn bench_public_api_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("public_api");
    
    // Test with different input sizes
    for size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("get_patient_records", size),
            size,
            |b, &size| {
                let (env, _, client) = setup_test_env_with_records(size);
                let patient = Address::generate(&env);
                
                b.iter(|| {
                    client.get_patient_records(black_box(&patient));
                });
            },
        );
    }
    
    group.finish();
}
```

#### `regression.rs` - Performance Regression Detection

```rust
fn regression_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("regression");
    
    // Set performance thresholds
    group.measurement_time(std::time::Duration::from_secs(10));
    group.sample_size(100);
    
    group.bench_function("critical_path_patient_workflow", |b| {
        b.iter(|| {
            let (env, _, client) = setup_test_env();
            let patient = create_test_patient(&env);
            
            // Complete patient workflow
            client.register_patient(&patient, &name, &dob, &contact, &emergency);
            let record_id = client.add_vision_record(&patient, &provider, &exam_type, &hash, &metadata);
            client.grant_access(&patient, &provider, &permissions, &duration);
            
            record_id
        });
    });
    
    group.finish();
}
```

### Interpreting Benchmark Results

- **ns/iter**: Nanoseconds per iteration (lower is better)
- **MB/s**: Megabytes per second throughput
- **Gas**: Estimated gas consumption
- **Regression**: Performance degradation > 10% from baseline

### Adding New Benchmarks

1. Create benchmark function following naming convention
2. Add to appropriate criterion group
3. Include performance thresholds for regression detection
4. Document what the benchmark measures

## Fuzz Testing Deep-Dive

### Corpus Management

```bash
# Fuzz corpus directory structure
fuzz/
├── corpus/
│   ├── vision_records/
│   │   ├── patient_registration/
│   │   ├── record_access/
│   │   └── emergency_scenarios/
│   ├── staking/
│   └── audit/
└── artifacts/
    ├── crashes/
    └── hangs/
```

### Coverage-Guided Fuzzing Configuration

```rust
// fuzz/fuzz_targets/vision_records.rs
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::Address as _, Address, Env, String};
use vision_records::{RecordType, Role, VisionRecordsContractClient};

#[derive(Arbitrary, Debug)]
pub enum FuzzAction {
    RegisterUser { name_len: u8, role: u8 },
    AddRecord { record_type: u8, hash_len: u8 },
    GrantAccess { permissions_len: u8, duration: u64 },
    RevokeAccess,
    EmergencyAccess,
}

fuzz_target!(|actions: Vec<FuzzAction>| {
    let env = Env::default();
    let admin = Address::generate(&env);
    
    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);
    
    // Initialize contract
    let _ = client.try_initialize(&admin);
    
    // Execute fuzz actions
    for action in actions {
        execute_fuzz_action(&env, &client, action);
    }
});

fn execute_fuzz_action(env: &Env, client: &VisionRecordsContractClient, action: FuzzAction) {
    match action {
        FuzzAction::RegisterUser { name_len, role } => {
            let user = Address::generate(env);
            let role_enum = match role % 4 {
                0 => Role::Patient,
                1 => Role::Optometrist,
                2 => Role::Ophthalmologist,
                _ => Role::Admin,
            };
            
            // Generate random name with length bounds
            let name_len = (name_len as usize).min(100).max(1);
            let name = "A".repeat(name_len);
            
            let _ = client.try_register_user(
                &user,
                &role_enum,
                &String::from_str(env, &name),
            );
        },
        FuzzAction::AddRecord { record_type, hash_len } => {
            // Implementation for record addition fuzzing
        },
        // ... other action implementations
    }
}
```

### Sanitizer Integration

```bash
# Run fuzzing with sanitizers
RUSTFLAGS="-Z sanitizer=address" cargo fuzz run vision_records

# Memory sanitizer
RUSTFLAGS="-Z sanitizer=memory" cargo fuzz run vision_records

# Undefined behavior sanitizer
RUSTFLAGS="-Z sanitizer=undefined" cargo fuzz run vision_records
```

### Investigating Fuzzer-Found Bugs

1. **Reproduce the crash**:
   ```bash
   cargo fuzz replay vision_records crash-xxxxx
   ```

2. **Minimize the input**:
   ```bash
   cargo fuzz tmin vision_records crash-xxxxx
   ```

3. **Analyze the stack trace**:
   - Look for buffer overflows
   - Check for integer overflows
   - Verify state consistency

4. **Add regression test**:
   ```rust
   #[test]
   fn test_fuzzer_crash_regression() {
       let (env, _, client) = setup_test_env();
       // Reproduce the exact conditions that caused the crash
       // Assert the expected behavior
   }
   ```

### Writing Fuzz Targets for New Contracts

1. **Identify entry points**: Public functions that accept external input
2. **Define action enum**: All possible operations to fuzz
3. **Implement arbitrary trait**: For generating valid test data
4. **Set up test environment**: Contract initialization and state
5. **Execute actions**: Random sequence of operations
6. **Handle panics gracefully**: Ensure fuzzer can continue

## CI Quality Gates

### Pipeline Stages

```yaml
# .github/workflows/ci.yml
name: CI Pipeline

on: [push, pull_request]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Rust fmt check
        run: cargo fmt -- --check
      - name: Clippy lint
        run: cargo clippy --all-targets --all-features -- -D warnings

  test:
    runs-on: ubuntu-latest
    needs: lint
    steps:
      - uses: actions/checkout@v3
      - name: Run unit tests
        run: cargo test --all
      - name: Run integration tests
        run: cargo test --test '*'
      - name: Check coverage
        run: ./scripts/run_coverage.sh

  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Dependency audit
        run: cargo deny check
      - name: Security audit
        run: cargo audit

  fuzz:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v3
      - name: Run brief fuzz test
        run: cargo fuzz run vision_records -- -max_total_time=60

  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run benchmarks
        run: cargo bench
      - name: Check for regressions
        run: ./scripts/check_benchmark_regressions.sh
```

### Quality Gate Requirements

| Gate | Tool | Success Criteria |
|------|------|------------------|
| **Formatting** | `cargo fmt` | No formatting changes needed |
| **Linting** | `cargo clippy` | Zero warnings, zero errors |
| **Testing** | `cargo test` | All tests pass |
| **Coverage** | `cargo tarpaulin` | ≥ 80% coverage per contract |
| **Security** | `cargo deny` | No denied dependencies |
| **Vulnerabilities** | `cargo audit` | No high/critical vulnerabilities |
| **Fuzzing** | `cargo fuzz` | No crashes in brief run |
| **Benchmarks** | `cargo bench` | No performance regressions |

### Running Full CI Pipeline Locally

```bash
#!/bin/bash
# scripts/run_full_ci.sh

set -e

echo "🔍 Running formatting check..."
cargo fmt -- --check

echo "🔍 Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

echo "🧪 Running tests..."
cargo test --all

echo "📊 Checking coverage..."
./scripts/run_coverage.sh

echo "🔒 Running security checks..."
cargo deny check
cargo audit

echo "🔥 Running brief fuzz test..."
cargo fuzz run vision_records -- -max_total_time=60

echo "📈 Running benchmarks..."
cargo bench

echo "✅ All CI checks passed!"
```

## Test Data Management

### Test Fixtures

```rust
// tests/fixtures/mod.rs
use soroban_sdk::{Address, Env, String};

pub struct TestFixtures {
    pub env: Env,
    pub admin: Address,
    pub patient: Address,
    pub provider: Address,
}

impl TestFixtures {
    pub fn new() -> Self {
        let env = Env::default();
        let admin = Address::generate(&env);
        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        
        Self { env, admin, patient, provider }
    }
    
    pub fn sample_patient_data(&self) -> PatientData {
        PatientData {
            name: String::from_str(&self.env, "John Doe"),
            date_of_birth: String::from_str(&self.env, "1990-01-01"),
            contact_info: String::from_str(&self.env, "john@example.com"),
            emergency_contact: String::from_str(&self.env, "Jane Doe"),
        }
    }
}
```

### Test Utilities

```rust
// tests/utils/mod.rs
use soroban_sdk::{Env, Address};

pub fn create_test_user(env: &Env, name: &str) -> Address {
    let user = Address::generate(env);
    // Additional setup as needed
    user
}

pub fn assert_contract_event(env: &Env, expected_topic: &str, expected_data: &str) {
    // Verify that specific event was emitted
    let events = env.events().all();
    assert!(events.iter().any(|event| {
        event.topic.to_string().contains(expected_topic) &&
        event.data.to_string().contains(expected_data)
    }));
}

pub fn setup_test_contract<T>(env: &Env, admin: &Address) -> T {
    // Generic contract setup
    // Implementation depends on contract type
}
```

## Continuous Integration Best Practices

1. **Parallel Execution**: Run tests in parallel where possible
2. **Caching**: Cache dependencies and build artifacts
3. **Fail Fast**: Configure CI to fail on first error
4. **Notifications**: Set up alerts for CI failures
5. **Artifact Storage**: Store test results and coverage reports
6. **Rollback Testing**: Test upgrade and rollback procedures

## Test Documentation Standards

All test files must include:

```rust
//! # Module Tests
//!
//! This module contains tests for [module_name].
//!
//! ## Test Coverage
//! - [x] Function 1: Description of test coverage
//! - [x] Function 2: Description of test coverage
//! - [ ] Function 3: TODO: Add edge case testing
//!
//! ## Test Categories
//! - Unit tests: Individual function testing
//! - Integration tests: Cross-module interaction testing
//! - Property tests: Invariant verification
//!
//! ## Known Limitations
//! - List any known gaps in test coverage
//! - Document any assumptions made in tests
```

This testing strategy provides a comprehensive framework for maintaining code quality, security, and performance across the entire Teye contract suite.
