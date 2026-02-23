use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env,
};

use identity::{
    IdentityContract, IdentityContractClient,
    recovery::RecoveryError,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (Env, IdentityContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    (env, client, owner)
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

// ── Initialization ───────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (_env, client, owner) = setup();

    assert!(client.is_owner_active(&owner));
}

#[test]
fn test_double_initialize_fails() {
    let (env, client, _owner) = setup();

    let other = Address::generate(&env);
    let result = client.try_initialize(&other);
    match result {
        Err(Ok(e)) => assert_eq!(e, RecoveryError::AlreadyInitialized),
        _ => panic!("Expected AlreadyInitialized error"),
    }
}

// ── Guardian management ──────────────────────────────────────────────────────

#[test]
fn test_add_guardians() {
    let (env, client, owner) = setup();

    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);

    client.add_guardian(&owner, &g1);
    client.add_guardian(&owner, &g2);
    client.add_guardian(&owner, &g3);

    let guardians = client.get_guardians(&owner);
    assert_eq!(guardians.len(), 3);
    assert!(guardians.contains(&g1));
    assert!(guardians.contains(&g2));
    assert!(guardians.contains(&g3));
}

#[test]
fn test_max_five_guardians() {
    let (env, client, owner) = setup();

    for _ in 0..5 {
        let g = Address::generate(&env);
        client.add_guardian(&owner, &g);
    }
    assert_eq!(client.get_guardians(&owner).len(), 5);

    // Sixth guardian should fail
    let extra = Address::generate(&env);
    let result = client.try_add_guardian(&owner, &extra);
    match result {
        Err(Ok(e)) => assert_eq!(e, RecoveryError::MaxGuardiansReached),
        _ => panic!("Expected MaxGuardiansReached error"),
    }
}

#[test]
fn test_duplicate_guardian_fails() {
    let (env, client, owner) = setup();

    let g1 = Address::generate(&env);
    client.add_guardian(&owner, &g1);

    let result = client.try_add_guardian(&owner, &g1);
    match result {
        Err(Ok(e)) => assert_eq!(e, RecoveryError::DuplicateGuardian),
        _ => panic!("Expected DuplicateGuardian error"),
    }
}

#[test]
fn test_remove_guardian() {
    let (env, client, owner) = setup();
    let (g1, _g2, _g3) = add_three_guardians(&env, &client, &owner);

    client.remove_guardian(&owner, &g1);

    let guardians = client.get_guardians(&owner);
    assert_eq!(guardians.len(), 2);
    assert!(!guardians.contains(&g1));
}

// ── Threshold ────────────────────────────────────────────────────────────────

#[test]
fn test_set_threshold() {
    let (env, client, owner) = setup();
    add_three_guardians(&env, &client, &owner);

    client.set_recovery_threshold(&owner, &2);
    assert_eq!(client.get_recovery_threshold(&owner), 2);
}

#[test]
fn test_threshold_exceeds_guardians_fails() {
    let (env, client, owner) = setup();
    add_three_guardians(&env, &client, &owner);

    let result = client.try_set_recovery_threshold(&owner, &4);
    match result {
        Err(Ok(e)) => assert_eq!(e, RecoveryError::InvalidThreshold),
        _ => panic!("Expected InvalidThreshold error"),
    }
}

// ── Full recovery flow ───────────────────────────────────────────────────────

#[test]
fn test_initiate_approve_execute_recovery() {
    let (env, client, owner) = setup();
    let (g1, g2, _g3) = add_three_guardians(&env, &client, &owner);
    let new_address = Address::generate(&env);

    // Set 2-of-3 threshold
    client.set_recovery_threshold(&owner, &2);

    // Guardian 1 initiates recovery (first approval)
    client.initiate_recovery(&g1, &owner, &new_address);

    let request = client.get_recovery_request(&owner);
    assert!(request.is_some());
    let req = request.unwrap();
    assert_eq!(req.new_address, new_address);
    assert_eq!(req.approvals.len(), 1);

    // Guardian 2 approves (meets 2-of-3 threshold)
    client.approve_recovery(&g2, &owner);

    let req = client.get_recovery_request(&owner).unwrap();
    assert_eq!(req.approvals.len(), 2);

    // Advance time past 48-hour cooldown
    env.ledger().with_mut(|li| {
        li.timestamp = req.execute_after + 1;
    });

    // Execute recovery
    let recovered = client.execute_recovery(&new_address, &owner);
    assert_eq!(recovered, new_address);

    // Old address deactivated
    assert!(!client.is_owner_active(&owner));

    // New address active
    assert!(client.is_owner_active(&new_address));

    // Recovery request cleaned up
    assert!(client.get_recovery_request(&owner).is_none());

    // Guardians transferred to new address
    let new_guardians = client.get_guardians(&new_address);
    assert_eq!(new_guardians.len(), 3);
}

