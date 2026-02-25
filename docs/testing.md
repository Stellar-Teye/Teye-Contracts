# Testing Documentation

## Overview

The Vision Records contract includes a comprehensive test suite covering unit tests, integration tests, and property-based tests. This document describes the testing strategy, test organization, and how to run the tests.

## Test Structure

```
contracts/vision_records/tests/
├── common/              # Shared test utilities
│   └── mod.rs          # Test context and helper functions
├── integration/        # Integration test suite
│   ├── mod.rs          # Integration test module
│   ├── patient_workflows.rs    # Patient registration and management
│   ├── provider_workflows.rs   # Provider onboarding workflows
│   ├── record_workflows.rs     # Record creation and access
│   ├── emergency_workflows.rs  # Emergency access scenarios
│   └── end_to_end.rs           # Complete end-to-end workflows
├── appointment.rs       # Appointment scheduling tests
├── audit.rs            # Audit logging tests
├── core.rs             # Core functionality tests
├── emergency.rs        # Emergency access tests
├── errors.rs           # Error handling tests
├── provider.rs         # Provider management tests
├── rate_limit.rs      # Rate limiting tests
├── rbac.rs             # Role-based access control tests
└── property/           # Property-based tests
    ├── access.rs
    ├── core.rs
    ├── main.rs
    ├── rbac.rs
    └── state_machine.rs
```

## Test Categories

### 1. Unit Tests

Unit tests focus on individual functions and components:

- **Core Tests** (`core.rs`): Basic contract functionality
- **RBAC Tests** (`rbac.rs`): Permission and role management
- **Provider Tests** (`provider.rs`): Provider registration and verification
- **Audit Tests** (`audit.rs`): Audit logging functionality
- **Rate Limit Tests** (`rate_limit.rs`): Rate limiting mechanisms
- **Error Tests** (`errors.rs`): Error handling and logging

### 2. Integration Tests

Integration tests cover complete user workflows:

#### Patient Workflows (`integration/patient_workflows.rs`)
- Patient registration workflow
- Granting access to family members
- Revoking access
- Managing multiple access grants
- Access expiration
- Viewing record lists

#### Provider Workflows (`integration/provider_workflows.rs`)
- Complete provider onboarding
- Provider registration with credentials
- Provider verification workflow
- Creating records workflow
- Searching for patients
- Rate limit bypass for verified providers

#### Record Workflows (`integration/record_workflows.rs`)
- Record creation workflow
- Record access by different users
- Multiple record types
- Access levels (Read, Write, Full)
- Access expiration
- Access revocation
- Multiple providers for same patient
- Audit logging

#### Emergency Workflows (`integration/emergency_workflows.rs`)
- Complete emergency access workflow
- Different emergency conditions
- Emergency access expiration
- Emergency access revocation
- Multiple emergency contacts
- Emergency access audit trail
- Verification requirements

#### End-to-End Workflows (`integration/end_to_end.rs`)
- Complete patient journey
- Complete provider workflow
- Complete emergency workflow
- Multi-provider collaboration
- Appointment scheduling integration
- Rate limiting integration
- Audit logging integration

### 3. Property-Based Tests

Property-based tests verify invariants and properties:

- **State Machine Tests**: Contract state transitions
- **Access Control Tests**: Permission invariants
- **Core Property Tests**: Core functionality properties

## Running Tests

### Run All Tests

```bash
cd contracts/vision_records
cargo test
```

### Run Specific Test Suite

```bash
# Run only integration tests
cargo test --test integration

# Run only unit tests
cargo test --lib

# Run specific test file
cargo test --test patient_workflows
```

### Run with Output

```bash
# Show test output
cargo test -- --nocapture

# Run specific test
cargo test test_patient_registration_workflow -- --nocapture
```

### Run with Coverage

```bash
# Install cargo-tarpaulin (coverage tool)
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html
```

## Test Coverage Goals

The test suite aims for **>90% code coverage** across all modules:

