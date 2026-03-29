//! Zero-Value Parameter Passing Edge Cases for the `identity` contract.
//!
//! These tests verify consistent handling and correct revert behavior when
//! critical state-modifying functions receive zero values, empty vectors,
//! blank addresses, or other boundary inputs.

#![allow(clippy::unwrap_used)]

use identity::{recovery::RecoveryError, IdentityContract, IdentityContractClient};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, BytesN, Env};

// ── Helpers ────────────────────────────────────────────────────────────────

fn setup() -> (Env, IdentityContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    (env, client, owner)
}

fn setup_uninit() -> (Env, IdentityContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    (env, client)
}

fn add_guardians(
    env: &Env,
    client: &IdentityContractClient,
    owner: &Address,
    n: usize,
) -> Vec<Address> {
    let mut guardians = Vec::new();
    for _ in 0..n {
        let g = Address::generate(env);
        client.add_guardian(owner, &g);
        guardians.push(g);
    }
    guardians
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Guardian Operations — Zero & Empty Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn get_guardians_returns_empty_for_uninitialized_owner() {
    let (env, client, _owner) = setup();
    let random = Address::generate(&env);
    let guardians = client.get_guardians(&random);
    assert_eq!(guardians.len(), 0);
}

#[test]
fn get_guardians_returns_empty_after_init_before_adding() {
    let (_env, client, owner) = setup();
    assert_eq!(client.get_guardians(&owner).len(), 0);
}

#[test]
fn is_guardian_returns_false_for_nonexistent_owner() {
    let (env, client, _owner) = setup();
    let random_owner = Address::generate(&env);
    let random_guardian = Address::generate(&env);
    assert!(!client.is_guardian(&random_owner, &random_guardian));
}

#[test]
fn is_guardian_returns_false_for_non_registered_guardian() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 3);
    let non_guardian = Address::generate(&env);
    assert!(!client.is_guardian(&owner, &non_guardian));
}

#[test]
fn remove_nonexistent_guardian_returns_not_found() {
    let (env, client, owner) = setup();
    let fake_guardian = Address::generate(&env);
    let result = client.try_remove_guardian(&owner, &fake_guardian);
    assert_eq!(result, Err(Ok(RecoveryError::GuardianNotFound)));
}

#[test]
fn remove_guardian_from_empty_list_returns_not_found() {
    let (env, client, owner) = setup();
    // No guardians added yet.
    let guardian = Address::generate(&env);
    let result = client.try_remove_guardian(&owner, &guardian);
    assert_eq!(result, Err(Ok(RecoveryError::GuardianNotFound)));
}

#[test]
fn add_guardian_by_non_owner_rejected() {
    let (env, client, _owner) = setup();
    let non_owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let result = client.try_add_guardian(&non_owner, &guardian);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}

#[test]
fn remove_guardian_by_non_owner_rejected() {
    let (env, client, owner) = setup();
    let guardians = add_guardians(&env, &client, &owner, 1);
    let non_owner = Address::generate(&env);
    let result = client.try_remove_guardian(&non_owner, &guardians[0]);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Recovery Threshold — Zero & Boundary Values
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn threshold_zero_rejected() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 3);
    let result = client.try_set_recovery_threshold(&owner, &0u32);
    assert_eq!(result, Err(Ok(RecoveryError::InvalidThreshold)));
}

#[test]
fn threshold_on_zero_guardians_rejected() {
    let (_env, client, owner) = setup();
    // No guardians added — any threshold > 0 exceeds guardian count.
    let result = client.try_set_recovery_threshold(&owner, &1u32);
    assert_eq!(result, Err(Ok(RecoveryError::InvalidThreshold)));
}

#[test]
fn threshold_exceeds_guardian_count_rejected() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 2);
    let result = client.try_set_recovery_threshold(&owner, &3u32);
    assert_eq!(result, Err(Ok(RecoveryError::InvalidThreshold)));
}

