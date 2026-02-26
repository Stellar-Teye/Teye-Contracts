# Access Control Architecture

## Overview

The Teye system implements a comprehensive, multi-layered access control architecture that combines:

- **Role-Based Access Control (RBAC)** - Coarse-grained role hierarchy
- **Attribute-Based Access Control (ABAC)** - Fine-grained context-aware policies
- **Delegation** - Trust delegation and role/permission transfer
- **Access Control Lists (ACLs) via Groups** - Bulk permission management
- **Consent Grants** - Patient-level access control
- **Policy Engine** - Composable policy DSL for complex authorization logic

This design enables flexible, auditable access control suitable for healthcare with HIPAA compliance considerations.

---

## 1. Role-Based Access Control (RBAC)

### 1.1 Role Hierarchy

The system defines a 5-level role hierarchy with increasing privileges:

```
Admin (5) ──┐
            ├── System-wide permissions
            │
Ophthalmologist (4)
            ├── Extended clinical permissions
            │
Optometrist (3) ──────┐
            ├── Clinical permissions
            │
Staff (2) ────────────┼── Operational permissions
            │
Patient (1) ──────────┴── Personal record management
            │
None (0) ────────────────── Unassigned / Visitor
```

### 1.2 Role Definitions and Base Permissions

| Role | Level | Base Permissions | Use Case |
|------|-------|------------------|----------|
| **Admin** | 5 | SystemAdmin, ManageUsers, ManageAccess, WriteRecord, ReadAnyRecord | Full system control, infrastructure changes |
| **Ophthalmologist** | 4 | ManageUsers, ManageAccess, WriteRecord, ReadAnyRecord | Senior eye specialist, complex cases, staff management |
| **Optometrist** | 3 | ManageUsers, ManageAccess, WriteRecord, ReadAnyRecord | Eye specialist, basic management |
| **Staff** | 2 | ManageUsers | Administrative support, data entry assistance |
| **Patient** | 1 | *None* | Personal record access (implicit), consent management |
| **None** | 0 | *None* | No automatic permissions |

**Permission Definitions:**

| Permission | ID | Description | Roles |
|------------|----|-----------|---------:|
| **ReadAnyRecord** | 1 | View any patient's records | Ophthalmologist, Optometrist |
| **WriteRecord** | 2 | Create/update vision records | Ophthalmologist, Optometrist |
| **ManageAccess** | 3 | Grant/revoke record access | Ophthalmologist, Optometrist, Ophthalmologist |
| **ManageUsers** | 4 | Create/modify user roles | Admin, Ophthalmologist, Optometrist, Staff |
| **SystemAdmin** | 5 | Contract upgrades, configuration | Admin only |

### 1.3 Permission Evaluation

When `has_permission(user, permission)` is called, the system evaluates in priority order:

```
┌─────────────────────────────────────────────┐
│ 1. Check Custom Revocation                  │
│    If custom_revokes contains permission    │
│    → DENY (return false)                    │
└─────────────────────────────────────────────┘
              ↓ (not denied)
┌─────────────────────────────────────────────┐
│ 2. Check Custom Grant                       │
│    If custom_grants contains permission     │
│    → ALLOW (return true)                    │
└─────────────────────────────────────────────┘
              ↓ (not granted)
┌─────────────────────────────────────────────┐
│ 3. Check Base Role Permissions              │
│    If role.base_permissions contains        │
│    → ALLOW (return true)                    │
└─────────────────────────────────────────────┘
              ↓ (not in base)
┌─────────────────────────────────────────────┐
│ 4. Check ACL Group Membership               │
│    For each group user belongs to:          │
│      If group.permissions contains          │
│      → ALLOW (return true)                  │
└─────────────────────────────────────────────┘
              ↓ (not in group)
┌─────────────────────────────────────────────┐
│ 5. Default Deny                             │
│    → DENY (return false)                    │
└─────────────────────────────────────────────┘
```

**Priority Precedence (Highest to Lowest):**
1. Custom Revoke (explicit deny override)
2. Custom Grant (explicit allow override)
3. Base Role Permissions
4. ACL Group Permissions
5. Delegation (evaluated separately)
6. Default: Deny

---

## 2. Delegation System

The delegation system enables users to transfer their permissions to others temporarily or permanently.