- **Core Functionality**: 100% coverage
- **Access Control**: 95%+ coverage
- **Provider Management**: 90%+ coverage
- **Emergency Access**: 95%+ coverage
- **Audit Logging**: 90%+ coverage
- **Rate Limiting**: 90%+ coverage

## Test Utilities

### Common Test Functions

The `common/mod.rs` module provides shared utilities:

```rust
// Create test environment
let ctx = setup_test_env();

// Create test user
let user = create_test_user(&ctx, Role::Patient, "John Doe");

// Create test record
let record_id = create_test_record(
    &ctx,
    &provider,
    &patient,
    &provider,
    RecordType::Examination,
    "QmHash123",
);
```

### Test Context

The `TestContext` struct provides:
- `env`: Soroban environment
- `client`: Contract client
- `admin`: Admin address

## Writing New Tests

### Integration Test Template

```rust
use super::{create_test_user, setup_test_env};
use soroban_sdk::{testutils::Address as _, Address, String};
use vision_records::{Role, VisionRecordsContractClient};

#[test]
fn test_my_workflow() {
    let ctx = setup_test_env();

    // Setup
    let user = create_test_user(&ctx, Role::Patient, "User");

    // Execute
    // ... perform actions ...

    // Verify
    // ... assert results ...
}
```

### Best Practices

1. **Use descriptive test names**: Test names should clearly describe what is being tested
2. **Follow AAA pattern**: Arrange, Act, Assert
3. **Test one thing per test**: Each test should verify a single behavior
4. **Use helper functions**: Reuse common setup code
5. **Clean up**: Tests should be independent and not rely on execution order
6. **Test edge cases**: Include boundary conditions and error cases
7. **Test workflows**: Integration tests should cover complete user journeys

## Test Scenarios Covered

### Patient Scenarios
- ✅ Registration and profile management
- ✅ Granting access to family members
- ✅ Granting access to healthcare providers
- ✅ Revoking access
- ✅ Access expiration
- ✅ Viewing own records
- ✅ Managing multiple access grants

### Provider Scenarios
- ✅ Provider registration
- ✅ Provider verification
- ✅ Creating records for patients
- ✅ Viewing patient records
- ✅ Managing multiple patients
- ✅ Rate limit bypass for verified providers

### Record Scenarios
- ✅ Creating different record types
- ✅ Accessing records with different permission levels
- ✅ Record access expiration
- ✅ Record access revocation
- ✅ Multiple providers for same patient
- ✅ Audit logging for record access

### Emergency Scenarios
- ✅ Emergency access request
- ✅ Different emergency conditions
- ✅ Emergency access expiration
- ✅ Emergency access revocation
- ✅ Emergency contacts notification
- ✅ Emergency access audit trail

### End-to-End Scenarios
- ✅ Complete patient journey
- ✅ Complete provider workflow
- ✅ Complete emergency workflow
- ✅ Multi-provider collaboration
- ✅ Appointment scheduling integration
- ✅ Rate limiting integration
- ✅ Audit logging integration

## Continuous Integration

Tests are automatically run in CI/CD pipelines:

- All tests must pass before merging
- Coverage reports are generated
- Test results are published

## Troubleshooting

### Common Issues

1. **Test fails with "not initialized"**: Ensure `setup_test_env()` is called
2. **Permission errors**: Grant necessary permissions before operations
3. **Rate limit errors**: Adjust rate limits or use verified providers
4. **Timestamp issues**: Use `env.ledger().set_timestamp()` for time-dependent tests

### Debug Tips

1. Use `--nocapture` to see print statements
2. Use `RUST_BACKTRACE=1` for stack traces
3. Check event logs for detailed error information
4. Verify test data setup before assertions

## Future Enhancements

- [ ] Performance benchmarks
- [ ] Stress testing
- [ ] Fuzzing tests
- [ ] Mutation testing
- [ ] Contract upgrade testing
