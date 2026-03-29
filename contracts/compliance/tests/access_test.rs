//! Unauthenticated admin function call tests for the `compliance` crate.
//!
//! Ensures that functions restricted to Admin role return denied/false when
//! invoked by non-admin or unauthenticated roles, mirroring the Soroban
//! `Unauthorized` pattern for on-chain access control.

use compliance::access_control::{AccessControl, PolicyAwareAccessControl, Role};

// ── Non-admin roles cannot write ──────────────────────────────────────────────

#[test]
fn test_researcher_cannot_write() {
    let ac = AccessControl::new();
    assert!(
        !ac.check(&Role::Researcher, "write"),
        "Researcher must not have write permission"
    );
}

#[test]
fn test_auditor_cannot_write() {
    let ac = AccessControl::new();
    assert!(
        !ac.check(&Role::Auditor, "write"),
        "Auditor must not have write permission"
    );
}

#[test]
fn test_patient_cannot_write() {
    let ac = AccessControl::new();
    assert!(
        !ac.check(&Role::Patient, "write"),
        "Patient must not have write permission"
    );
}

// ── Non-privileged roles cannot audit ─────────────────────────────────────────

#[test]
fn test_clinician_cannot_audit() {
    let ac = AccessControl::new();
    assert!(
        !ac.check(&Role::Clinician, "audit"),
        "Clinician must not have audit permission"
    );
}

#[test]
fn test_researcher_cannot_audit() {
    let ac = AccessControl::new();
    assert!(
        !ac.check(&Role::Researcher, "audit"),
        "Researcher must not have audit permission"
    );
}

#[test]
fn test_patient_cannot_audit() {
    let ac = AccessControl::new();
    assert!(
        !ac.check(&Role::Patient, "audit"),
        "Patient must not have audit permission"
    );
}

// ── Unknown / admin-only operations always denied to non-admins ───────────────

#[test]
fn test_unknown_permission_denied_for_all_roles() {
    let ac = AccessControl::new();
    let all_roles = [
        Role::Admin,
        Role::Clinician,
        Role::Researcher,
        Role::Auditor,
        Role::Patient,
    ];
    for role in &all_roles {
        assert!(
            !ac.check(role, "delete"),
            "{role:?} must not have 'delete' permission"
        );
        assert!(
            !ac.check(role, "sudo"),
            "{role:?} must not have 'sudo' permission"
        );
        assert!(
            !ac.check(role, ""),
            "{role:?} must not have empty-string permission"
        );
    }
}

// ── Policy verdict of false blocks even Admin ─────────────────────────────────

#[test]
fn test_admin_blocked_when_policy_verdict_false() {
    let pac = PolicyAwareAccessControl::new().with_verdict(false);
    assert!(
        !pac.check_with_policy(&Role::Admin, "read"),
        "Admin must be blocked when policy verdict is false"
    );
    assert!(
        !pac.check_with_policy(&Role::Admin, "write"),
        "Admin must be blocked when policy verdict is false"
    );
    assert!(
        !pac.check_with_policy(&Role::Admin, "audit"),
        "Admin must be blocked when policy verdict is false"
    );
}

#[test]
fn test_non_admin_blocked_regardless_of_policy_verdict_true() {
    // Even with a permissive policy, roles that don't have a permission stay denied.
    let pac = PolicyAwareAccessControl::new().with_verdict(true);
    assert!(
        !pac.check_with_policy(&Role::Researcher, "write"),
        "Researcher cannot write even with a true policy verdict"
    );
    assert!(
        !pac.check_with_policy(&Role::Patient, "audit"),
        "Patient cannot audit even with a true policy verdict"
    );
}

// ── Minimal-privilege role (Patient) ─────────────────────────────────────────

#[test]
fn test_patient_read_only() {
    let ac = AccessControl::new();
    assert!(ac.check(&Role::Patient, "read"), "Patient must be able to read");
    assert!(!ac.check(&Role::Patient, "write"), "Patient must not write");
    assert!(!ac.check(&Role::Patient, "audit"), "Patient must not audit");
}

// ── Admin has all permissions ──────────────────────────────────────────────────

#[test]
fn test_admin_has_all_base_permissions() {
    let ac = AccessControl::new();
    assert!(ac.check(&Role::Admin, "read"));
    assert!(ac.check(&Role::Admin, "write"));
    assert!(ac.check(&Role::Admin, "audit"));
}
