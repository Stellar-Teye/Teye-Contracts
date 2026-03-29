extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, BytesN, Env, String,
};

use crate::{VaultContract, VaultContractClient, VaultError};
use identity::{IdentityContract, IdentityContractClient};

// ─────────────────────────────────────────────────────────────────────────────
// Shared setup helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Register and initialise the vault + identity contracts.
/// Returns `(vault_client, identity_client, admin, owner)`.
fn setup_vault(
    env: &Env,
) -> (
    VaultContractClient<'_>,
    IdentityContractClient<'_>,
    Address,
    Address,
) {
    env.mock_all_auths();

    let admin = Address::generate(env);
    let owner = Address::generate(env);

    let identity_id = env.register(IdentityContract, ());
    let identity = IdentityContractClient::new(env, &identity_id);
    identity.initialize(&owner);

    let vault_id = env.register(VaultContract, ());
    let vault = VaultContractClient::new(env, &vault_id);
    vault.initialize(&admin, &identity_id);

    (vault, identity, admin, owner)
}

/// Add `n` guardians for `owner` in the identity contract and return them.
fn add_guardians(
    env: &Env,
    identity: &IdentityContractClient,
    owner: &Address,
    n: u32,
) -> std::vec::Vec<Address> {
    let mut guardians = std::vec::Vec::new();
    for _ in 0..n {
        let g = Address::generate(env);
        identity.add_guardian(owner, &g);
        guardians.push(g);
    }
    guardians
}

// ─────────────────────────────────────────────────────────────────────────────
// Existing smoke test (preserved)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize_and_deadman_guard() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultContract, ());
    let client = VaultContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let identity = Address::generate(&env);
    client.initialize(&admin, &identity);

    let owner = Address::generate(&env);
    let res = client.try_trigger_deadman_release(&owner);
    assert!(res.is_err());

    env.ledger().set_timestamp(1_000);
    let bytes = BytesN::from_array(&env, &[3u8; 32]);
    let _ = bytes;
}

// ─────────────────────────────────────────────────────────────────────────────
// #477 — Multi-signature threshold configuration
// ─────────────────────────────────────────────────────────────────────────────

/// threshold > shard_count is an invalid configuration.
#[test]
fn test_configure_vault_threshold_exceeds_shard_count() {
    let env = Env::default();
    let (vault, identity, _admin, owner) = setup_vault(&env);
    add_guardians(&env, &identity, &owner, 3);

    let seed = BytesN::from_array(&env, &[1u8; 32]);
    let data_ref = String::from_str(&env, "hash");

    // threshold (4) > shard_count (3) — must be rejected
    let result = vault.try_configure_vault(&owner, &4, &3, &4, &3600, &data_ref, &seed);
    assert_eq!(result, Err(Ok(VaultError::InvalidConfig)));
}

/// emergency_threshold must not be less than threshold.
#[test]
fn test_configure_vault_emergency_threshold_below_threshold() {
    let env = Env::default();
    let (vault, identity, _admin, owner) = setup_vault(&env);
    add_guardians(&env, &identity, &owner, 5);

    let seed = BytesN::from_array(&env, &[2u8; 32]);
    let data_ref = String::from_str(&env, "hash");

    // emergency_threshold (2) < threshold (3) — must be rejected
    let result = vault.try_configure_vault(&owner, &3, &5, &2, &3600, &data_ref, &seed);
    assert_eq!(result, Err(Ok(VaultError::InvalidConfig)));
}

/// zero threshold is rejected.
#[test]
fn test_configure_vault_zero_threshold_rejected() {
    let env = Env::default();
    let (vault, identity, _admin, owner) = setup_vault(&env);
    add_guardians(&env, &identity, &owner, 3);

    let seed = BytesN::from_array(&env, &[3u8; 32]);
    let data_ref = String::from_str(&env, "hash");

    let result = vault.try_configure_vault(&owner, &0, &3, &3, &3600, &data_ref, &seed);
    assert_eq!(result, Err(Ok(VaultError::InvalidConfig)));
}

/// A valid 3-of-5 multi-sig threshold configuration must succeed and store
/// a vault record with the correct policy.
#[test]
fn test_configure_vault_three_of_five_threshold_succeeds() {
    let env = Env::default();
    let (vault, identity, _admin, owner) = setup_vault(&env);
    add_guardians(&env, &identity, &owner, 5);

    let seed = BytesN::from_array(&env, &[4u8; 32]);
    let data_ref = String::from_str(&env, "ipfs://ustb");

    let snapshot = vault.configure_vault(&owner, &3, &5, &4, &86400, &data_ref, &seed);

    assert_eq!(snapshot.record.policy.threshold, 3);
    assert_eq!(snapshot.record.policy.shard_count, 5);
    assert_eq!(snapshot.record.policy.emergency_threshold, 4);
    assert_eq!(snapshot.shard_holders.len(), 5);

    // stored record should be retrievable
    let stored = vault.get_vault(&owner);
    assert_eq!(stored.policy.threshold, 3);
}