### 2.1 Full Role Delegation

**Definition**: One user delegates their entire role (and all its base permissions) to another user.

**Use Cases:**
- Coverage during absences
- Team members assisting with workload
- Mentorship and supervision

**Example Scenarios:**

```rust
// Scenario: Dr. Alice on vacation, delegates to Dr. Bob
delegate_role(
    &env,
    dr_alice,           // delegator
    dr_bob,             // delegatee
    Role::Ophthalmologist,
    next_month_timestamp // expires_at
);

// Result: Dr. Bob now has all Ophthalmologist permissions
// - ReadAnyRecord ✓
// - WriteRecord ✓
// - ManageAccess ✓
// - ManageUsers ✓
```

**Delegation Check:**
```rust
has_delegated_permission(
    &env,
    &dr_alice,        // delegator
    &dr_bob,          // delegatee
    &Permission::WriteRecord
) → true
```

**Storage Schema:**
```
Key: ("DELEGATE", delegator, delegatee)
Value: {
    delegator: Address,
    delegatee: Address,
    role: Role,
    expires_at: u64
}
```

### 2.2 Scoped Delegation

**Definition**: One user delegates specific permissions (not a full role) to another user.

**Use Cases:**
- Contractor with limited access
- Temporary authorization for specific tasks
- Least-privilege principle

**Example Scenarios:**

```rust
// Scenario: Contractor can write records but not manage access
delegate_permissions(
    &env,
    hospital_admin,
    contractor,
    vec![Permission::WriteRecord],
    end_of_project_timestamp
);

// Result: Contractor has ONLY WriteRecord from this delegation
// - ReadAnyRecord ✗ (not delegated)
// - WriteRecord ✓ (delegated)
// - ManageAccess ✗ (not delegated)
```

**Storage Schema:**
```
Key: ("DLG_SCOPE", delegator, delegatee)
Value: {
    delegator: Address,
    delegatee: Address,
    permissions: Vec<Permission>,
    expires_at: u64
}
```

### 2.3 Delegation Indices

For efficient permission lookups, the system maintains two indices:

**Delegatee Index** (Who can delegate to me):
```
Key: ("DEL_IDX", delegatee)
Value: Vec<Address> [list of delegators]

Purpose: Fast lookups for "check all my incoming delegations"
```

**Delegator Index** (Who I delegate to):
```
Key: ("DLGTR_IDX", delegator)
Value: Vec<Address> [list of delegatees]

Purpose: Cascade cleanup when delegator revokes all delegations
```

### 2.4 Delegation Revocation

**Full Revocation:**
```rust
revoke_delegations_from(&env, &delegator)
→ Removes ALL delegations from this delegator
→ Updates delegatee indices
→ Returns detailed list of what was revoked
```

**Selective Revocation:**
```rust
// For individual revocations, delete from storage
// and update indices manually
```

---

## 3. Custom Grants and Revokes

The RBAC system allows fine-grained override of role-based permissions.

### 3.1 Custom Grant

**Definition**: Explicitly grant a permission to a user, overriding their base role.

**Use Cases:**
- Temporary elevation (intern becomes core contributor)
- Special projects (researcher needs extended access)
- Exception handling

```rust
grant_custom_permission(&env, intern, Permission::WriteRecord)?;

// Result: Intern can now WriteRecord even if Staff role doesn't include it
// Order of removal from revokes: if same permission in revokes, removes from revokes
```

**Storage Impact:**
```
RoleAssignment {
    role: Staff,
    custom_grants: [WriteRecord],  // ← Added
    custom_revokes: [],
    expires_at: 0
}
```

### 3.2 Custom Revoke

**Definition**: Explicitly deny a permission to a user, overriding their role and delegations.

**Use Cases:**
- Disciplinary action (temporarily revoke WriteRecord)
- Security incident response (revoke access immediately)
- Least-privilege enforcement

```rust
revoke_custom_permission(&env, compromised_staff, Permission::ManageAccess)?;

// Result: Staff member loses ManageAccess despite role granting it
// Overrides: base permissions, custom grants, and delegations
```

**Storage Impact:**
```
RoleAssignment {
    role: Staff,
    custom_grants: [],
    custom_revokes: [ManageAccess],  // ← Added
    expires_at: 0
}
```

