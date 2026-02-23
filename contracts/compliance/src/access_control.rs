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
