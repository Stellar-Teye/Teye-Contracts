use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol, Vec};

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

/// Time-based access restrictions
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum TimeRestriction {
    /// No time restriction
    None,
    /// Only allow access during business hours (9 AM - 5 PM UTC)
    BusinessHours,
    /// Only allow access during specific hour range (start_hour, end_hour, inclusive)
    HourRange(u32, u32),
    /// Only allow access on specific days of week (bitmask: 0b0000001 = Sunday, 0b1000000 = Saturday)
    DaysOfWeek(u32),
}

/// Credential types for attribute-based access
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum CredentialType {
    None,
    MedicalLicense,
    ResearchCredentials,
    EmergencyCredentials,
    AdminCredentials,
}

/// Record sensitivity levels
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum SensitivityLevel {
    Public,
    Standard,
    Confidential,
    Restricted,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionalRole {
    None,
    Some(Role),
}

/// Attribute-based access policy conditions
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyConditions {
    pub required_role: OptionalRole,
    pub time_restriction: TimeRestriction,
    pub required_credential: CredentialType,
    pub min_sensitivity_level: SensitivityLevel,
    pub consent_required: bool,
}

/// Access policy combining RBAC with attribute-based conditions
#[contracttype]
#[derive(Clone, Debug)]
pub struct AccessPolicy {
    pub id: String,
    pub name: String,
    pub conditions: PolicyConditions,
    pub enabled: bool,
}

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
#[repr(u32)]
pub enum Role {
    None = 0,
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

/// Represents a scoped delegation: only specific permissions (not a full role) are delegated.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ScopedDelegation {
    pub delegator: Address,
    pub delegatee: Address,
    pub permissions: Vec<Permission>,
    pub expires_at: u64, // 0 means never expires
}

/// Internal store schema helpers
pub fn user_assignment_key(user: &Address) -> (soroban_sdk::Symbol, Address) {
    (symbol_short!("ROLE_ASN"), user.clone())
}

pub fn delegation_key(delegator: &Address, delegatee: &Address) -> (Symbol, Address, Address) {
    (
        symbol_short!("DELEGATE"),
        delegator.clone(),
        delegatee.clone(),
    )
}

pub fn scoped_delegation_key(
    delegator: &Address,
    delegatee: &Address,
) -> (Symbol, Address, Address) {
    (
        symbol_short!("DLG_SCOPE"),
        delegator.clone(),
        delegatee.clone(),
    )
}

pub fn delegatee_index_key(delegatee: &Address) -> (Symbol, Address) {
    (symbol_short!("DEL_IDX"), delegatee.clone())
}

pub fn delegator_index_key(delegator: &Address) -> (Symbol, Address) {
    (symbol_short!("DLGTR_IDX"), delegator.clone())
}

pub fn acl_group_key(name: &String) -> (Symbol, String) {
    (symbol_short!("ACL_GRP"), name.clone())
}

pub fn user_groups_key(user: &Address) -> (Symbol, Address) {
    (symbol_short!("USR_GRPS"), user.clone())
}

// pub fn delegatee_index_key(delegatee: &Address) -> (Symbol, Address) {
//     (symbol_short!("DEL_IDX"), delegatee.clone())
// }

pub fn access_policy_key(id: &String) -> (Symbol, String) {
    (symbol_short!("ACC_POL"), id.clone())
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevokedDelegation {
    pub delegatee: Address,
    pub is_scoped: bool,
}

pub fn user_credential_key(user: &Address) -> (Symbol, Address) {
    (symbol_short!("USER_CRED"), user.clone())
}

pub fn record_sensitivity_key(record_id: &u64) -> (Symbol, u64) {
    (symbol_short!("REC_SENS"), *record_id)
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
        delegators.push_back(delegator.clone());
    }
    env.storage().persistent().set(&idx_key, &delegators);
    extend_ttl_address_key(env, &idx_key);

    // Maintain the delegator's index of delegatees for cascade cleanup.
    let delegator_idx_key = delegator_index_key(&delegator);
    let mut delegatees: Vec<Address> = env
        .storage()
        .persistent()
        .get(&delegator_idx_key)
        .unwrap_or(Vec::new(env));
    if !delegatees.contains(&delegatee) {
        delegatees.push_back(delegatee);
    }
    env.storage()
        .persistent()
        .set(&delegator_idx_key, &delegatees);
    extend_ttl_address_key(env, &delegator_idx_key);
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

/// Create a scoped delegation: grant only specific permissions (not a full role) to the delegatee.
/// The delegatee will have only these permissions in the context of this delegator→delegatee link.
/// Respects `expires_at` (0 = never expires).
pub fn delegate_permissions(
    env: &Env,
    delegator: Address,
    delegatee: Address,
    permissions: Vec<Permission>,
    expires_at: u64,
) {
    if permissions.is_empty() {
        return;
    }

    let del = ScopedDelegation {
        delegator: delegator.clone(),
        delegatee: delegatee.clone(),
        permissions: permissions.clone(),
        expires_at,
    };

    let key = scoped_delegation_key(&delegator, &delegatee);
    env.storage().persistent().set(&key, &del);
    extend_ttl_delegation_key(env, &key);

    let idx_key = delegatee_index_key(&delegatee);
    let mut delegators: Vec<Address> = env
        .storage()
        .persistent()
        .get(&idx_key)
        .unwrap_or(Vec::new(env));

    if !delegators.contains(&delegator) {
        delegators.push_back(delegator.clone());
    }
    env.storage().persistent().set(&idx_key, &delegators);
    extend_ttl_address_key(env, &idx_key);

    // Maintain the delegator's index of delegatees for cascade cleanup.
    let delegator_idx_key = delegator_index_key(&delegator);
    let mut delegatees: Vec<Address> = env
        .storage()
        .persistent()
        .get(&delegator_idx_key)
        .unwrap_or(Vec::new(env));
    if !delegatees.contains(&delegatee) {
        delegatees.push_back(delegatee);
    }
    env.storage()
        .persistent()
        .set(&delegator_idx_key, &delegatees);
    extend_ttl_address_key(env, &delegator_idx_key);
}

/// Retrieve the active scoped delegation for a particular delegator→delegatee pair.
pub fn get_active_scoped_delegation(
    env: &Env,
    delegator: &Address,
    delegatee: &Address,
) -> Option<ScopedDelegation> {
    if let Some(del) = env
        .storage()
        .persistent()
        .get::<_, ScopedDelegation>(&scoped_delegation_key(delegator, delegatee))
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
    if !env.storage().persistent().has(&acl_group_key(&group_name)) {
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
/// Returns true if either:
/// - There is an active full role delegation and the role's base permissions include `permission`, or
/// - There is an active scoped delegation whose permission list includes `permission`.
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
    // Full role delegation: delegatee gets all permissions of the role
    if let Some(delegation) = get_active_delegation(env, delegator, delegatee) {
        if get_base_permissions(env, &delegation.role).contains(permission) {
            return true;
        }
    }
    // Scoped delegation: delegatee gets only the listed permissions
    if let Some(scoped) = get_active_scoped_delegation(env, delegator, delegatee) {
        if scoped.permissions.contains(permission) {
            return true;
        }
    }
    false
}

/// Revoke all delegations (full role and scoped) originating from `delegator`.
///
/// Returns details for each removed delegation so callers can emit per-revocation events.
pub fn revoke_delegations_from(env: &Env, delegator: &Address) -> Vec<RevokedDelegation> {
    let delegator_idx_key = delegator_index_key(delegator);
    let delegatees: Vec<Address> = env
        .storage()
        .persistent()
        .get(&delegator_idx_key)
        .unwrap_or(Vec::new(env));

    let mut revoked = Vec::new(env);

    for delegatee in delegatees.iter() {
        let role_key = delegation_key(delegator, &delegatee);
        if env.storage().persistent().has(&role_key) {
            env.storage().persistent().remove(&role_key);
            revoked.push_back(RevokedDelegation {
                delegatee: delegatee.clone(),
                is_scoped: false,
            });
        }

        let scoped_key = scoped_delegation_key(delegator, &delegatee);
        if env.storage().persistent().has(&scoped_key) {
            env.storage().persistent().remove(&scoped_key);
            revoked.push_back(RevokedDelegation {
                delegatee: delegatee.clone(),
                is_scoped: true,
            });
        }

        let delegatee_idx_key = delegatee_index_key(&delegatee);
        let delegators: Vec<Address> = env
            .storage()
            .persistent()
            .get(&delegatee_idx_key)
            .unwrap_or(Vec::new(env));
        let mut remaining = Vec::new(env);
        for indexed_delegator in delegators.iter() {
            if indexed_delegator != delegator.clone() {
                remaining.push_back(indexed_delegator);
            }
        }

        if remaining.is_empty() {
            env.storage().persistent().remove(&delegatee_idx_key);
        } else {
            env.storage()
                .persistent()
                .set(&delegatee_idx_key, &remaining);
            extend_ttl_address_key(env, &delegatee_idx_key);
        }
    }

    env.storage().persistent().remove(&delegator_idx_key);
    revoked
}

// ======================== ABAC Policy Engine ========================

/// Check if current time satisfies time restriction
fn satisfies_time_restriction(env: &Env, restriction: &TimeRestriction) -> bool {
    match restriction {
        TimeRestriction::None => true,
        TimeRestriction::BusinessHours => {
            let timestamp = env.ledger().timestamp();
            let hour = (timestamp / 3600) % 24;
            (9..=17).contains(&hour)
        }
        TimeRestriction::HourRange(start, end) => {
            let timestamp = env.ledger().timestamp();
            let hour = (timestamp / 3600) % 24;
            if start <= end {
                hour >= *start as u64 && hour <= *end as u64
            } else {
                // Handle overnight range (e.g., 22-6 means 10 PM to 6 AM)
                hour >= *start as u64 || hour <= *end as u64
            }
        }
        TimeRestriction::DaysOfWeek(day_mask) => {
            let timestamp = env.ledger().timestamp();
            let day_of_week = ((timestamp / 86400) + 4) % 7; // Unix epoch was Thursday
            (day_mask & (1 << day_of_week)) != 0
        }
    }
}

/// Get user's credential type from storage
fn get_user_credential(env: &Env, user: &Address) -> CredentialType {
    let key = user_credential_key(user);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(CredentialType::None)
}

/// Get record sensitivity level from storage
fn get_record_sensitivity(env: &Env, record_id: &u64) -> SensitivityLevel {
    let key = record_sensitivity_key(record_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(SensitivityLevel::Standard)
}

/// Context for policy evaluation
#[contracttype]
#[derive(Clone, Debug)]
pub struct PolicyContext {
    pub user: Address,
    pub resource_id: Option<u64>, // Record ID if applicable
    pub patient: Option<Address>, // Patient address if applicable
    pub current_time: u64,
}

/// Evaluate an access policy against the given context
pub fn evaluate_policy(env: &Env, policy: &AccessPolicy, context: &PolicyContext) -> bool {
    if !policy.enabled {
        return false;
    }

    let conditions = &policy.conditions;

    // Check role requirement
    if let OptionalRole::Some(required_role) = &conditions.required_role {
        if let Some(assignment) = get_active_assignment(env, &context.user) {
            if assignment.role != conditions.required_role {
                return false;
            }
        } else {
            return false;
        }
    }

    // Check time restriction
    if !satisfies_time_restriction(env, &conditions.time_restriction) {
        return false;
    }

    // Check credential requirement
    if conditions.required_credential != CredentialType::None {
        let user_credential = get_user_credential(env, &context.user);
        if user_credential != conditions.required_credential {
            return false;
        }
    }

    // Check sensitivity level requirement
    if let Some(record_id) = &context.resource_id {
        let record_sensitivity = get_record_sensitivity(env, record_id);
        // User can access records at or above their minimum sensitivity level
        if (record_sensitivity as u32) < (conditions.min_sensitivity_level as u32) {
            return false;
        }
    }

    // Check consent requirement
    if conditions.consent_required {
        if let (Some(patient), Some(_record_id)) = (&context.patient, &context.resource_id) {
            // Check if there's active consent for this user to access this patient's records
            let consent_key = (
                symbol_short!("CONSENT"),
                patient.clone(),
                context.user.clone(),
            );
            if let Some(consent) = env
                .storage()
                .persistent()
                .get::<_, ConsentGrant>(&consent_key)
            {
                if consent.revoked || consent.expires_at <= context.current_time {
                    return false;
                }
            } else {
                return false;
            }
        } else {
            return false; // Consent required but no patient context provided
        }
    }

    true
}

/// Evaluate all applicable policies for a user and resource
pub fn evaluate_access_policies(
    env: &Env,
    user: &Address,
    resource_id: Option<u64>,
    patient: Option<Address>,
) -> bool {
    // Get all policies (in a real implementation, you might want to index policies by user/resource)
    // For now, we'll check a few default policy IDs
    let mut default_policy_ids = Vec::new(env);
    default_policy_ids.push_back(String::from_str(env, "default_medical_access"));
    default_policy_ids.push_back(String::from_str(env, "emergency_access"));
    default_policy_ids.push_back(String::from_str(env, "research_access"));

    let context = PolicyContext {
        user: user.clone(),
        resource_id,
        patient,
        current_time: env.ledger().timestamp(),
    };
    let mut found_policy = false;

    for i in 0..default_policy_ids.len() {
        if let Some(policy_id) = default_policy_ids.get(i) {
            let key = access_policy_key(&policy_id);
            if let Some(policy) = env.storage().persistent().get::<_, AccessPolicy>(&key) {
                found_policy = true;
                if evaluate_policy(env, &policy, &context) {
                    return true;
                }
            }
        }
    }

    // Backward-compatible default: if no ABAC policies are configured,
    // don't block otherwise valid consent/access grants.
    if !found_policy {
        return true;
    }

    false
}

/// Set user credential type
pub fn set_user_credential(env: &Env, user: Address, credential: CredentialType) {
    let key = user_credential_key(&user);
    env.storage().persistent().set(&key, &credential);
    extend_ttl_address_key(env, &key);
}

/// Set record sensitivity level
pub fn set_record_sensitivity(env: &Env, record_id: u64, sensitivity: SensitivityLevel) {
    let key = record_sensitivity_key(&record_id);
    env.storage().persistent().set(&key, &sensitivity);
    extend_ttl_u64_key(env, &key);
}

/// Create or update an access policy
pub fn create_access_policy(env: &Env, policy: AccessPolicy) {
    let key = access_policy_key(&policy.id);
    env.storage().persistent().set(&key, &policy);
}

fn extend_ttl_u64_key(env: &Env, key: &(soroban_sdk::Symbol, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Consent grant structure for ABAC evaluation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentGrant {
    pub patient: Address,
    pub grantee: Address,
    pub consent_type: crate::ConsentType,
    pub granted_at: u64,
    pub expires_at: u64,
    pub revoked: bool,
}

// ======================== Policy Engine Integration ========================

/// Builds an [`teye_common::policy_dsl::EvalContext`] from the existing RBAC
/// context, populating subject attributes (role, credential) automatically.
pub fn build_eval_context(
    env: &Env,
    user: &Address,
    action: &str,
    resource_id: Option<u64>,
) -> teye_common::policy_dsl::EvalContext {
    let mut attr_keys = Vec::new(env);
    let mut attr_vals = Vec::new(env);

    // Populate role attribute from the active assignment
    if let Some(assignment) = get_active_assignment(env, user) {
        attr_keys.push_back(String::from_str(env, "role"));
        let role_str = match assignment.role {
            Role::None => "none",
            Role::Patient => "patient",
            Role::Staff => "staff",
            Role::Optometrist => "optometrist",
            Role::Ophthalmologist => "ophthalmologist",
            Role::Admin => "admin",
        };
        attr_vals.push_back(String::from_str(env, role_str));
    }

    // Populate credential attribute
    let cred = get_user_credential(env, user);
    if cred != CredentialType::None {
        attr_keys.push_back(String::from_str(env, "credential"));
        let cred_str = match cred {
            CredentialType::None => "none",
            CredentialType::MedicalLicense => "medical_license",
            CredentialType::ResearchCredentials => "research",
            CredentialType::EmergencyCredentials => "emergency",
            CredentialType::AdminCredentials => "admin",
        };
        attr_vals.push_back(String::from_str(env, cred_str));
    }

    // Populate sensitivity attribute if a resource is specified
    if let Some(rid) = resource_id {
        let sensitivity = get_record_sensitivity(env, &rid);
        attr_keys.push_back(String::from_str(env, "sensitivity"));
        let sens_str = match sensitivity {
            SensitivityLevel::Public => "public",
            SensitivityLevel::Standard => "standard",
            SensitivityLevel::Confidential => "confidential",
            SensitivityLevel::Restricted => "restricted",
        };
        attr_vals.push_back(String::from_str(env, sens_str));
    }

    let res_id_str = match resource_id {
        Some(id) => {
            let mut buf = String::from_str(env, "record_");
            // Simple numeric-to-string for on-chain use
            let id_str = id_to_string(env, id);
            buf = concat_strings(env, &buf, &id_str);
            buf
        }
        None => String::from_str(env, "global"),
    };

    teye_common::policy_dsl::EvalContext {
        subject: user.clone(),
        resource_id: res_id_str,
        action: String::from_str(env, action),
        timestamp: env.ledger().timestamp(),
        attr_keys,
        attr_vals,
    }
}

/// Evaluates the composable policy engine for a given user and action.
///
/// This function complements the existing `has_permission` / ABAC checks by
/// delegating to the common crate's policy engine. Returns `true` if the
/// policy engine permits the action, `false` otherwise.
pub fn check_policy_engine(
    env: &Env,
    user: &Address,
    action: &str,
    resource_id: Option<u64>,
) -> bool {
    let ctx = build_eval_context(env, user, action, resource_id);
    let result = teye_common::policy_engine::evaluate(env, &ctx);
    result.effect == teye_common::policy_dsl::PolicyEffect::Permit
}

/// Runs a policy simulation without side-effects, useful for what-if analysis.
pub fn simulate_policy_check(
    env: &Env,
    user: &Address,
    action: &str,
    resource_id: Option<u64>,
) -> teye_common::policy_dsl::SimulationResult {
    let ctx = build_eval_context(env, user, action, resource_id);
    teye_common::policy_engine::simulate(env, &ctx)
}

// ── Numeric helpers for on-chain string building ────────────────────────────

fn id_to_string(env: &Env, mut id: u64) -> String {
    if id == 0 {
        return String::from_str(env, "0");
    }
    let mut digits: soroban_sdk::Vec<u32> = Vec::new(env);
    while id > 0 {
        digits.push_back((id % 10) as u32);
        id /= 10;
    }
    let mut result = String::from_str(env, "");
    let len = digits.len();
    for i in 0..len {
        let d = digits.get(len - 1 - i).unwrap();
        let ch = match d {
            0 => "0",
            1 => "1",
            2 => "2",
            3 => "3",
            4 => "4",
            5 => "5",
            6 => "6",
            7 => "7",
            8 => "8",
            9 => "9",
            _ => "0",
        };
        result = concat_strings(env, &result, &String::from_str(env, ch));
    }
    result
}

fn concat_strings(env: &Env, a: &String, b: &String) -> String {
    let a_bytes = a.to_bytes();
    let b_bytes = b.to_bytes();
    let mut combined = soroban_sdk::Bytes::new(env);
    for i in 0..a_bytes.len() {
        combined.push_back(a_bytes.get(i).unwrap());
    }
    for i in 0..b_bytes.len() {
        combined.push_back(b_bytes.get(i).unwrap());
    }
    let mut buf = [0u8; 256];
    let len = combined.len() as usize;
    for (i, slot) in buf.iter_mut().enumerate().take(len) {
        *slot = combined.get(i as u32).unwrap();
    }
    String::from_bytes(env, &buf[..len])
}
