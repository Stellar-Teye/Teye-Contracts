// Integration test suite for Vision Records Contract
// These tests cover complete user workflows and end-to-end scenarios

mod emergency_workflows;
mod end_to_end;
mod patient_workflows;
mod provider_workflows;
mod record_workflows;

// Re-export common utilities for use in integration tests
pub use super::common::{create_test_user, setup_test_env};
