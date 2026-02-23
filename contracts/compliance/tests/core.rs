use compliance::access_control::{AccessControl, Role};
use compliance::audit::AuditLog;
use compliance::retention::RetentionManager;

#[test]
fn test_access_control_defaults() {
    let ac = AccessControl::new();
    assert!(ac.check(&Role::Admin, "write"));
    assert!(!ac.check(&Role::Researcher, "write"));
}

#[test]
fn test_audit_record() {
    let mut log = AuditLog::default();
    log.record("user1", "read", "record:1");
    assert_eq!(log.query().len(), 1);
}

#[test]
fn test_retention() {
    let mut rm = RetentionManager::new();
    rm.add_policy("phi", 1);
    // new records shouldn't be purged immediately
    let now = rm.created_at;
    assert!(!rm.should_purge(now, "phi"));
}
