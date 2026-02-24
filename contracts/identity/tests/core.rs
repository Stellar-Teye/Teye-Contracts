#![allow(clippy::unwrap_used, clippy::expect_used)]
use identity::credential::Credential;
use identity::did::{DIDDocument, DIDError, DIDRegistry, VerificationMethod}; // make it one line
use identity::recovery::RecoveryManager;

#[test]
fn test_did_register_and_resolve() {
    let mut reg = DIDRegistry::default();
    let mut doc = DIDDocument::new("did:example:123").expect("valid DID");
    doc.add_verification_method(VerificationMethod {
        id: "vm1".into(),
        type_: "Ed25519VerificationKey2018".into(),
        public_key: vec![1, 2, 3],
    });
    reg.register(doc).expect("first registration should succeed");
    let resolved = reg.resolve("did:example:123").expect("should resolve");
    assert_eq!(resolved.id, "did:example:123");
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

// DID format validation tests

#[test]
fn test_did_missing_prefix_rejected() {
    let result = DIDDocument::new("notadid:example:123");
    assert_eq!(result.unwrap_err(), DIDError::MissingPrefix);
}

#[test]
fn test_did_missing_method_rejected() {
    let result = DIDDocument::new("did:");
    assert_eq!(result.unwrap_err(), DIDError::MissingMethod);
}

#[test]
fn test_did_missing_identifier_rejected() {
    let result = DIDDocument::new("did:example:");
    assert_eq!(result.unwrap_err(), DIDError::MissingIdentifier);
}

#[test]
fn test_did_no_colon_after_method() {
    let result = DIDDocument::new("did:example");
    assert_eq!(result.unwrap_err(), DIDError::MissingMethod);
}

#[test]
fn test_did_invalid_method_chars_rejected() {
    // Method should be lowercase alphanumeric only
    let result = DIDDocument::new("did:EXAMPLE:123");
    assert_eq!(result.unwrap_err(), DIDError::InvalidCharacters);
}

#[test]
fn test_did_invalid_id_chars_rejected() {
    // Spaces are not allowed in the identifier
    let result = DIDDocument::new("did:example:invalid id");
    assert_eq!(result.unwrap_err(), DIDError::InvalidCharacters);
}

#[test]
fn test_did_valid_complex_id() {
    // Colons, dots, hyphens, underscores & percent-encoding are allowed in id
    let doc = DIDDocument::new("did:web:example.com%3A443:path:sub").expect("valid DID");
    assert_eq!(doc.id, "did:web:example.com%3A443:path:sub");
}

#[test]
fn test_registry_duplicate_rejected() {
    let mut reg = DIDRegistry::default();
    let doc1 = DIDDocument::new("did:example:dup").expect("valid");
    let doc2 = DIDDocument::new("did:example:dup").expect("valid");
    reg.register(doc1).expect("first should succeed");
    assert_eq!(reg.register(doc2).unwrap_err(), DIDError::AlreadyRegistered);
}

#[test]
fn test_registry_resolve_not_found() {
    let reg = DIDRegistry::default();
    assert_eq!(
        reg.resolve("did:example:nonexistent").unwrap_err(),
        DIDError::NotFound,
    );
}

#[test]
fn test_register_rejects_manually_constructed_invalid_doc() {
    use std::collections::HashMap;

    // Bypass DIDDocument::new() by constructing directly with public fields
    let bad_doc = DIDDocument {
        id: "not-a-did".to_string(),
        controller: None,
        verification_methods: HashMap::new(),
    };

    let mut reg = DIDRegistry::default();
    assert_eq!(
        reg.register(bad_doc).unwrap_err(),
        DIDError::MissingPrefix,
    );
}

// DID format validation tests

#[test]
fn test_did_missing_prefix_rejected() {
    let result = DIDDocument::new("notadid:example:123");
    assert_eq!(result.unwrap_err(), DIDError::MissingPrefix);
}

#[test]
fn test_did_missing_method_rejected() {
    let result = DIDDocument::new("did:");
    assert_eq!(result.unwrap_err(), DIDError::MissingMethod);
}

#[test]
fn test_did_missing_identifier_rejected() {
    let result = DIDDocument::new("did:example:");
    assert_eq!(result.unwrap_err(), DIDError::MissingIdentifier);
}

#[test]
fn test_did_no_colon_after_method() {
    let result = DIDDocument::new("did:example");
    assert_eq!(result.unwrap_err(), DIDError::MissingMethod);
}

#[test]
fn test_did_invalid_method_chars_rejected() {
    // Method should be lowercase alphanumeric only
    let result = DIDDocument::new("did:EXAMPLE:123");
    assert_eq!(result.unwrap_err(), DIDError::InvalidCharacters);
}

#[test]
fn test_did_invalid_id_chars_rejected() {
    // Spaces are not allowed in the identifier
    let result = DIDDocument::new("did:example:invalid id");
    assert_eq!(result.unwrap_err(), DIDError::InvalidCharacters);
}

#[test]
fn test_did_valid_complex_id() {
    // Colons, dots, hyphens, underscores & percent-encoding are allowed in id
    let doc = DIDDocument::new("did:web:example.com%3A443:path:sub").expect("valid DID");
    assert_eq!(doc.id, "did:web:example.com%3A443:path:sub");
}

#[test]
fn test_registry_duplicate_rejected() {
    let mut reg = DIDRegistry::default();
    let doc1 = DIDDocument::new("did:example:dup").expect("valid");
    let doc2 = DIDDocument::new("did:example:dup").expect("valid");
    reg.register(doc1).expect("first should succeed");
    assert_eq!(reg.register(doc2).unwrap_err(), DIDError::AlreadyRegistered);
}

#[test]
fn test_registry_resolve_not_found() {
    let reg = DIDRegistry::default();
    assert_eq!(
        reg.resolve("did:example:nonexistent").unwrap_err(),
        DIDError::NotFound,
    );
}
