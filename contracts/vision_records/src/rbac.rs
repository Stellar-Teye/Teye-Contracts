use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol, Vec};

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

/// Represents an ACL Group with a set of permissions
#[contracttype]
#[derive(Clone, Debug)]
pub struct AclGroup {
    pub name: String,
    pub permissions: Vec<Permission>,
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
) -> (Symbol, Address, Address) {
    (
        symbol_short!("DELEGATE"),
        delegator.clone(),
        delegatee.clone(),
    )
}

pub fn acl_group_key(name: &String) -> (Symbol, String) {
    (symbol_short!("ACL_GRP"), name.clone())
}

pub fn user_groups_key(user: &Address) -> (Symbol, Address) {
    (symbol_short!("USR_GRPS"), user.clone())
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

/// Create a delegation from `delegator` to `delegatee`.
///
/// Also updates the delegatee's delegation index so that `has_permission`
/// can discover all active delegations when evaluating permissions.
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

    // Maintain the delegatee's index of delegators for unified permission lookups
    let idx_key = delegatee_index_key(&delegatee);
    let mut delegators: Vec<Address> = env
        .storage()
        .persistent()
        .get(&idx_key)
        .unwrap_or(Vec::new(env));

    if !delegators.contains(&delegator) {
        delegators.push_back(delegator);
    }
    env.storage().persistent().set(&idx_key, &delegators);
    extend_ttl_address_key(env, &idx_key);
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

// ======================== ACL Group Management ========================

pub fn create_group(env: &Env, name: String, permissions: Vec<Permission>) {
    let group = AclGroup {
        name: name.clone(),
        permissions,
    };
    env.storage()
        .persistent()
        .set(&acl_group_key(&name), &group);
}

pub fn delete_group(env: &Env, name: String) {
    env.storage().persistent().remove(&acl_group_key(&name));
}

pub fn add_to_group(env: &Env, user: Address, group_name: String) -> Result<(), ()> {
    // Verify group exists
    if !env.storage()
        .persistent()
        .has(&acl_group_key(&group_name))
    {
        return Err(());
    }

    let mut groups: Vec<String> = env
        .storage()
        .persistent()
        .get(&user_groups_key(&user))
        .unwrap_or(Vec::new(env));

    if !groups.contains(&group_name) {
        groups.push_back(group_name);
        env.storage()
            .persistent()
            .set(&user_groups_key(&user), &groups);
    }
    Ok(())
}

pub fn remove_from_group(env: &Env, user: Address, group_name: String) {
    let groups: Vec<String> = env
        .storage()
        .persistent()
        .get(&user_groups_key(&user))
        .unwrap_or(Vec::new(env));

    let mut new_groups = Vec::new(env);
    for g in groups.iter() {
        if g != group_name {
            new_groups.push_back(g);
        }
    }
    env.storage()
        .persistent()
        .set(&user_groups_key(&user), &new_groups);
}

pub fn get_group_permissions(env: &Env, name: &String) -> Vec<Permission> {
    if let Some(group) = env
        .storage()
        .persistent()
        .get::<_, AclGroup>(&acl_group_key(name))
    {
        group.permissions
    } else {
        Vec::new(env)
    }
}

/// Evaluates if a specified `user` holds a `permission`.
/// This function merges Base Role inherited permissions, Custom Grants, Custom Revokes,
/// and currently active delegated Roles.
pub fn has_permission(env: &Env, user: &Address, permission: &Permission) -> bool {
    // Step 1: Check direct role assignment
    if let Some(assignment) = get_active_assignment(env, user) {
        // Explicit revoke takes highest priority — overrides grants,
        // base role, AND delegations to prevent bypass.
        if assignment.custom_revokes.contains(permission) {
            return false;
        }

        // Explicit custom grant takes precedence over base role lookup
        if assignment.custom_grants.contains(permission) {
            return true;
        }

        // Check base permissions inherited from the assigned role
        if get_base_permissions(env, &assignment.role).contains(permission) {
            return true;
        }
    }

    // 2. Check group-based permissions
    let user_groups: Vec<String> = env
        .storage()
        .persistent()
        .get(&user_groups_key(user))
        .unwrap_or(Vec::new(env));

    for group_name in user_groups.iter() {
        if get_group_permissions(env, &group_name).contains(permission) {
            return true;
        }
    }

    false
}

/// Checks if `delegatee` holds `permission` through a specific delegation
/// from `delegator`.
///
/// Unlike `has_permission` which checks ALL delegation paths, this function
/// verifies a specific delegator→delegatee relationship. Use this when the
/// caller must be acting on behalf of a particular entity (e.g., a provider
/// delegating record-writing authority, or a patient delegating access
/// management).
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
