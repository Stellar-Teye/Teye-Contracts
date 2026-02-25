use compliance::access_control::{AccessControl, Role};
use compliance::audit::{ComplianceAuditLog, SearchKey};
use compliance::retention::RetentionManager;

#[test]
fn test_access_control_defaults() {
    let ac = AccessControl::new();
    assert!(ac.check(&Role::Admin, "write"));
    assert!(!ac.check(&Role::Researcher, "write"));
}

#[test]
fn test_audit_record() {
    let key = SearchKey::from_bytes(&[0x42u8; 32]).unwrap();
    let mut log = ComplianceAuditLog::new(key);
    // Use timestamp 1000 for deterministic testing
    log.record(1000, "user1", "read", "record:1", "ok");
    assert_eq!(log.len(), 1);
    
    // Test searchable functionality while we are at it
    let hits = log.search("user1");
    assert_eq!(hits, vec![1]);

    let mut log = AuditLog::default();
    log.record("user1", "read", "record:1", 1000);
    assert_eq!(log.query().len(), 1);
}

#[test]
fn test_retention() {
    let mut rm = RetentionManager::new(1000);
    rm.add_policy("phi", 1);
    // new records shouldn't be purged immediately
    let now = rm.created_at;
    assert!(!rm.should_purge(now, "phi", 1000));
}
