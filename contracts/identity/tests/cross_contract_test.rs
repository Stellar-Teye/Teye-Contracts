#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Cross-Contract Calling Invariants
//!
//! Tests that the identity contract correctly parses external contract calls
//! and handles external failures gracefully. Covers:
//!   - Mock secondary contracts that query identity state cross-contract
//!   - Identity state invariants visible across contract boundaries
//!   - Credential binding state consistency across cross-contract reads
//!   - Guardian/recovery state remains consistent across external call boundaries
//!   - Two-phase commit invariants hold when observed cross-contract
//!   - Error propagation through cross-contract boundaries

use identity::{recovery::RecoveryError, IdentityContract, IdentityContractClient};
use soroban_sdk::{
    contract, contractimpl, contracterror, testutils::Address as _, testutils::Ledger as _, Address,
    BytesN, Env,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Mock Consumer Contract — queries identity contract state cross-contract
// ═══════════════════════════════════════════════════════════════════════════════

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ConsumerError {
    OwnerNotActive = 1,
    InsufficientGuardians = 2,
    ThresholdMismatch = 3,
    CredentialNotBound = 4,
}

#[contract]
struct MockConsumerContract;

#[contractimpl]
impl MockConsumerContract {
    /// Query the identity contract to check if an owner is active.
    pub fn check_identity_active(
        env: Env,
        identity_id: Address,
        owner: Address,
    ) -> Result<bool, ConsumerError> {
        let client = IdentityContractClient::new(&env, &identity_id);
        let is_active = client.is_owner_active(&owner);
        if !is_active {
            return Err(ConsumerError::OwnerNotActive);
        }
        Ok(true)
    }

    /// Query the identity contract for guardians and validate minimum count.
    pub fn verify_guardian_count(
        env: Env,
        identity_id: Address,
        owner: Address,
        min_required: u32,
    ) -> Result<u32, ConsumerError> {
        let client = IdentityContractClient::new(&env, &identity_id);
        let guardians = client.get_guardians(&owner);
        let count = guardians.len();
        if count < min_required {
            return Err(ConsumerError::InsufficientGuardians);
        }
        Ok(count)
    }

    /// Query guardian membership from the identity contract.
    pub fn is_guardian_for(
        env: Env,
        identity_id: Address,
        owner: Address,
        guardian: Address,
    ) -> bool {
        let client = IdentityContractClient::new(&env, &identity_id);
        client.is_guardian(&owner, &guardian)
    }

    /// Query recovery threshold from the identity contract.
    pub fn get_threshold(env: Env, identity_id: Address, owner: Address) -> u32 {
        let client = IdentityContractClient::new(&env, &identity_id);
        client.get_recovery_threshold(&owner)
    }

    /// Query bound credentials count from the identity contract.
    pub fn get_credential_count(env: Env, identity_id: Address, holder: Address) -> u32 {
        let client = IdentityContractClient::new(&env, &identity_id);
        client.get_bound_credentials(&holder).len()
    }

    /// Verify a specific credential is bound, returning error if not.
    pub fn require_credential_bound(
        env: Env,
        identity_id: Address,
        holder: Address,
        credential_id: BytesN<32>,
    ) -> Result<(), ConsumerError> {
        let client = IdentityContractClient::new(&env, &identity_id);
        if !client.is_credential_bound(&holder, &credential_id) {
            return Err(ConsumerError::CredentialNotBound);
        }
        Ok(())
    }

    /// Composite cross-contract query: verify owner is active AND has enough guardians.
    pub fn validate_identity_readiness(
        env: Env,
        identity_id: Address,
        owner: Address,
        min_guardians: u32,
        expected_threshold: u32,
    ) -> Result<(), ConsumerError> {
        let client = IdentityContractClient::new(&env, &identity_id);

        if !client.is_owner_active(&owner) {
            return Err(ConsumerError::OwnerNotActive);
        }

        let guardians = client.get_guardians(&owner);
        if guardians.len() < min_guardians {
            return Err(ConsumerError::InsufficientGuardians);
        }

        let threshold = client.get_recovery_threshold(&owner);
        if threshold != expected_threshold {
            return Err(ConsumerError::ThresholdMismatch);
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Mock Failing Consumer — panics when calling identity, tests isolation
// ═══════════════════════════════════════════════════════════════════════════════

#[contract]
struct MockFailingConsumerContract;

#[contractimpl]
impl MockFailingConsumerContract {
    /// Calls identity then panics — verifies identity state is not corrupted
    /// by a consuming contract's failure.
    pub fn read_and_crash(env: Env, identity_id: Address, owner: Address) -> u32 {
        let client = IdentityContractClient::new(&env, &identity_id);
        let count = client.get_guardians(&owner).len();
        panic!("consumer crash after reading {} guardians", count);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn setup() -> (Env, IdentityContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    (env, client, contract_id, owner)
}

fn add_three_guardians(
    env: &Env,
    client: &IdentityContractClient,
    owner: &Address,
) -> (Address, Address, Address) {
    let g1 = Address::generate(env);
    let g2 = Address::generate(env);
    let g3 = Address::generate(env);

    client.add_guardian(owner, &g1);
    client.add_guardian(owner, &g2);
    client.add_guardian(owner, &g3);

    (g1, g2, g3)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests: Consumer contract correctly reads identity state cross-contract
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn consumer_reads_owner_active_status_cross_contract() {
    let (env, _client, contract_id, owner) = setup();

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    // Active owner should be detected cross-contract.
    assert!(consumer.check_identity_active(&contract_id, &owner));

    // Non-existent owner should return OwnerNotActive.
    let unknown = Address::generate(&env);
    let result = consumer.try_check_identity_active(&contract_id, &unknown);
    assert_eq!(result, Err(Ok(ConsumerError::OwnerNotActive)));
}

#[test]
fn consumer_reads_guardian_list_cross_contract() {
    let (env, client, contract_id, owner) = setup();
    let (g1, g2, g3) = add_three_guardians(&env, &client, &owner);

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    // Guardian count should be visible cross-contract.
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &1), 3);

    // Minimum requirement enforcement.
    let result = consumer.try_verify_guardian_count(&contract_id, &owner, &5);
    assert_eq!(result, Err(Ok(ConsumerError::InsufficientGuardians)));

    // Guardian membership check.
    assert!(consumer.is_guardian_for(&contract_id, &owner, &g1));
    assert!(consumer.is_guardian_for(&contract_id, &owner, &g2));
    assert!(consumer.is_guardian_for(&contract_id, &owner, &g3));

    let outsider = Address::generate(&env);
    assert!(!consumer.is_guardian_for(&contract_id, &owner, &outsider));
}

#[test]
fn consumer_reads_recovery_threshold_cross_contract() {
    let (env, client, contract_id, owner) = setup();
    let (_g1, _g2, _g3) = add_three_guardians(&env, &client, &owner);
    client.set_recovery_threshold(&owner, &2);

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    assert_eq!(consumer.get_threshold(&contract_id, &owner), 2);
}

#[test]
fn consumer_reads_bound_credentials_cross_contract() {
    let (env, client, contract_id, owner) = setup();

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    // No credentials initially.
    assert_eq!(consumer.get_credential_count(&contract_id, &owner), 0);

    // Bind a credential and verify count updates cross-contract.
    let cred = BytesN::from_array(&env, &[0xABu8; 32]);
    client.bind_credential(&owner, &cred);
    assert_eq!(consumer.get_credential_count(&contract_id, &owner), 1);

    // Bind a second credential.
    let cred2 = BytesN::from_array(&env, &[0xCDu8; 32]);
    client.bind_credential(&owner, &cred2);
    assert_eq!(consumer.get_credential_count(&contract_id, &owner), 2);

    // Unbind one and verify.
    client.unbind_credential(&owner, &cred);
    assert_eq!(consumer.get_credential_count(&contract_id, &owner), 1);
}

#[test]
fn consumer_verifies_credential_binding_cross_contract() {
    let (env, client, contract_id, owner) = setup();

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    let cred = BytesN::from_array(&env, &[0xEEu8; 32]);

    // Credential not yet bound — consumer should detect this.
    let result = consumer.try_require_credential_bound(&contract_id, &owner, &cred);
    assert_eq!(result, Err(Ok(ConsumerError::CredentialNotBound)));

    // Bind the credential.
    client.bind_credential(&owner, &cred);

    // Now the cross-contract check should succeed.
    consumer.require_credential_bound(&contract_id, &owner, &cred);

    // Unbind and verify the consumer sees it's gone.
    client.unbind_credential(&owner, &cred);
    let result = consumer.try_require_credential_bound(&contract_id, &owner, &cred);
    assert_eq!(result, Err(Ok(ConsumerError::CredentialNotBound)));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests: Composite cross-contract queries
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn composite_identity_readiness_check_cross_contract() {
    let (env, client, contract_id, owner) = setup();

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    // No guardians yet — should fail guardian check.
    let result = consumer.try_validate_identity_readiness(&contract_id, &owner, &3, &0);
    assert_eq!(result, Err(Ok(ConsumerError::InsufficientGuardians)));

    // Add guardians.
    let (_g1, _g2, _g3) = add_three_guardians(&env, &client, &owner);

    // Threshold not yet set (0) — should fail threshold check.
    let result = consumer.try_validate_identity_readiness(&contract_id, &owner, &3, &2);
    assert_eq!(result, Err(Ok(ConsumerError::ThresholdMismatch)));

    // Set threshold.
    client.set_recovery_threshold(&owner, &2);

    // Now everything should pass.
    consumer.validate_identity_readiness(&contract_id, &owner, &3, &2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests: Identity state remains consistent after recovery across contracts
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn consumer_sees_updated_owner_after_recovery_execution() {
    let (env, client, contract_id, owner) = setup();
    let (g1, g2, _g3) = add_three_guardians(&env, &client, &owner);
    client.set_recovery_threshold(&owner, &2);

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    // Owner is active before recovery.
    assert!(consumer.check_identity_active(&contract_id, &owner));

    // Execute full recovery flow.
    let new_owner = Address::generate(&env);
    client.initiate_recovery(&g1, &owner, &new_owner);
    client.approve_recovery(&g2, &owner);

    let req = client.get_recovery_request(&owner).unwrap();
    env.ledger().set_timestamp(req.execute_after + 1);

    let caller = Address::generate(&env);
    client.execute_recovery(&caller, &owner);

    // Old owner is deactivated — consumer should see this cross-contract.
    let result = consumer.try_check_identity_active(&contract_id, &owner);
    assert_eq!(result, Err(Ok(ConsumerError::OwnerNotActive)));

    // New owner is active.
    assert!(consumer.check_identity_active(&contract_id, &new_owner));

    // Guardian list is transferred to new owner.
    assert_eq!(consumer.verify_guardian_count(&contract_id, &new_owner, &1), 3);

    // Threshold is transferred to new owner.
    assert_eq!(consumer.get_threshold(&contract_id, &new_owner), 2);
}

#[test]
fn consumer_readiness_check_fails_for_deactivated_owner_after_recovery() {
    let (env, client, contract_id, owner) = setup();
    let (g1, g2, _g3) = add_three_guardians(&env, &client, &owner);
    client.set_recovery_threshold(&owner, &2);

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    // Passes before recovery.
    consumer.validate_identity_readiness(&contract_id, &owner, &3, &2);

    // Execute recovery.
    let new_owner = Address::generate(&env);
    client.initiate_recovery(&g1, &owner, &new_owner);
    client.approve_recovery(&g2, &owner);
    let req = client.get_recovery_request(&owner).unwrap();
    env.ledger().set_timestamp(req.execute_after + 1);
    let caller = Address::generate(&env);
    client.execute_recovery(&caller, &owner);

    // Old owner fails readiness check.
    let result = consumer.try_validate_identity_readiness(&contract_id, &owner, &3, &2);
    assert_eq!(result, Err(Ok(ConsumerError::OwnerNotActive)));

    // New owner passes readiness check.
    consumer.validate_identity_readiness(&contract_id, &new_owner, &3, &2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests: Identity state not corrupted by consuming contract failures
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn identity_state_survives_consumer_panic() {
    let (env, client, contract_id, owner) = setup();
    let (g1, g2, g3) = add_three_guardians(&env, &client, &owner);
    client.set_recovery_threshold(&owner, &2);

    let cred = BytesN::from_array(&env, &[0x11u8; 32]);
    client.bind_credential(&owner, &cred);

    // Register a consumer that will panic after reading identity state.
    let failing_id = env.register(MockFailingConsumerContract, ());
    let failing_consumer = MockFailingConsumerContractClient::new(&env, &failing_id);

    // The consumer panics — the cross-contract call fails.
    let result = failing_consumer.try_read_and_crash(&contract_id, &owner);
    assert!(result.is_err(), "Consumer should have panicked");

    // All identity state must remain intact after the external contract crash.
    assert!(client.is_owner_active(&owner));
    assert_eq!(client.get_guardians(&owner).len(), 3);
    assert!(client.is_guardian(&owner, &g1));
    assert!(client.is_guardian(&owner, &g2));
    assert!(client.is_guardian(&owner, &g3));
    assert_eq!(client.get_recovery_threshold(&owner), 2);
    assert!(client.is_credential_bound(&owner, &cred));
    assert_eq!(client.get_bound_credentials(&owner).len(), 1);
}

#[test]
fn repeated_consumer_panics_do_not_degrade_identity_state() {
    let (env, client, contract_id, owner) = setup();
    add_three_guardians(&env, &client, &owner);

    let failing_id = env.register(MockFailingConsumerContract, ());
    let failing_consumer = MockFailingConsumerContractClient::new(&env, &failing_id);

    // Repeatedly crash the consumer.
    for _ in 0..5 {
        let result = failing_consumer.try_read_and_crash(&contract_id, &owner);
        assert!(result.is_err());
    }

    // State should be completely unaffected.
    assert!(client.is_owner_active(&owner));
    assert_eq!(client.get_guardians(&owner).len(), 3);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests: Guardian operations visible cross-contract after state changes
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn guardian_mutations_immediately_visible_cross_contract() {
    let (env, client, contract_id, owner) = setup();

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    // Initially no guardians.
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &0), 0);

    // Add guardians one by one and verify consumer sees updates.
    let g1 = Address::generate(&env);
    client.add_guardian(&owner, &g1);
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &0), 1);
    assert!(consumer.is_guardian_for(&contract_id, &owner, &g1));

    let g2 = Address::generate(&env);
    client.add_guardian(&owner, &g2);
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &0), 2);

    let g3 = Address::generate(&env);
    client.add_guardian(&owner, &g3);
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &0), 3);

    // Set threshold — visible cross-contract.
    client.set_recovery_threshold(&owner, &2);
    assert_eq!(consumer.get_threshold(&contract_id, &owner), 2);

    // Remove a guardian — immediately visible.
    client.remove_guardian(&owner, &g1);
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &0), 2);
    assert!(!consumer.is_guardian_for(&contract_id, &owner, &g1));
    assert!(consumer.is_guardian_for(&contract_id, &owner, &g2));
    assert!(consumer.is_guardian_for(&contract_id, &owner, &g3));
}

#[test]
fn guardian_operations_functional_after_consumer_panic() {
    let (env, client, contract_id, owner) = setup();

    // Cause a consumer panic first.
    let failing_id = env.register(MockFailingConsumerContract, ());
    let failing_consumer = MockFailingConsumerContractClient::new(&env, &failing_id);
    let _ = failing_consumer.try_read_and_crash(&contract_id, &owner);

    // Guardian operations should work normally after the external failure.
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);
    client.add_guardian(&owner, &g1);
    client.add_guardian(&owner, &g2);
    client.add_guardian(&owner, &g3);

    assert_eq!(client.get_guardians(&owner).len(), 3);
    assert!(client.is_guardian(&owner, &g1));

    client.set_recovery_threshold(&owner, &2);
    assert_eq!(client.get_recovery_threshold(&owner), 2);

    client.remove_guardian(&owner, &g2);
    assert_eq!(client.get_guardians(&owner).len(), 2);
    assert!(!client.is_guardian(&owner, &g2));
}