/// A valid high-threshold 3-of-5 (maximum guardian count) configuration must succeed.
#[test]
fn test_configure_vault_max_guardian_threshold_succeeds() {
    let env = Env::default();
    let (vault, identity, _admin, owner) = setup_vault(&env);
    // Identity contract caps guardians at 5.
    add_guardians(&env, &identity, &owner, 5);

    let seed = BytesN::from_array(&env, &[5u8; 32]);
    let data_ref = String::from_str(&env, "hash");

    // 4-of-5 — high threshold within the maximum allowed shard count.
    let snapshot = vault.configure_vault(&owner, &4, &5, &5, &0, &data_ref, &seed);

    assert_eq!(snapshot.record.policy.threshold, 4);
    assert_eq!(snapshot.record.policy.shard_count, 5);
    assert_eq!(snapshot.shard_holders.len(), 5);
}

// ─────────────────────────────────────────────────────────────────────────────
// #477 — Emergency approval quorum
// ─────────────────────────────────────────────────────────────────────────────

/// Attempting emergency reconstruction before any approvals are submitted must
/// return EmergencyThresholdNotMet.
#[test]
fn test_emergency_reconstruct_no_approvals_fails() {
    let env = Env::default();
    let (vault, identity, _admin, owner) = setup_vault(&env);
    add_guardians(&env, &identity, &owner, 5);

    let seed = BytesN::from_array(&env, &[6u8; 32]);
    let data_ref = String::from_str(&env, "hash");
    vault.configure_vault(&owner, &3, &5, &4, &86400, &data_ref, &seed);

    // No approvals submitted — emergency_threshold (4) is not met
    let result = vault.try_emergency_reconstruct(&owner);
    assert_eq!(result, Err(Ok(VaultError::EmergencyThresholdNotMet)));
}

/// Only guardians that hold shares may submit an emergency approval.
/// A random address that is not a guardian must be rejected.
#[test]
fn test_emergency_approval_non_guardian_rejected() {
    let env = Env::default();
    let (vault, identity, _admin, owner) = setup_vault(&env);
    add_guardians(&env, &identity, &owner, 3);

    let seed = BytesN::from_array(&env, &[7u8; 32]);
    let data_ref = String::from_str(&env, "hash");
    vault.configure_vault(&owner, &2, &3, &3, &3600, &data_ref, &seed);

    let non_guardian = Address::generate(&env);
    let result = vault.try_submit_emergency_approval(&non_guardian, &owner);
    assert_eq!(result, Err(Ok(VaultError::Unauthorized)));
}

// ─────────────────────────────────────────────────────────────────────────────
// #477 — Deadman release trigger
// ─────────────────────────────────────────────────────────────────────────────

/// Triggering the deadman switch before the inactivity timeout has elapsed
/// must return DeadmanNotReady.
#[test]
fn test_deadman_release_before_timeout_rejected() {
    let env = Env::default();
    env.ledger().set_timestamp(1_000);

    let (vault, identity, _admin, owner) = setup_vault(&env);
    add_guardians(&env, &identity, &owner, 3);

    let seed = BytesN::from_array(&env, &[8u8; 32]);
    let data_ref = String::from_str(&env, "hash");
    // inactivity_timeout_secs = 3600
    vault.configure_vault(&owner, &2, &3, &3, &3600, &data_ref, &seed);

    // Advance time — but not past the 3600-second window
    env.ledger().set_timestamp(1_000 + 1_000);

    let result = vault.try_trigger_deadman_release(&owner);
    assert_eq!(result, Err(Ok(VaultError::DeadmanNotReady)));
}

/// After the full inactivity timeout elapses, trigger_deadman_release must
/// return Ok(true).
#[test]
fn test_deadman_release_after_timeout_succeeds() {
    let env = Env::default();
    env.ledger().set_timestamp(1_000);

    let (vault, identity, _admin, owner) = setup_vault(&env);
    add_guardians(&env, &identity, &owner, 3);

    let seed = BytesN::from_array(&env, &[9u8; 32]);
    let data_ref = String::from_str(&env, "hash");
    vault.configure_vault(&owner, &2, &3, &3, &3600, &data_ref, &seed);

    // Advance time past the 3600-second inactivity window
    env.ledger().set_timestamp(1_000 + 3_601);

    let result = vault.trigger_deadman_release(&owner);
    assert!(result);
}

/// touch_activity resets the deadman timer — a trigger that would have fired
/// before the touch must be rejected after the touch extends the window.
#[test]
fn test_touch_activity_resets_deadman_timer() {
    let env = Env::default();
    env.ledger().set_timestamp(1_000);

    let (vault, identity, _admin, owner) = setup_vault(&env);
    add_guardians(&env, &identity, &owner, 3);

    let seed = BytesN::from_array(&env, &[10u8; 32]);
    let data_ref = String::from_str(&env, "hash");
    vault.configure_vault(&owner, &2, &3, &3, &3600, &data_ref, &seed);

    // Advance almost to the timeout boundary, then touch activity
    env.ledger().set_timestamp(1_000 + 3_500);
    vault.touch_activity(&owner);

    // Now advance past the original deadline — but the timer was reset
    env.ledger().set_timestamp(1_000 + 3_700);
    let result = vault.try_trigger_deadman_release(&owner);
    assert_eq!(result, Err(Ok(VaultError::DeadmanNotReady)));
}
