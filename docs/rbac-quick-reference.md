# RBAC Quick Reference

## Role → Permission Mapping

| Role | Level | Permissions | Common Use Cases |
|------|-------|-------------|------------------|
| **Admin** | 5 | ✓ SystemAdmin<br/>✓ ManageUsers<br/>✓ ManageAccess<br/>✓ WriteRecord<br/>✓ ReadAnyRecord | • System administration<br/>• User management<br/>• Contract upgrades<br/>• Policy management |
| **Ophthalmologist** | 4 | ✓ ManageUsers<br/>✓ ManageAccess<br/>✓ WriteRecord<br/>✓ ReadAnyRecord<br/>✗ SystemAdmin | • Patient examination<br/>• Prescription management<br/>• Staff oversight<br/>• Complex cases |
| **Optometrist** | 3 | ✓ ManageUsers<br/>✓ ManageAccess<br/>✓ WriteRecord<br/>✓ ReadAnyRecord<br/>✗ SystemAdmin | • Eye exams<br/>• Prescription writing<br/>• Record management<br/>• Basic staff management |
| **Staff** | 2 | ✓ ManageUsers<br/>✗ ManageAccess<br/>✗ WriteRecord<br/>✗ ReadAnyRecord<br/>✗ SystemAdmin | • Schedule management<br/>• Patient check-in<br/>• Administrative support<br/>• Data input assistance |
| **Patient** | 1 | *None (implicit)* | • Own record access<br/>• Consent management<br/>• Report viewing |
| **None** | 0 | *None* | • Unassigned users<br/>• Visitors<br/>• Temp access |

---

## Permission Definitions

| Permission | ID | Allows | Requires | Used For |
|---------|----|---------|-----------|----|
| **ReadAnyRecord** | 1 | View any patient record | Medical professional role | Accessing medical data |
| **WriteRecord** | 2 | Create/modify records | Clinical role | Entering exam results, prescriptions |
| **ManageAccess** | 3 | Grant/revoke access grants | Senior clinical role | Delegation, consent approval |
| **ManageUsers** | 4 | Create/modify user roles | Supervisor role | User administration, staffing |
| **SystemAdmin** | 5 | System-level changes | Admin role only | Upgrades, configuration, maintenance |

---

## Function Quick Reference

### Assigning Roles

```rust
// Assign a role to a user (with optional expiration)
assign_role(&env, user_address, Role::Optometrist, 0); // 0 = never expires

// Get a user's current role
if let Some(assignment) = get_active_assignment(&env, &user) {
    println!("Role: {:?}", assignment.role);
}
```

### Custom Permission Overrides

```rust
// Grant special permission (override role)
grant_custom_permission(&env, user_address, Permission::SystemAdmin)?;

// Revoke permission (even if role grants it)
revoke_custom_permission(&env, user_address, Permission::ManageAccess)?;
```

### Full Role Delegation

```rust
// Delegate entire role with expiration
delegate_role(
    &env,
    delegator_address,
    delegatee_address,
    Role::Ophthalmologist,
    expires_at_timestamp
);

// Check delegation
if has_delegated_permission(&env, &delegator, &delegatee, &Permission::WriteRecord) {
    println!("Delegatee can write records via delegation");
}
```

### Scoped Permission Delegation

```rust
// Delegate specific permissions only
delegate_permissions(
    &env,
    delegator_address,
    delegatee_address,
    vec![Permission::WriteRecord],  // Only this
    expires_at_timestamp
);

// Check delegation
if has_delegated_permission(&env, &delegator, &delegatee, &Permission::WriteRecord) {
    // ✓ Delegated
} else if has_delegated_permission(&env, &delegator, &delegatee, &Permission::ManageAccess) {
    // ✗ Not delegated
}
```

### Revoking Delegations

```rust
// Revoke ALL delegations from a delegator
let revoked = revoke_delegations_from(&env, &delegator);
for rev in revoked.iter() {
    println!("Revoked {}", rev.is_scoped); // is_scoped: false = full, true = scoped
}
```