#[test]
fn recovery_flow_functional_after_consumer_panic() {
    let (env, client, contract_id, owner) = setup();
    let (g1, g2, _g3) = add_three_guardians(&env, &client, &owner);
    client.set_recovery_threshold(&owner, &2);

    // Cause a consumer panic first.
    let failing_id = env.register(MockFailingConsumerContract, ());
    let failing_consumer = MockFailingConsumerContractClient::new(&env, &failing_id);
    let _ = failing_consumer.try_read_and_crash(&contract_id, &owner);

    // Full recovery flow should still work.
    let new_owner = Address::generate(&env);
    client.initiate_recovery(&g1, &owner, &new_owner);
    client.approve_recovery(&g2, &owner);

    let req = client.get_recovery_request(&owner).unwrap();
    env.ledger().set_timestamp(req.execute_after + 1);

    let caller = Address::generate(&env);
    let executed = client.execute_recovery(&caller, &owner);
    assert_eq!(executed, new_owner);
    assert!(!client.is_owner_active(&owner));
    assert!(client.is_owner_active(&new_owner));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests: Two-phase commit invariants hold across cross-contract boundaries
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn two_phase_commit_visible_cross_contract() {
    let (env, client, contract_id, owner) = setup();

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    let g1 = Address::generate(&env);

    // Two-phase guardian addition — visible to consumer after commit.
    client.prepare_add_guardian(&owner, &g1);
    // Before commit: guardian not yet added.
    assert!(!consumer.is_guardian_for(&contract_id, &owner, &g1));

    client.commit_add_guardian(&owner, &g1);
    // After commit: guardian visible cross-contract.
    assert!(consumer.is_guardian_for(&contract_id, &owner, &g1));
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &0), 1);

    // Add more guardians for threshold test.
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);
    client.add_guardian(&owner, &g2);
    client.add_guardian(&owner, &g3);

    // Two-phase threshold change.
    client.prepare_set_recovery_threshold(&owner, &2);
    client.commit_set_recovery_threshold(&owner, &2);
    assert_eq!(consumer.get_threshold(&contract_id, &owner), 2);

    // Two-phase guardian removal — visible after commit.
    client.prepare_remove_guardian(&owner, &g1);
    client.commit_remove_guardian(&owner, &g1);
    assert!(!consumer.is_guardian_for(&contract_id, &owner, &g1));
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &0), 2);
}

