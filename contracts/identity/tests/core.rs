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

#[test]
fn test_credential_issue_and_verify() {
    let mut cred = Credential::new("cred1", "did:example:issuer", "did:example:subject");
    cred.add_claim("name", "Alice");
    cred.sign(&[0u8; 32]);
    assert!(cred.verify(&[0u8; 32]));
}

#[test]
fn test_recovery_flow() {
    let mut rm = RecoveryManager::default();
    rm.add_agent(identity::recovery::RecoveryAgent {
        id: "agent1".into(),
        contact: "agent@example.com".into(),
    });
    rm.request_recovery("did:example:123", "agent1");
    let executed = rm.execute_recovery("did:example:123");
    assert_eq!(executed.unwrap(), "agent1");
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