### ACL Group Management

```rust
// Create a group
create_group(&env, "researchers", vec![Permission::ReadAnyRecord]);

// Add user to group
add_to_group(&env, user_address, "researchers")?;

// Check group membership (automatic in has_permission)
remove_from_group(&env, user_address, "researchers");

// Delete group
delete_group(&env, "researchers");
```

### Core Access Check

```rust
// THE main function: check if user has permission
if has_permission(&env, &user_address, &Permission::WriteRecord) {
    // Grant access
    println!("Access granted");
} else {
    // Deny access
    println!("Access denied");
}
```

---

## Decision Trees

### "Can user X read records?"

```
START: has_permission(user_x, ReadAnyRecord)?

1. Is user_x active and not expired?
   NO → return false
   YES → continue

2. Is ReadAnyRecord in custom_revokes?
   YES → return false (explicit deny wins)
   NO → continue

3. Is ReadAnyRecord in custom_grants?
   YES → return true ✓
   NO → continue

4. Does user_x's role include ReadAnyRecord?
   [Admin, Ophthalmologist, Optometrist: YES]
   [Staff, Patient, None: NO]
   YES → return true ✓
   NO → continue

5. Is user_x in any group with ReadAnyRecord?
   YES → return true ✓
   NO → continue

6. Default deny → return false
```

### "Can user X perform action Y on behalf of delegator Z?"

```
START: has_delegated_permission(delegator_z, user_x, permission_y)?

1. Is there a full role delegation (delegator_z → user_x)?
   YES:
     a. Is delegation expired?
        YES → skip
        NO → Does delegated role include permission_y?
             YES → return true ✓
             NO → continue
   NO → continue

2. Is there a scoped delegation (delegator_z → user_x)?
   YES:
     a. Is delegation expired?
        YES → skip
        NO → Is permission_y in delegated permissions?
             YES → return true ✓
             NO → continue
   NO → continue

3. Default deny → return false
```

### "Can patient P access their own records?"

```
START: Patient needs to verify own record access

Patient always has implicit access to own records.
Override: If custom_revoke is applied, check revocation.

Additional checks for other users accessing patient records:
1. has_permission(user, ReadAnyRecord)? [RBAC check]
   OR
2. has_delegated_permission(user, ...)?  [Delegation check]
   OR
3. consent_grant_exists(patient_p, user)? [Explicit consent check]

At least ONE must pass for access.
```

---

## Expiration & TTL

### Checking Expiration

```
Timestamp-based expiration used throughout:

expires_at value:
- 0 = Never expires
- n > 0 = Expires at timestamp n
- If current_timestamp >= expires_at (and expires_at != 0) → Expired

Example:
assignment.expires_at = 1704067200  // Jan 1, 2024
if current_time >= 1704067200 → Assignment inactive
```

### Role Assignment Expiration

```rust
// Assign with 30-day expiration
let expires_at = env.ledger().timestamp() + 2_592_000; // 30 days in seconds
assign_role(&env, user, Role::Optometrist, expires_at);

// Check if still active
if let Some(assignment) = get_active_assignment(&env, &user) {
    // User still has role
}
```

### Delegation Expiration

```rust
// Delegate with expiration
delegate_role(&env, delegator, delegatee, Role::Optometrist, expires_at);

// After expiration, delegation is transparently ignored:
has_delegated_permission(&env, &delegator, &delegatee, &perm)?
// → Returns false if delegation has expired
```

---

## Common Patterns

### Pattern 1: Elevate Intern to Senior Developer (Temporary)

```rust
// Initial: Intern has Staff role
assign_role(&env, intern, Role::Staff, never_expires);

// During project: Elevate specific permission
grant_custom_permission(&env, intern, Permission::WriteRecord)?;

// Project ends: Revoke elevation
revoke_custom_permission(&env, intern, Permission::WriteRecord)?;
```