#[test]
fn two_phase_rollback_preserves_state_across_contract_boundaries() {
    let (env, client, contract_id, owner) = setup();
    let (_g1, _g2, _g3) = add_three_guardians(&env, &client, &owner);

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    // Prepare a guardian addition but rollback instead of committing.
    let g4 = Address::generate(&env);
    client.prepare_add_guardian(&owner, &g4);
    client.rollback_add_guardian(&owner, &g4);

    // Guardian should NOT be present — rollback prevented the addition.
    assert!(!consumer.is_guardian_for(&contract_id, &owner, &g4));
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &1), 3);

    // Add g4 normally then test rollback of removal.
    client.add_guardian(&owner, &g4);
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &1), 4);

    client.prepare_remove_guardian(&owner, &g4);
    client.rollback_remove_guardian(&owner, &g4);

    // Guardian should still be present — rollback preserved state.
    assert!(consumer.is_guardian_for(&contract_id, &owner, &g4));
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &1), 4);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests: Cross-contract error propagation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn unauthorized_caller_errors_propagate_through_cross_contract_queries() {
    let (env, client, contract_id, owner) = setup();

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    // An unauthorized caller cannot modify state.
    let attacker = Address::generate(&env);
    let new_guardian = Address::generate(&env);
    assert_eq!(
        client.try_add_guardian(&attacker, &new_guardian),
        Err(Ok(RecoveryError::Unauthorized))
    );

    // Cross-contract read still works — no state corruption from failed write.
    assert!(consumer.check_identity_active(&contract_id, &owner));
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &0), 0);
}

