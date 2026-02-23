use soroban_sdk::{contracttype, symbol_short, Address, Env, Vec};

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

fn extend_ttl_address_key(env: &Env, key: &(soroban_sdk::Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_ttl_delegation_key(env: &Env, key: &(soroban_sdk::Symbol, Address, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Permission {
    ReadAnyRecord = 1,
    WriteRecord = 2,
    ManageAccess = 3,
    ManageUsers = 4,
    SystemAdmin = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)] // Needed for easier comparison/conversion
pub enum Role {
    Patient = 1,
    Staff = 2,
    Optometrist = 3,
    Ophthalmologist = 4,
    Admin = 5,
}

pub fn get_base_permissions(env: &Env, role: &Role) -> Vec<Permission> {
    let mut perms = Vec::new(env);

    if *role == Role::Admin {
        perms.push_back(Permission::SystemAdmin);
    }

    if *role == Role::Admin
        || *role == Role::Ophthalmologist
        || *role == Role::Optometrist
        || *role == Role::Staff
    {
        perms.push_back(Permission::ManageUsers);
    }

    if *role == Role::Admin || *role == Role::Ophthalmologist || *role == Role::Optometrist {
        perms.push_back(Permission::WriteRecord);
        perms.push_back(Permission::ManageAccess);
        perms.push_back(Permission::ReadAnyRecord);
    }

    // Patients have essentially no specific global permissions, they manage their own implicitly

    perms
}

/// Represents an assigned role with specific custom grants or revocations
#[contracttype]
#[derive(Clone, Debug)]
pub struct RoleAssignment {
    pub role: Role,
    pub custom_grants: Vec<Permission>,
    pub custom_revokes: Vec<Permission>,
    pub expires_at: u64, // 0 means never expires
}

/// Represents the delegation of a role to someone else
#[contracttype]
#[derive(Clone, Debug)]
pub struct Delegation {
    pub delegator: Address,
    pub delegatee: Address,
    pub role: Role,
    pub expires_at: u64, // 0 means never expires
}

/// Internal store schema helpers
pub fn user_assignment_key(user: &Address) -> (soroban_sdk::Symbol, Address) {
    (symbol_short!("ROLE_ASN"), user.clone())
}

pub fn delegation_key(
    delegator: &Address,
    delegatee: &Address,
) -> (soroban_sdk::Symbol, Address, Address) {
    (
        symbol_short!("DELEGATE"),
        delegator.clone(),
        delegatee.clone(),
    )
}

// ======================== Core RBAC Engine ========================

pub fn assign_role(env: &Env, user: Address, role: Role, expires_at: u64) {
    let assignment = RoleAssignment {
        role,
        custom_grants: Vec::new(env),
        custom_revokes: Vec::new(env),
        expires_at,
    };

    let key = user_assignment_key(&user);
    env.storage().persistent().set(&key, &assignment);
    extend_ttl_address_key(env, &key);
}

/// Retrieve the active assignment for a user, or None if it doesn't exist or is expired
pub fn get_active_assignment(env: &Env, user: &Address) -> Option<RoleAssignment> {
    if let Some(assignment) = env
        .storage()
        .persistent()
        .get::<_, RoleAssignment>(&user_assignment_key(user))
    {
        if assignment.expires_at == 0 || assignment.expires_at > env.ledger().timestamp() {
            return Some(assignment);
        }
    }
    None
}

/// Set custom permissions for an existing assignment
pub fn grant_custom_permission(env: &Env, user: Address, permission: Permission) -> Result<(), ()> {
    let mut assignment = get_active_assignment(env, &user).ok_or(())?;

    // Remove from revokes if present
    let mut new_revokes = Vec::new(env);
    for r in assignment.custom_revokes.iter() {
        if r != permission {
            new_revokes.push_back(r);
        }
    }
    assignment.custom_revokes = new_revokes;

    // Add to grants if not already there
    if !assignment.custom_grants.contains(&permission) {
        assignment.custom_grants.push_back(permission);
    }

    let key = user_assignment_key(&user);
    env.storage().persistent().set(&key, &assignment);
    extend_ttl_address_key(env, &key);
    Ok(())
}

/// Revoke a permission for a specific user specifically
pub fn revoke_custom_permission(
    env: &Env,
    user: Address,
    permission: Permission,
) -> Result<(), ()> {
    let mut assignment = get_active_assignment(env, &user).ok_or(())?;

    // Remove from grants if present
    let mut new_grants = Vec::new(env);
    for g in assignment.custom_grants.iter() {
        if g != permission {
            new_grants.push_back(g);
        }
    }
    assignment.custom_grants = new_grants;

    // Add to revokes if not already there
    if !assignment.custom_revokes.contains(&permission) {
        assignment.custom_revokes.push_back(permission);
    }

    let key = user_assignment_key(&user);
    env.storage().persistent().set(&key, &assignment);
    extend_ttl_address_key(env, &key);
    Ok(())
}

/// Create a delegation from `delegator` to `delegatee`
pub fn delegate_role(
    env: &Env,
    delegator: Address,
    delegatee: Address,
    role: Role,
    expires_at: u64,
) {
    let del = Delegation {
        delegator: delegator.clone(),
        delegatee: delegatee.clone(),
        role,
        expires_at,
    };

    let key = delegation_key(&delegator, &delegatee);
    env.storage().persistent().set(&key, &del);
    extend_ttl_delegation_key(env, &key);
}

/// Retrieve the active delegations for a particular `delegatee` representing `delegator`
pub fn get_active_delegation(
    env: &Env,
    delegator: &Address,
    delegatee: &Address,
) -> Option<Delegation> {
    if let Some(del) = env
        .storage()
        .persistent()
        .get::<_, Delegation>(&delegation_key(delegator, delegatee))
    {
        if del.expires_at == 0 || del.expires_at > env.ledger().timestamp() {
            return Some(del);
        }
    }
    None
}

/// Evaluates if a specified `user` holds a `permission`.
/// This function merges Base Role inherited permissions, Custom Grants, Custom Revokes,
/// and currently active delegated Roles.
pub fn has_permission(env: &Env, user: &Address, permission: &Permission) -> bool {
    // 1. Check primary active assignment
    if let Some(assignment) = get_active_assignment(env, user) {
        // Did we explicitly revoke it?
        if assignment.custom_revokes.contains(permission) {
            return false; // Explicit revoke overrides all basic/grant logic below
        }

        // Did we explicitly grant it?
        if assignment.custom_grants.contains(permission) {
            return true;
        }

        // Do we get it implicitly through our baseline role hierarchy?
        if get_base_permissions(env, &assignment.role).contains(permission) {
            return true;
        }
    }

    false
}

/// Same as has_permission, but also checks if `delegatee` can perform the action on behalf of `delegator`.
pub fn has_delegated_permission(
    env: &Env,
    delegator: &Address,
    delegatee: &Address,
    permission: &Permission,
) -> bool {
    if let Some(delegation) = get_active_delegation(env, delegator, delegatee) {
        if get_base_permissions(env, &delegation.role).contains(permission) {
            return true;
        }
    }
    false
}
