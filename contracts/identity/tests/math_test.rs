#![allow(clippy::unwrap_used, clippy::expect_used)]

use identity::{recovery::RecoveryError, IdentityContract, IdentityContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, IdentityContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    client.initialize(&owner);
    (env, client, owner)
}

fn add_guardians(env: &Env, client: &IdentityContractClient, owner: &Address, n: usize) {
    for _ in 0..n {
        client.add_guardian(owner, &Address::generate(env));
    }
}

// ── Threshold boundary tests ──────────────────────────────────────────────────

#[test]
fn test_threshold_zero_rejected() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 3);
    assert_eq!(
        client.try_set_recovery_threshold(&owner, &0u32),
        Err(Ok(RecoveryError::InvalidThreshold)),
        "threshold=0 must return InvalidThreshold"
    );
}

#[test]
fn test_threshold_u32_max_rejected() {
    let (env, client, owner) = setup();
    // Only 3 guardians; u32::MAX vastly exceeds guardian count.
    add_guardians(&env, &client, &owner, 3);
    assert_eq!(
        client.try_set_recovery_threshold(&owner, &u32::MAX),
        Err(Ok(RecoveryError::InvalidThreshold)),
        "threshold=u32::MAX must be rejected when guardian count < u32::MAX"
    );
}

#[test]
fn test_threshold_exceeds_guardian_count_rejected() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 3);
    // Threshold of 4 exceeds 3 guardians.
    assert_eq!(
        client.try_set_recovery_threshold(&owner, &4u32),
        Err(Ok(RecoveryError::InvalidThreshold)),
        "threshold exceeding guardian count must return InvalidThreshold"
    );
}

#[test]
fn test_threshold_at_guardian_count_accepted() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 3);
    // Threshold equal to guardian count is the maximum valid value.
    assert_eq!(
        client.try_set_recovery_threshold(&owner, &3u32),
        Ok(Ok(())),
        "threshold == guardian count must be accepted"
    );
    assert_eq!(client.get_recovery_threshold(&owner), 3);
}

// ── Guardian count boundary tests ─────────────────────────────────────────────

#[test]
fn test_max_guardians_boundary() {
    let (env, client, owner) = setup();
    // MAX_GUARDIANS is 5; fill to capacity.
    add_guardians(&env, &client, &owner, 5);
    assert_eq!(client.get_guardians(&owner).len(), 5);
}

#[test]
fn test_exceeding_max_guardians_rejected() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 5);
    // The 6th guardian must be rejected with MaxGuardiansReached.
    assert_eq!(
        client.try_add_guardian(&owner, &Address::generate(&env)),
        Err(Ok(RecoveryError::MaxGuardiansReached)),
        "adding a 6th guardian must return MaxGuardiansReached"
    );
}

#[test]
fn test_max_guardian_count_then_max_threshold() {
    let (env, client, owner) = setup();
    add_guardians(&env, &client, &owner, 5);
    // Set threshold to 5 (== guardian count) — must succeed.
    assert_eq!(
        client.try_set_recovery_threshold(&owner, &5u32),
        Ok(Ok(())),
        "threshold == MAX_GUARDIANS must be accepted"
    );
    // Setting threshold to 6 exceeds guardian count — must fail.
    assert_eq!(
        client.try_set_recovery_threshold(&owner, &6u32),
        Err(Ok(RecoveryError::InvalidThreshold)),
        "threshold > MAX_GUARDIANS must return InvalidThreshold"
    );
}

#[test]
fn test_duplicate_guardian_rejected() {
    let (env, client, owner) = setup();
    let guardian = Address::generate(&env);
    client.add_guardian(&owner, &guardian);
    // Adding the same guardian again must be rejected.
    assert_eq!(
        client.try_add_guardian(&owner, &guardian),
        Err(Ok(RecoveryError::DuplicateGuardian)),
        "duplicate guardian must be rejected"
    );
}