#[test]
fn duplicate_guardian_error_does_not_corrupt_cross_contract_reads() {
    let (env, client, contract_id, owner) = setup();
    let g1 = Address::generate(&env);
    client.add_guardian(&owner, &g1);

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    // Attempt to add duplicate — should fail.
    assert_eq!(
        client.try_add_guardian(&owner, &g1),
        Err(Ok(RecoveryError::DuplicateGuardian))
    );

    // State should still show exactly 1 guardian cross-contract.
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &0), 1);
    assert!(consumer.is_guardian_for(&contract_id, &owner, &g1));
}

#[test]
fn max_guardians_error_does_not_corrupt_cross_contract_reads() {
    let (env, client, contract_id, owner) = setup();

    // Add 5 guardians (max).
    for _ in 0..5 {
        let g = Address::generate(&env);
        client.add_guardian(&owner, &g);
    }

    let consumer_id = env.register(MockConsumerContract, ());
    let consumer = MockConsumerContractClient::new(&env, &consumer_id);

    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &0), 5);

    // Attempt to add a 6th — should fail.
    let extra = Address::generate(&env);
    assert_eq!(
        client.try_add_guardian(&owner, &extra),
        Err(Ok(RecoveryError::MaxGuardiansReached))
    );

    // Still exactly 5 guardians visible cross-contract.
    assert_eq!(consumer.verify_guardian_count(&contract_id, &owner, &0), 5);
    assert!(!consumer.is_guardian_for(&contract_id, &owner, &extra));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests: Multiple consumers read consistent state
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn multiple_consumers_read_consistent_state() {
    let (env, client, contract_id, owner) = setup();
    let (_g1, _g2, _g3) = add_three_guardians(&env, &client, &owner);
    client.set_recovery_threshold(&owner, &2);

    let cred = BytesN::from_array(&env, &[0xCCu8; 32]);
    client.bind_credential(&owner, &cred);

    // Register two independent consumer contracts.
    let consumer1_id = env.register(MockConsumerContract, ());
    let consumer1 = MockConsumerContractClient::new(&env, &consumer1_id);

    let consumer2_id = env.register(MockConsumerContract, ());
    let consumer2 = MockConsumerContractClient::new(&env, &consumer2_id);

    // Both consumers should see identical state.
    assert_eq!(
        consumer1.check_identity_active(&contract_id, &owner),
        consumer2.check_identity_active(&contract_id, &owner)
    );
    assert_eq!(
        consumer1.verify_guardian_count(&contract_id, &owner, &1),
        consumer2.verify_guardian_count(&contract_id, &owner, &1)
    );
    assert_eq!(
        consumer1.get_threshold(&contract_id, &owner),
        consumer2.get_threshold(&contract_id, &owner)
    );
    assert_eq!(
        consumer1.get_credential_count(&contract_id, &owner),
        consumer2.get_credential_count(&contract_id, &owner)
    );
}