// ── Cooldown enforcement ─────────────────────────────────────────────────────

#[test]
fn test_execute_before_cooldown_fails() {
    let (env, client, owner) = setup();
    let (g1, g2, _g3) = add_three_guardians(&env, &client, &owner);
    let new_address = Address::generate(&env);

    client.set_recovery_threshold(&owner, &2);
    client.initiate_recovery(&g1, &owner, &new_address);
    client.approve_recovery(&g2, &owner);

    // Try to execute immediately (before cooldown)
    let caller = Address::generate(&env);
    let result = client.try_execute_recovery(&caller, &owner);
    match result {
        Err(Ok(e)) => assert_eq!(e, RecoveryError::CooldownNotExpired),
        _ => panic!("Expected CooldownNotExpired error"),
    }
}

// ── Insufficient approvals ───────────────────────────────────────────────────

#[test]
fn test_execute_insufficient_approvals_fails() {
    let (env, client, owner) = setup();
    let (g1, _g2, _g3) = add_three_guardians(&env, &client, &owner);
    let new_address = Address::generate(&env);

    // Set 3-of-3 threshold
    client.set_recovery_threshold(&owner, &3);

    // Only one approval
    client.initiate_recovery(&g1, &owner, &new_address);

    let req = client.get_recovery_request(&owner).unwrap();
    env.ledger().with_mut(|li| {
        li.timestamp = req.execute_after + 1;
    });

    let caller = Address::generate(&env);
    let result = client.try_execute_recovery(&caller, &owner);
    match result {
        Err(Ok(e)) => assert_eq!(e, RecoveryError::InsufficientApprovals),
        _ => panic!("Expected InsufficientApprovals error"),
    }
}

// ── Cancellation ─────────────────────────────────────────────────────────────

#[test]
fn test_cancel_recovery() {
    let (env, client, owner) = setup();
    let (g1, _g2, _g3) = add_three_guardians(&env, &client, &owner);
    let new_address = Address::generate(&env);

    client.set_recovery_threshold(&owner, &2);
    client.initiate_recovery(&g1, &owner, &new_address);
    assert!(client.get_recovery_request(&owner).is_some());

    // Owner cancels
    client.cancel_recovery(&owner);
    assert!(client.get_recovery_request(&owner).is_none());

    // Owner is still active
    assert!(client.is_owner_active(&owner));
}

// ── Non-guardian cannot initiate ─────────────────────────────────────────────

#[test]
fn test_non_guardian_cannot_initiate() {
    let (env, client, owner) = setup();
    add_three_guardians(&env, &client, &owner);

    client.set_recovery_threshold(&owner, &2);

    let impostor = Address::generate(&env);
    let new_address = Address::generate(&env);
    let result = client.try_initiate_recovery(&impostor, &owner, &new_address);
    match result {
        Err(Ok(e)) => assert_eq!(e, RecoveryError::NotAGuardian),
        _ => panic!("Expected NotAGuardian error"),
    }
}

// ── Duplicate approval rejected ──────────────────────────────────────────────

#[test]
fn test_duplicate_approval_fails() {
    let (env, client, owner) = setup();
    let (g1, _g2, _g3) = add_three_guardians(&env, &client, &owner);
    let new_address = Address::generate(&env);

    client.set_recovery_threshold(&owner, &2);
    client.initiate_recovery(&g1, &owner, &new_address);

    // Guardian 1 already approved via initiation; second approval should fail
    let result = client.try_approve_recovery(&g1, &owner);
    match result {
        Err(Ok(e)) => assert_eq!(e, RecoveryError::AlreadyApproved),
        _ => panic!("Expected AlreadyApproved error"),
    }
}
