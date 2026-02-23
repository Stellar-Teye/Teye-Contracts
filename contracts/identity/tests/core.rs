#![allow(clippy::unwrap_used, clippy::expect_used)]
use identity::credential::Credential;
use identity::did::DIDDocument;
use identity::did::DIDRegistry;
use identity::did::VerificationMethod;
use identity::recovery::RecoveryManager;

#[test]
fn test_did_register_and_resolve() {
    let mut reg = DIDRegistry::default();
    let mut doc = DIDDocument::new("did:example:123");
    doc.add_verification_method(VerificationMethod {
        id: "vm1".into(),
        type_: "Ed25519VerificationKey2018".into(),
        public_key: vec![1, 2, 3],
    });
    reg.register(doc);
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
