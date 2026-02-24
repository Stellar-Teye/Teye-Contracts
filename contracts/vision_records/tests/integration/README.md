# Integration Test Suite

This directory contains comprehensive integration tests for the Vision Records contract, covering complete user workflows and end-to-end scenarios.

## Test Files

### `patient_workflows.rs`
Tests for patient registration and management workflows:
- Patient registration
- Granting access to family members
- Revoking access
- Managing multiple access grants
- Access expiration
- Viewing record lists

### `provider_workflows.rs`
Tests for provider onboarding and management:
- Complete provider onboarding
- Provider registration with credentials
- Provider verification workflow
- Creating records workflow
- Searching for patients
- Rate limit bypass for verified providers

### `record_workflows.rs`
Tests for record creation and access:
- Record creation workflow
- Record access by different users
- Multiple record types
- Access levels (Read, Write, Full)
- Access expiration and revocation
- Multiple providers for same patient
- Audit logging

### `emergency_workflows.rs`
Tests for emergency access scenarios:
- Complete emergency access workflow
- Different emergency conditions
- Emergency access expiration
- Emergency access revocation
- Multiple emergency contacts
- Emergency access audit trail
- Verification requirements

### `end_to_end.rs`
End-to-end workflow tests:
- Complete patient journey
- Complete provider workflow
- Complete emergency workflow
- Multi-provider collaboration
- Appointment scheduling integration
- Rate limiting integration
- Audit logging integration

## Running Tests

```bash
# Run all integration tests
cargo test --test integration

# Run specific test file
cargo test --test integration patient_workflows

# Run with output
cargo test --test integration -- --nocapture
```

## Test Coverage

These integration tests aim to achieve >90% code coverage by testing:
- All user workflows from start to finish
- Edge cases and error scenarios
- Integration between different contract features
- Real-world usage patterns

## Adding New Tests

When adding new integration tests:

1. Place tests in the appropriate workflow file
2. Use descriptive test names that explain the scenario
3. Follow the AAA pattern (Arrange, Act, Assert)
4. Use helper functions from `common/mod.rs`
5. Test complete workflows, not just individual functions
6. Include both success and failure scenarios

## Example Test

```rust
#[test]
fn test_patient_grant_access_workflow() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let family = create_test_user(&ctx, Role::Patient, "Family");

    // Grant access
    ctx.client.grant_access(
        &patient,
        &patient,
        &family,
        &AccessLevel::Read,
        &3600u64,
    );

    // Verify access
    let access_level = ctx.client.check_access(&patient, &family);
    assert_eq!(access_level, AccessLevel::Read);
}
```