### Pattern 2: Emergency Override (System Admin)

```rust
// Normal: Doctor has Optometrist role
assign_role(&env, doctor, Role::Optometrist, never_expires);

// Emergency: System admin grants SystemAdmin temporarily
grant_custom_permission(&env, doctor, Permission::SystemAdmin)?;

// Later: Revoke the override
revoke_custom_permission(&env, doctor, Permission::SystemAdmin)?;
```

### Pattern 3: On-Call Coverage

```rust
// Scheduled coverage period
let tomorrow = env.ledger().timestamp() + 86_400;
let next_week = tomorrow + (7 * 86_400);

// Primary doctor delegates to on-call
delegate_role(&env, primary_doctor, on_call_doctor, Role::Ophthalmologist, next_week);

// On-call has all Ophthalmologist permissions
// Automatically revoked after one week
```

### Pattern 4: Contractor with Limited Scope

```rust
// Contractor has no role initially
assign_role(&env, contractor, Role::None, project_end_date);

// Grant only WriteRecord for data entry task
delegate_permissions(
    &env,
    admin,
    contractor,
    vec![Permission::WriteRecord],
    project_end_date
);

// Contractor can only write records
// Cannot read, manage access, or manage users
```

### Pattern 5: Research Study Access

```rust
// Researcher has Staff role (minimal)
assign_role(&env, researcher, Role::Staff, study_enrollment_expires);

// Patient grants consent for study
ConsentGrant {
    patient: patient_addr,
    grantee: researcher_addr,
    consent_type: ReadOnly,
    expires_at: study_end_date,
    revoked: false,
}

// Check access: both RBAC and consent required
has_permission(&env, &researcher, &Permission::ReadAnyRecord)?  // Likely false
// PLUS
consent_grant_valid(patient, researcher)?  // Must be true
// One OR the other OR ABAC policy must permit
```

---

## Troubleshooting

### User has permission that shouldn't

1. Check `custom_grants` - explicit grants override roles
2. Check `get_base_permissions(role)` - role includes it
3. Check ACL groups - user in group that grants it
4. Check delegations - incoming delegation grants it
5. Check `custom_revokes` - should deny if present (has priority)

### User doesn't have permission they should

1. Check role expiration: `get_active_assignment` → `expires_at`
2. Check custom_revokes: explicit deny overrides everything
3. Check delegation expiration: delegation may have expired
4. Check group membership: user added to group?
5. Check role assignment exists: `get_active_assignment` returns Some?

### Delegation not working

1. Check delegator is still active (role not revoked)
2. Check delegation not expired: `if del.expires_at < current_timestamp`
3. Check correct delegator/delegatee in `has_delegated_permission`
4. Check full vs scoped: using correct check?
5. Verify permission is in base role (full) or permission list (scoped)

---

## Storage Keys (For Developers)

```rust
// Role assignments
("ROLE_ASN", user_address)              // RoleAssignment

// Full delegations  
("DELEGATE", delegator, delegatee)      // Delegation

// Scoped delegations
("DLG_SCOPE", delegator, delegatee)     // ScopedDelegation

// Indices for fast lookup
("DEL_IDX", delegatee)                  // Vec<Address> [delegators]
("DLGTR_IDX", delegator)                // Vec<Address> [delegatees]

// Groups
("ACL_GRP", group_name)                 // AclGroup
("USR_GRPS", user_address)              // Vec<String> [group_names]

// ABAC
("ACC_POL", policy_id)                  // AccessPolicy
("USER_CRED", user_address)             // CredentialType
("REC_SENS", record_id)                 // SensitivityLevel

// Extended storage (for future use)
("CONSENT", patient, grantee)           // ConsentGrant
```

---

## API Summary