**Priority (Highest → Lowest):**
1. ✓ Custom Revoke → Deny (absolute)
2. ✓ Custom Grant → Allow  
3. ✓ Base Role Permissions → Allow
4. ✓ Delegations → Allow
5. → Deny

---

## 4. Access Control Lists (via Groups)

The system supports named groups of permissions for bulk management.

### 4.1 Group Definition

**Definition**: A named collection of permissions that can be assigned to users.

```rust
AclGroup {
    name: "Researchers",
    permissions: vec![Permission::ReadAnyRecord]
}
```

**Membership:**
```
Key: ("USR_GRPS", user_address)
Value: Vec<String> ["Researchers", "Specialists"]
```

### 4.2 Group Operations

**Create Group:**
```rust
create_group(&env, "researchers", vec![Permission::ReadAnyRecord]);
```

**Add User to Group:**
```rust
add_to_group(&env, researcher1, "researchers")?;
```

**Remove User from Group:**
```rust
remove_from_group(&env, researcher1, "researchers");
```

**Delete Group:**
```rust
delete_group(&env, "researchers");
```

**Permission Inheritance:**
```
Has permission if:
  - User belongs to group AND
  - Group contains permission

Check: for_each group in user.groups:
         if group.permissions.contains(permission)
           return true
```

---

## 5. Attribute-Based Access Control (ABAC)

### 5.1 Policy Types

**Time-Based Restrictions:**
```rust
pub enum TimeRestriction {
    None,                      // Always allow
    BusinessHours,             // 9 AM - 5 PM UTC only
    HourRange(9, 17),          // Custom hour ranges
    DaysOfWeek(0b0011111),     // Specific days (bitmask)
}
```

**Credentials:**
```rust
pub enum CredentialType {
    None,                      // No credential required
    MedicalLicense,            // Valid medical license
    ResearchCredentials,       // Research authorization
    EmergencyCredentials,      // Emergency override
    AdminCredentials,          // Admin authorization
}
```

**Sensitivity Levels:**
```rust
pub enum SensitivityLevel {
    Public,                    // Accessible to all
    Standard,                  // Generic health records
    Confidential,              // Sensitive medical info
    Restricted,                // Highly sensitive (genetics, etc.)
}
```

### 5.2 Policy Evaluation

**Policy Context:**
```rust
PolicyContext {
    user: Address,             // Who is requesting
    resource_id: Option<u64>,  // What record
    patient: Option<Address>,  // Patient (if different from resource owner)
    current_time: u64,         // When (for time-based checks)
}
```

**Policy Conditions (All must pass):**
```rust
PolicyConditions {
    required_role: Role,                    // User must have this role
    time_restriction: TimeRestriction,      // Must satisfy time constraint
    required_credential: CredentialType,    // Must have credential
    min_sensitivity_level: SensitivityLevel,// Can access records of this level
    consent_required: bool,                 // Patient must consent
}
```

**Evaluation Logic:**
```
1. Check if policy is enabled
   → Fail if disabled

2. Check required_role
   → User's assigned role must match
   
3. Check time_restriction
   → Current time must satisfy constraint

4. Check required_credential
   → User must have this credential type

5. Check min_sensitivity_level (if resource_id provided)
   → Record sensitivity ≥ user's minimum accessible level

6. Check consent_required
   → Patient must have active, non-revoked consent grant

All conditions must pass for policy to grant access
```

---

## 6. Policy Engine Integration

### 6.1 Context Building

```rust
pub fn build_eval_context(
    env: &Env,
    user: &Address,
    action: &str,
    resource_id: Option<u64>
) → EvalContext
```

The context builder auto-populates user attributes:
- User's role (from RoleAssignment)
- User's credentials
- Resource sensitivity level (from storage)

### 6.2 Policy Evaluation

```rust
let ctx = build_eval_context(&env, &user, "read_record", Some(record_id));
let result = check_policy_engine(&env, &user, "read_record", Some(record_id));

// Returns: true if policy permits, false otherwise
```

### 6.3 Policy Simulation

```rust
SimulationResult {
    effect: Permit | Deny,
    reason: String,
    applicable_policies: Vec<String>,
    failing_conditions: Vec<String>
}
```

Useful for debugging and what-if analysis.

---

## 7. Consent Grants

### 7.1 Patient-Level Access Control

