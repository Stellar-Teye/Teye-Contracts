use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Role {
    Admin,
    Clinician,
    Researcher,
    Auditor,
    Patient,
}

#[derive(Debug, Clone)]
pub struct PermissionSet {
    pub can_read: bool,
    pub can_write: bool,
    pub can_audit: bool,
}

#[derive(Default)]
pub struct AccessControl {
    pub role_permissions: HashMap<Role, PermissionSet>,
}

impl AccessControl {
    pub fn new() -> Self {
        let mut ac = AccessControl::default();
        ac.role_permissions.insert(
            Role::Admin,
            PermissionSet {
                can_read: true,
                can_write: true,
                can_audit: true,
            },
        );
        ac.role_permissions.insert(
            Role::Clinician,
            PermissionSet {
                can_read: true,
                can_write: true,
                can_audit: false,
            },
        );
        ac.role_permissions.insert(
            Role::Researcher,
            PermissionSet {
                can_read: true,
                can_write: false,
                can_audit: false,
            },
        );
        ac.role_permissions.insert(
            Role::Auditor,
            PermissionSet {
                can_read: true,
                can_write: false,
                can_audit: true,
            },
        );
        ac.role_permissions.insert(
            Role::Patient,
            PermissionSet {
                can_read: true,
                can_write: false,
                can_audit: false,
            },
        );
        ac
    }

    pub fn check(&self, role: &Role, permission: &str) -> bool {
        if let Some(p) = self.role_permissions.get(role) {
            match permission {
                "read" => p.can_read,
                "write" => p.can_write,
                "audit" => p.can_audit,
                _ => false,
            }
        } else {
            false
        }
    }
}

// ── Policy-Aware Compliance Layer ───────────────────────────────────────────

/// Maps a compliance [`Role`] to the string representation used by the
/// common crate's policy engine attribute conditions.
pub fn role_to_policy_attr(role: &Role) -> &'static str {
    match role {
        Role::Admin => "admin",
        Role::Clinician => "clinician",
        Role::Researcher => "researcher",
        Role::Auditor => "auditor",
        Role::Patient => "patient",
    }
}

/// Wraps the base [`AccessControl`] with an additional layer that can
/// consult the composable policy engine's verdict when performing
/// compliance-level access checks.
///
/// The `policy_verdict` field is meant to be populated from the on-chain
/// policy engine result (e.g. via an off-chain compliance service that
/// invokes the contract's `evaluate_policy_engine` entry point) before
/// calling [`check_with_policy`].
pub struct PolicyAwareAccessControl {
    pub base: AccessControl,
    pub policy_verdict: Option<bool>,
}

impl PolicyAwareAccessControl {
    pub fn new() -> Self {
        Self {
            base: AccessControl::new(),
            policy_verdict: None,
        }
    }

    pub fn with_verdict(mut self, verdict: bool) -> Self {
        self.policy_verdict = Some(verdict);
        self
    }

    /// Performs a two-layer check:
    /// 1. The base role-permission check.
    /// 2. If a policy verdict has been supplied, both must agree (AND logic).
    ///    If no verdict is supplied, falls back to the base check alone.
    pub fn check_with_policy(&self, role: &Role, permission: &str) -> bool {
        let base_ok = self.base.check(role, permission);
        match self.policy_verdict {
            Some(verdict) => base_ok && verdict,
            None => base_ok,
        }
    }
}

impl Default for PolicyAwareAccessControl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_access_control_works() {
        let ac = AccessControl::new();
        assert!(ac.check(&Role::Admin, "read"));
        assert!(ac.check(&Role::Admin, "write"));
        assert!(ac.check(&Role::Admin, "audit"));
        assert!(ac.check(&Role::Clinician, "read"));
        assert!(ac.check(&Role::Clinician, "write"));
        assert!(!ac.check(&Role::Clinician, "audit"));
        assert!(!ac.check(&Role::Patient, "write"));
        assert!(!ac.check(&Role::Admin, "unknown"));
    }

    #[test]
    fn policy_aware_permits_when_both_agree() {
        let pac = PolicyAwareAccessControl::new().with_verdict(true);
        assert!(pac.check_with_policy(&Role::Admin, "read"));
    }

    #[test]
    fn policy_aware_denies_when_verdict_false() {
        let pac = PolicyAwareAccessControl::new().with_verdict(false);
        assert!(!pac.check_with_policy(&Role::Admin, "read"));
    }

    #[test]
    fn policy_aware_denies_when_base_denies() {
        let pac = PolicyAwareAccessControl::new().with_verdict(true);
        assert!(!pac.check_with_policy(&Role::Patient, "write"));
    }

    #[test]
    fn policy_aware_falls_back_without_verdict() {
        let pac = PolicyAwareAccessControl::new();
        assert!(pac.check_with_policy(&Role::Clinician, "read"));
        assert!(!pac.check_with_policy(&Role::Patient, "audit"));
    }

    #[test]
    fn role_to_attr_mapping() {
        assert_eq!(role_to_policy_attr(&Role::Admin), "admin");
        assert_eq!(role_to_policy_attr(&Role::Clinician), "clinician");
        assert_eq!(role_to_policy_attr(&Role::Researcher), "researcher");
        assert_eq!(role_to_policy_attr(&Role::Auditor), "auditor");
        assert_eq!(role_to_policy_attr(&Role::Patient), "patient");
    }
}