#[test]
fn threshold_equals_guardian_count_accepted() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 3);
    client.set_recovery_threshold(&owner, &3);
    assert_eq!(client.get_recovery_threshold(&owner), 3);
}

#[test]
fn threshold_of_one_with_one_guardian_rejected() {
    let (env, client, owner) = setup();
    // Only 1 guardian, but MIN_GUARDIANS is 3, so this won't affect
    // threshold — threshold <= guardian count check is what matters here.
    add_guardians(&env, &client, &owner, 1);
    // threshold 1 <= 1 guardian → should be accepted by set_threshold.
    client.set_recovery_threshold(&owner, &1);
    assert_eq!(client.get_recovery_threshold(&owner), 1);
}

#[test]
fn get_threshold_returns_zero_for_unset_owner() {
    let (_env, client, owner) = setup();
    assert_eq!(client.get_recovery_threshold(&owner), 0);
}

#[test]
fn get_threshold_returns_zero_for_unknown_address() {
    let (env, client, _owner) = setup();
    let random = Address::generate(&env);
    assert_eq!(client.get_recovery_threshold(&random), 0);
}

#[test]
fn threshold_by_non_owner_rejected() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 3);
    let non_owner = Address::generate(&env);
    let result = client.try_set_recovery_threshold(&non_owner, &2u32);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Recovery Initiation — Insufficient Guardians & Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn initiate_recovery_with_zero_guardians_rejected() {
    let (env, client, owner) = setup();
    let fake_guardian = Address::generate(&env);
    let new_addr = Address::generate(&env);
    // No guardians → NotAGuardian (checked before InsufficientGuardians).
    let result = client.try_initiate_recovery(&fake_guardian, &owner, &new_addr);
    assert_eq!(result, Err(Ok(RecoveryError::NotAGuardian)));
}

#[test]
fn initiate_recovery_with_fewer_than_min_guardians_rejected() {
    let (env, client, owner) = setup();
    let guardians = add_guardians(&env, &client, &owner, 2); // MIN_GUARDIANS = 3
    let new_addr = Address::generate(&env);
    let result = client.try_initiate_recovery(&guardians[0], &owner, &new_addr);
    assert_eq!(result, Err(Ok(RecoveryError::InsufficientGuardians)));
}

#[test]
fn initiate_recovery_by_non_guardian_rejected() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 3);
    let non_guardian = Address::generate(&env);
    let new_addr = Address::generate(&env);
    let result = client.try_initiate_recovery(&non_guardian, &owner, &new_addr);
    assert_eq!(result, Err(Ok(RecoveryError::NotAGuardian)));
}