**Definition**: Patients explicitly grant other users access to their records.

```rust
ConsentGrant {
    patient: Address,
    grantee: Address,
    consent_type: ConsentType,
    granted_at: u64,
    expires_at: u64,
    revoked: bool
}
```

**Consent Types:**
```rust
pub enum ConsentType {
    FullAccess,                // All records
    ReadOnly,                  // View only
    SpecificRecord(record_id), // Single record
    Timebound(expires_at),     // Valid until timestamp
}
```

**Workflow:**

```
1. Doctor requests access:
   "Can I view your records?"
   
2. Patient grants consent:
   ConsentGrant {
       patient: patient_addr,
       grantee: doctor_addr,
       consent_type: FullAccess,
       expires_at: next_year
   }
   
3. Doctor checks access:
   - has_permission(doctor, ReadAnyRecord) ✓
   - check_consent(patient, doctor) ✓
   → Both needed for access
   
4. Patient revokes:
   consent_grant.revoked = true
   → Doctor loses access
```

---

## 8. Complete Access Decision Flow

**Request:** "Can user X perform action Y on resource Z?"

```
START
├─ 1. Get user's active RoleAssignment
│  ├─ If None → DENY
│  ├─ If expired → DENY
│  └─ Store role, custom_grants, custom_revokes
│
├─ 2. Check Custom Revoke
│  ├─ If permission in custom_revokes → DENY ✗
│  └─ Continue
│
├─ 3. Check Custom Grant
│  ├─ If permission in custom_grants → ALLOW ✓
│  └─ Continue
│
├─ 4. Check Base Role Permissions
│  ├─ If get_base_permissions(role) contains permission
│  │  → ALLOW ✓
│  └─ Continue
│
├─ 5. Check ACL Group Memberships
│  ├─ For each group in user.groups:
│  │  ├─ If group.permissions contains permission
│  │  │  → ALLOW ✓
│  │  └─ Continue
│  └─ Continue
│
├─ 6. Check RBAC Result
│  ├─ If any allow above → RBAC = true
│  └─ Else → RBAC = false
│
├─ 7. Check ABAC Policies (if applicable)
│  ├─ For each configured policy:
│  │  ├─ Evaluate all conditions:
│  │  │  ├─ Role match?
│  │  │  ├─ Time restriction OK?
│  │  │  ├─ Credentials present?
│  │  │  ├─ Sensitivity level OK?
│  │  │  └─ Consent granted?
│  │  ├─ If all satisfied → ABAC = true
│  │  └─ Continue
│  └─ If any policy permits → ABAC = true
│
├─ 8. Check Delegations (if RBAC false)
│  ├─ For each delegation → user:
│  │  ├─ Is it active (not expired)?
│  │  ├─ Full role: does granted role have permission?
│  │  ├─ Scoped: is permission in delegated list?
│  │  └─ If yes → DELEGATION = true
│  └─ Continue
│
├─ 9. Check Consent (if resource is patient record)
│  ├─ Does patient consent exist?
│  ├─ Is it revoked?
│  ├─ Is it expired?
│  └─ If valid → CONSENT = true
│
├─ 10. Final Decision
│  ├─ If RBAC = true → ALLOW ✓
│  ├─ Else if ABAC = true → ALLOW ✓
│  ├─ Else if DELEGATION = true → ALLOW ✓
│  ├─ Else if CONSENT = true → ALLOW ✓
│  └─ Else → DENY ✗
│
END
```

---

## 9. Storage Schema Summary

| Key Pattern | Value Type | Purpose |
|-------------|-----------|---------|
| `("ROLE_ASN", user)` | RoleAssignment | User's current role and custom permissions |
| `("DELEGATE", delegator, delegatee)` | Delegation | Full role delegation agreement |
| `("DLG_SCOPE", delegator, delegatee)` | ScopedDelegation | Permission-specific delegation |
| `("DEL_IDX", delegatee)` | Vec<Address> | Index of delegators → delegatee |
| `("DLGTR_IDX", delegator)` | Vec<Address> | Index of delegatees ← delegator |
| `("ACL_GRP", name)` | AclGroup | Named group definition |
| `("USR_GRPS", user)` | Vec<String> | Groups user belongs to |
| `("ACC_POL", id)` | AccessPolicy | ABAC policy definition |
| `("USER_CRED", user)` | CredentialType | User's verified credential |
| `("REC_SENS", record_id)` | SensitivityLevel | Record sensitivity classification |
| `("CONSENT", patient, grantee)` | ConsentGrant | Consent grant agreement |