#[test]
fn credential_binding_independent_of_consumer_failures() {
    let (env, client, contract_id, owner) = setup();

    // Bind credentials.
    let cred1 = BytesN::from_array(&env, &[0x44u8; 32]);
    let cred2 = BytesN::from_array(&env, &[0x55u8; 32]);
    client.bind_credential(&owner, &cred1);
    client.bind_credential(&owner, &cred2);

    // Crash a consumer contract.
    let failing_id = env.register(MockFailingConsumerContract, ());
    let failing_consumer = MockFailingConsumerContractClient::new(&env, &failing_id);
    let _ = failing_consumer.try_read_and_crash(&contract_id, &owner);

    // Bindings are preserved.
    assert!(client.is_credential_bound(&owner, &cred1));
    assert!(client.is_credential_bound(&owner, &cred2));
    assert_eq!(client.get_bound_credentials(&owner).len(), 2);

    // Can still bind/unbind after failure.
    let cred3 = BytesN::from_array(&env, &[0x77u8; 32]);
    client.bind_credential(&owner, &cred3);
    assert_eq!(client.get_bound_credentials(&owner).len(), 3);

    client.unbind_credential(&owner, &cred1);
    assert!(!client.is_credential_bound(&owner, &cred1));
    assert_eq!(client.get_bound_credentials(&owner).len(), 2);
}