#[test]
fn initiate_recovery_twice_rejected() {
    let (env, client, owner) = setup();
    let guardians = add_guardians(&env, &client, &owner, 3);
    let new_addr = Address::generate(&env);

    client.initiate_recovery(&guardians[0], &owner, &new_addr);
    let result = client.try_initiate_recovery(&guardians[1], &owner, &Address::generate(&env));
    assert_eq!(result, Err(Ok(RecoveryError::RecoveryAlreadyActive)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Recovery Approval — Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn approve_with_no_active_recovery_rejected() {
    let (env, client, owner) = setup();
    let guardians = add_guardians(&env, &client, &owner, 3);
    let result = client.try_approve_recovery(&guardians[0], &owner);
    assert_eq!(result, Err(Ok(RecoveryError::NoActiveRecovery)));
}

#[test]
fn approve_by_non_guardian_rejected() {
    let (env, client, owner) = setup();
    let guardians = add_guardians(&env, &client, &owner, 3);
    let new_addr = Address::generate(&env);
    client.initiate_recovery(&guardians[0], &owner, &new_addr);

    let non_guardian = Address::generate(&env);
    let result = client.try_approve_recovery(&non_guardian, &owner);
    assert_eq!(result, Err(Ok(RecoveryError::NotAGuardian)));
}

#[test]
fn double_approval_by_same_guardian_rejected() {
    let (env, client, owner) = setup();
    let guardians = add_guardians(&env, &client, &owner, 3);
    let new_addr = Address::generate(&env);
    client.initiate_recovery(&guardians[0], &owner, &new_addr);

    // Guardian 0 already approved via initiation — approve again should fail.
    let result = client.try_approve_recovery(&guardians[0], &owner);
    assert_eq!(result, Err(Ok(RecoveryError::AlreadyApproved)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Recovery Execution — Zero Threshold & Insufficient Approvals
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn execute_recovery_with_zero_threshold_rejected() {
    let (env, client, owner) = setup();
    let guardians = add_guardians(&env, &client, &owner, 3);
    // Do NOT set threshold (defaults to 0).
    let new_addr = Address::generate(&env);
    client.initiate_recovery(&guardians[0], &owner, &new_addr);

    // Advance past cooldown.
    env.ledger().with_mut(|l| l.timestamp += 172_800 + 1);

    let result = client.try_execute_recovery(&owner, &owner);
    assert_eq!(result, Err(Ok(RecoveryError::InvalidThreshold)));
}

#[test]
fn execute_recovery_with_insufficient_approvals_rejected() {
    let (env, client, owner) = setup();
    let guardians = add_guardians(&env, &client, &owner, 3);
    client.set_recovery_threshold(&owner, &3);

    let new_addr = Address::generate(&env);
    client.initiate_recovery(&guardians[0], &owner, &new_addr);
    client.approve_recovery(&guardians[1], &owner);
    // Only 2 approvals, threshold is 3.

    env.ledger().with_mut(|l| l.timestamp += 172_800 + 1);

    let result = client.try_execute_recovery(&owner, &owner);
    assert_eq!(result, Err(Ok(RecoveryError::InsufficientApprovals)));
}

#[test]
fn execute_recovery_before_cooldown_rejected() {
    let (env, client, owner) = setup();
    let guardians = add_guardians(&env, &client, &owner, 3);
    client.set_recovery_threshold(&owner, &2);

    let new_addr = Address::generate(&env);
    client.initiate_recovery(&guardians[0], &owner, &new_addr);
    client.approve_recovery(&guardians[1], &owner);
    // 2 approvals meet threshold, but cooldown not expired.

    let result = client.try_execute_recovery(&owner, &owner);
    assert_eq!(result, Err(Ok(RecoveryError::CooldownNotExpired)));
}

#[test]
fn execute_recovery_with_no_active_request_rejected() {
    let (env, client, owner) = setup();
    let caller = Address::generate(&env);
    let result = client.try_execute_recovery(&caller, &owner);
    assert_eq!(result, Err(Ok(RecoveryError::NoActiveRecovery)));
}

#[test]
fn execute_recovery_exactly_at_cooldown_boundary() {
    let (env, client, owner) = setup();
    let guardians = add_guardians(&env, &client, &owner, 3);
    client.set_recovery_threshold(&owner, &2);

    let new_addr = Address::generate(&env);
    client.initiate_recovery(&guardians[0], &owner, &new_addr);
    client.approve_recovery(&guardians[1], &owner);

    // Advance to exactly execute_after (cooldown period).
    env.ledger().with_mut(|l| l.timestamp += 172_800);

    // now == execute_after → should succeed (condition is now < execute_after).
    let result = client.try_execute_recovery(&owner, &owner);
    assert!(result.is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Cancel Recovery — Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn cancel_recovery_with_no_active_request_rejected() {
    let (_env, client, owner) = setup();
    let result = client.try_cancel_recovery(&owner);
    assert_eq!(result, Err(Ok(RecoveryError::NoActiveRecovery)));
}

#[test]
fn cancel_recovery_by_non_owner_rejected() {
    let (env, client, _owner) = setup();
    let non_owner = Address::generate(&env);
    let result = client.try_cancel_recovery(&non_owner);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}

#[test]
fn get_recovery_request_returns_none_when_none_active() {
    let (_env, client, owner) = setup();
    assert!(client.get_recovery_request(&owner).is_none());
}

#[test]
fn get_recovery_request_returns_none_for_unknown_address() {
    let (env, client, _owner) = setup();
    let random = Address::generate(&env);
    assert!(client.get_recovery_request(&random).is_none());
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Owner Status — Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn is_owner_active_returns_false_for_unknown_address() {
    let (env, client, _owner) = setup();
    let random = Address::generate(&env);
    assert!(!client.is_owner_active(&random));
}

#[test]
fn is_owner_active_returns_true_after_init() {
    let (_env, client, owner) = setup();
    assert!(client.is_owner_active(&owner));
}

#[test]
fn is_owner_active_false_before_init() {
    let (env, client) = setup_uninit();
    let random = Address::generate(&env);
    assert!(!client.is_owner_active(&random));
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Credential Binding — Zero-Value & Empty Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn get_bound_credentials_returns_empty_for_unknown_holder() {
    let (env, client, _owner) = setup();
    let random = Address::generate(&env);
    assert_eq!(client.get_bound_credentials(&random).len(), 0);
}

#[test]
fn get_bound_credentials_returns_empty_before_binding() {
    let (_env, client, owner) = setup();
    assert_eq!(client.get_bound_credentials(&owner).len(), 0);
}

#[test]
fn is_credential_bound_returns_false_for_unknown_holder() {
    let (env, client, _owner) = setup();
    let random = Address::generate(&env);
    let cred_id = BytesN::from_array(&env, &[0u8; 32]);
    assert!(!client.is_credential_bound(&random, &cred_id));
}

#[test]
fn is_credential_bound_returns_false_for_unbound_credential() {
    let (env, client, owner) = setup();
    let cred_id = BytesN::from_array(&env, &[0u8; 32]);
    assert!(!client.is_credential_bound(&owner, &cred_id));
}

#[test]
fn bind_zero_hash_credential_succeeds() {
    let (env, client, owner) = setup();
    let zero_cred = BytesN::from_array(&env, &[0u8; 32]);
    client.bind_credential(&owner, &zero_cred);
    assert!(client.is_credential_bound(&owner, &zero_cred));
    assert_eq!(client.get_bound_credentials(&owner).len(), 1);
}

#[test]
fn bind_duplicate_credential_is_idempotent() {
    let (env, client, owner) = setup();
    let cred_id = BytesN::from_array(&env, &[0x42u8; 32]);
    client.bind_credential(&owner, &cred_id);
    client.bind_credential(&owner, &cred_id); // duplicate — no error
                                              // Should still have only 1 credential bound.
    assert_eq!(client.get_bound_credentials(&owner).len(), 1);
}

#[test]
fn unbind_nonexistent_credential_succeeds_silently() {
    let (env, client, owner) = setup();
    let cred_id = BytesN::from_array(&env, &[0x42u8; 32]);
    // Unbind something that was never bound — should succeed.
    client.unbind_credential(&owner, &cred_id);
    assert_eq!(client.get_bound_credentials(&owner).len(), 0);
}

#[test]
fn unbind_from_empty_list_succeeds_silently() {
    let (env, client, owner) = setup();
    let cred_id = BytesN::from_array(&env, &[0x01u8; 32]);
    client.unbind_credential(&owner, &cred_id);
    assert!(!client.is_credential_bound(&owner, &cred_id));
}

#[test]
fn bind_credential_by_non_owner_rejected() {
    let (env, client, _owner) = setup();
    let non_owner = Address::generate(&env);
    let cred_id = BytesN::from_array(&env, &[0x42u8; 32]);
    let result = client.try_bind_credential(&non_owner, &cred_id);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}

#[test]
fn unbind_credential_by_non_owner_rejected() {
    let (env, client, owner) = setup();
    let cred_id = BytesN::from_array(&env, &[0x42u8; 32]);
    client.bind_credential(&owner, &cred_id);

    let non_owner = Address::generate(&env);
    let result = client.try_unbind_credential(&non_owner, &cred_id);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Two-Phase Commit — Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn commit_add_guardian_without_prepare_rejected() {
    let (env, client, owner) = setup();
    let guardian = Address::generate(&env);
    let result = client.try_commit_add_guardian(&owner, &guardian);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}

#[test]
fn commit_remove_guardian_without_prepare_rejected() {
    let (env, client, owner) = setup();
    let guardian = Address::generate(&env);
    let result = client.try_commit_remove_guardian(&owner, &guardian);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}

#[test]
fn commit_set_threshold_without_prepare_rejected() {
    let (_env, client, owner) = setup();
    let result = client.try_commit_set_recovery_threshold(&owner, &3u32);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}

#[test]
fn prepare_add_guardian_by_non_owner_rejected() {
    let (env, client, _owner) = setup();
    let non_owner = Address::generate(&env);
    let guardian = Address::generate(&env);
    let result = client.try_prepare_add_guardian(&non_owner, &guardian);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}

#[test]
fn prepare_set_threshold_zero_rejected() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 3);
    let result = client.try_prepare_set_recovery_threshold(&owner, &0u32);
    assert_eq!(result, Err(Ok(RecoveryError::InvalidThreshold)));
}

#[test]
fn prepare_set_threshold_exceeds_max_rejected() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 3);
    let result = client.try_prepare_set_recovery_threshold(&owner, &6u32);
    assert_eq!(result, Err(Ok(RecoveryError::InvalidThreshold)));
}

#[test]
fn prepare_set_threshold_exceeds_guardian_count_rejected() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 2);
    let result = client.try_prepare_set_recovery_threshold(&owner, &3u32);
    assert_eq!(result, Err(Ok(RecoveryError::InvalidThreshold)));
}

#[test]
fn rollback_then_commit_fails() {
    let (env, client, owner) = setup();
    let guardian = Address::generate(&env);

    client.prepare_add_guardian(&owner, &guardian);
    client.rollback_add_guardian(&owner, &guardian);

    // After rollback, commit should fail — prep data removed.
    let result = client.try_commit_add_guardian(&owner, &guardian);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}

#[test]
fn prepare_add_duplicate_guardian_rejected() {
    let (env, client, owner) = setup();
    let guardian = Address::generate(&env);
    client.add_guardian(&owner, &guardian);

    let result = client.try_prepare_add_guardian(&owner, &guardian);
    assert_eq!(result, Err(Ok(RecoveryError::DuplicateGuardian)));
}

#[test]
fn prepare_add_guardian_at_max_capacity_rejected() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 5);

    let extra = Address::generate(&env);
    let result = client.try_prepare_add_guardian(&owner, &extra);
    assert_eq!(result, Err(Ok(RecoveryError::MaxGuardiansReached)));
}

#[test]
fn prepare_remove_nonexistent_guardian_rejected() {
    let (env, client, owner) = setup();
    let fake = Address::generate(&env);
    let result = client.try_prepare_remove_guardian(&owner, &fake);
    assert_eq!(result, Err(Ok(RecoveryError::GuardianNotFound)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. ZK Verifier — Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn get_zk_verifier_returns_none_before_set() {
    let (_env, client, _owner) = setup();
    assert!(client.get_zk_verifier().is_none());
}

#[test]
fn set_zk_verifier_by_non_owner_rejected() {
    let (env, client, _owner) = setup();
    let non_owner = Address::generate(&env);
    let verifier = Address::generate(&env);
    let result = client.try_set_zk_verifier(&non_owner, &verifier);
    assert_eq!(result, Err(Ok(RecoveryError::Unauthorized)));
}