---

## 10. Example Scenarios

### Scenario 1: Dr. Alice Grants Dr. Bob Access (Coverage)

```
Dr. Alice (Ophthalmologist) is on 2-week vacation.
Dr. Bob (Staff) will cover her work.

1. Admin assigns Dr. Alice:
   assign_role(alice, Ophthalmologist, never_expires)
   
2. Admin assigns Dr. Bob:
   assign_role(bob, Staff, never_expires)
   
3. Dr. Alice delegates:
   delegate_role(alice, bob, Ophthalmologist, in_2_weeks)
   
4. Check access:
   has_permission(bob, ReadAnyRecord)?
   - Direct role: Staff → No ReadAnyRecord
   - Delegations: From Alice → Ophthalmologist → Yes ReadAnyRecord ✓
   → ALLOW

5. In 2 weeks, delegation expires:
   has_permission(bob, ReadAnyRecord)?
   → DENY (delegation expired)
```

### Scenario 2: Patient Grants Research Access

```
Patient P wants to contribute to research study.
Researcher R requests access to patient's records.

1. Patient creates consent:
   ConsentGrant {
       patient: patient_p,
       grantee: researcher_r,
       consent_type: ReadOnly,
       expires_at: end_of_study
   }
   
2. Researcher R attempts access:
   has_permission(researcher_r, ReadAnyRecord)?
   - Direct role: Maybe not directly
   - ABAC Policy requires consent → Check ✓
   - Consent valid and not revoked ✓
   → ALLOW

3. Patient revokes (early):
   consent_grant.revoked = true
   
4. Researcher R attempts access again:
   → DENY (consent revoked)
```

### Scenario 3: Contractor with Limited Scope

```
Contractor C hired to update patient databases.
Only needs WriteRecord, not ManageAccess.

1. Admin assigns:
   assign_role(contractor_c, None, at_end_of_project)
   
2. Admin grants scoped delegation:
   delegate_permissions(
       admin,
       contractor_c,
       vec![Permission::WriteRecord],
       at_end_of_project
   )
   
3. Check permissions:
   has_permission(contractor_c, WriteRecord)?
     → Scoped delegation grants it ✓
   has_permission(contractor_c, ManageAccess)?
     → Not in role, not in delegation → DENY

4. Project ends:
   Delegation expires automatically
   All contractor access revoked
```

---

## 11. Security Considerations

### 11.1 Custom Revoke Authority

Custom revokes take absolute highest priority. Only Admin should have authority to revoke permissions even from higher roles (e.g., Admin can revoke from Ophthalmologist if compromise suspected).

### 11.2 Delegation Expiration

All delegations respect TTL and expiration timestamps. Delegations **do not grant permanent change** in role, only temporary permission use.

### 11.3 Consent Management

Patients must explicitly consent for non-role-based access. Consent can be revoked immediately, even if delegation exists.

### 11.4 Audit Trail

Every access decision should log:
- User, action, resource
- Decision (allow/deny)
- Reason (which check passed)
- Timestamp
- Applicable policies/delegations

### 11.5 Rate Limiting

Combine with the system's rate limiting to prevent abuse of access checks or delegation grants.

---

## 12. Implementation Checklist

- [x] Role hierarchy defined (Patient → Admin)
- [x] Base permissions mapped to roles
- [x] `has_permission()` evaluates in correct priority
- [x] Custom grants/revokes implemented
- [x] Full role delegation working
- [x] Scoped delegation working
- [x] ACL groups functional
- [x] ABAC policies implemented
- [x] Consent grants working
- [x] Delegation expiration checked
- [x] Audit logging integration point
- [x] Policy engine integration points defined
- [x] Storage keys documented
- [x] Example scenarios tested

---

## 13. References

- [RBAC Quick Reference](rbac-quick-reference.md)
- [contracts/vision_records/src/rbac.rs](../contracts/vision_records/src/rbac.rs) - Implementation
- [contracts/vision_records/tests/test_rbac.rs](../contracts/vision_records/tests/test_rbac.rs) - Test suite