| Function | Purpose | Returns |
|----------|---------|---------|
| `assign_role(env, user, role, expires_at)` | Set user's primary role | - |
| `get_active_assignment(env, user)` | Get user's current role | `Option<RoleAssignment>` |
| `grant_custom_permission(env, user, perm)` | Override grant | `Result<(), ()>` |
| `revoke_custom_permission(env, user, perm)` | Override deny | `Result<(), ()>` |
| `delegate_role(env, from, to, role, expires)` | Full role delegation | - |
| `get_active_delegation(env, from, to)` | Get full delegation | `Option<Delegation>` |
| `delegate_permissions(env, from, to, perms, exp)` | Scoped delegation | - |
| `get_active_scoped_delegation(env, from, to)` | Get scoped delegation | `Option<ScopedDelegation>` |
| `revoke_delegations_from(env, from)` | Revoke all from delegator | `Vec<RevokedDelegation>` |
| `create_group(env, name, perms)` | Create ACL group | - |
| `delete_group(env, name)` | Delete ACL group | - |
| `add_to_group(env, user, group)` | Add user to group | `Result<(), ()>` |
| `remove_from_group(env, user, group)` | Remove user from group | - |
| `get_group_permissions(env, name)` | Get group's permissions | `Vec<Permission>` |
| **`has_permission(env, user, perm)`** | **Main access check** | **`bool`** |
| `has_delegated_permission(env, from, to, perm)` | Delegation-specific check | `bool` |
| `create_access_policy(env, policy)` | Create ABAC policy | - |
| `evaluate_policy(env, policy, context)` | Evaluate single policy | `bool` |
| `evaluate_access_policies(env, user, resource, patient)` | Evaluate all policies | `bool` |
| `set_user_credential(env, user, cred)` | Assign credential type | - |
| `set_record_sensitivity(env, record, sens)` | Set record sensitivity | - |
| `check_policy_engine(env, user, action, resource)` | Policy DSL check | `bool` |
| `simulate_policy_check(env, user, action, resource)` | Dry-run policy check | `SimulationResult` |

---

## Examples

### Example 1: System Administrator Setup

```rust
// Create admin account
assign_role(&env, admin_user, Role::Admin, 0); // Never expires

// Verify permissions
assert!(has_permission(&env, &admin_user, &Permission::SystemAdmin));
assert!(has_permission(&env, &admin_user, &Permission::ManageUsers));
```

### Example 2: Doctor-Patient Interaction

```rust
// Doctor's role is set by admin
assign_role(&env, dr_alice, Role::Ophthalmologist, 0);

// Patient grants explicit consent
let consent = ConsentGrant {
    patient: patient_bob.clone(),
    grantee: dr_alice.clone(),
    consent_type: ConsentType::FullAccess,
    granted_at: env.ledger().timestamp(),
    expires_at: env.ledger().timestamp() + (365 * 86_400), // 1 year
    revoked: false,
};

// Dr. Alice can now access patient's records (via RBAC + consent)
assert!(has_permission(&env, &dr_alice, &Permission::ReadAnyRecord));
```

### Example 3: Emergency Delegation

```rust
// Normally: Dr. David has Staff role
assign_role(&env, dr_david, Role::Staff, 0);

// Emergency: Dr. Carol (Ophthalmologist) delegates to Dr. David
let emergency_timestamp = env.ledger().timestamp() + (8 * 3600); // 8 hours
delegate_role(&env, dr_carol, dr_david, Role::Ophthalmologist, emergency_timestamp);

// Dr. David now has Ophthalmologist permissions for 8 hours
assert!(has_delegated_permission(&env, &dr_carol, &dr_david, &Permission::WriteRecord));

// After 8 hours: delegation expires
// ... time passes ...
assert!(!has_delegated_permission(&env, &dr_carol, &dr_david, &Permission::WriteRecord));
```

---

## See Also

- [Complete Architecture Guide](access-control-architecture.md)
- [Implementation: contracts/vision_records/src/rbac.rs](../contracts/vision_records/src/rbac.rs)
- [Tests: contracts/vision_records/tests/test_rbac.rs](../contracts/vision_records/tests/test_rbac.rs)
