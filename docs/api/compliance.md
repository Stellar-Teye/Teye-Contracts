# Compliance Contract API Reference

## Contract Purpose

Enforces healthcare compliance requirements (HIPAA, GDPR, BAA) and provides role-based access control for medical records. Tracks audit logs for regulatory compliance and manages consent requirements.

## Core Components

The Compliance contract is structured as a collection of sub-modules that work together:

- **access_control** — Role and permission management
- **audit** — Compliance audit trail logging
- **baa** — Business Associate Agreement tracking
- **retention** — Data retention policy enforcement

---

## Roles & Permissions

### Role Types

```rust
pub enum Role {
    Admin,                // Full access, configuration
    Clinician,           // Read + write on patient records
    Researcher,          // Read-only access
    Auditor,             // Read-only + audit access
    Patient,             // Read-only on own records
}
```

### Permission Matrix

| Role       | Read | Write | Audit |
| ---------- | ---- | ----- | ----- |
| Admin      | ✓    | ✓     | ✓     |
| Clinician  | ✓    | ✓     | ✗     |
| Researcher | ✓    | ✗     | ✗     |
| Auditor    | ✓    | ✗     | ✓     |
| Patient    | ✓    | ✗     | ✗     |

---

## Access Control Functions

### Role Management

#### `assign_role(env: Env, caller: Address, user: Address, role: Role) -> Result<(), ComplianceError>`

Assign a role to a user (admin only).

**Parameters:**

- `caller` - Admin address (must authenticate)
- `user` - User to assign role to
- `role` - Role from the enum

**Returns:** `Result<(), ComplianceError>`

**Errors:**

- `Unauthorized` - Caller is not admin
- `InvalidRole` - Role invalid or not permitted

**Example:**

```rust
client.assign_role(&env, &admin, &user, &Role::Clinician)?;
```

---

#### `check_permission(env: Env, user: Address, role: Role, permission: &str) -> bool`

Verify a user has a specific permission.

**Parameters:**

- `user` - User to check
- `role` - User's role
- `permission` - Permission string: "read", "write", or "audit"

**Returns:** `bool` — true if user has permission

---

#### `get_user_role(env: Env, user: Address) -> Option<Role>`

Retrieve the current role of a user.

**Parameters:**

- `user` - User address

**Returns:** `Option<Role>`

---

#### `revoke_role(env: Env, caller: Address, user: Address) -> Result<(), ComplianceError>`

Remove all roles from a user (admin only).

**Parameters:**

- `caller` - Admin address
- `user` - User to deauthorize

**Returns:** `Result<(), ComplianceError>`

---

## Audit & Compliance Functions

### Audit Logging

#### `log_access(env: Env, user: Address, record_id: u64, action: &str, result: bool) -> Result<(), ComplianceError>`

Record an access attempt for compliance auditing.

**Parameters:**

- `user` - Accessing user
- `record_id` - Record being accessed
- `action` - Action string ("read", "write", "delete")
- `result` - Success/failure of operation

**Returns:** `Result<(), ComplianceError>`

**Storage:**
Persists audit entry with:

- Timestamp
- User address
- Record ID
- Action
- Success/failure result

**Example:**

```rust
client.log_access(&env, &user, &record_id, "read", true)?;
```

---

#### `get_audit_log(env: Env, record_id: u64) -> Vec<AuditEntry>`

Retrieve all access attempts for a record.

**Parameters:**

- `record_id` - Record to audit

**Returns:** Vector of audit entries (time-sorted)

---

#### `get_user_audit_history(env: Env, user: Address) -> Vec<AuditEntry>`

Retrieve all actions performed by a user.

**Parameters:**

- `user` - User to audit

**Returns:** All audit entries for this user

---

### Data Retention

#### `set_retention_policy(env: Env, caller: Address, record_type: Symbol, retention_days: u64) -> Result<(), ComplianceError>`

Configure how long records must be retained (admin only).

**Parameters:**

- `caller` - Admin address
- `record_type` - Type of record (e.g., `symbol_short!("EXAM")`)
- `retention_days` - Minimum retention period in days

**Returns:** `Result<(), ComplianceError>`

**Example:**

```rust
// Retain exam records for 7 years (2555 days)
client.set_retention_policy(&env, &admin, &symbol_short!("EXAM"), 2555)?;
```

---

#### `get_retention_policy(env: Env, record_type: Symbol) -> u64`

Get the retention period for a record type.

**Parameters:**

- `record_type` - Record type

**Returns:** Retention period in days

---

#### `check_retention_expired(env: Env, record_id: u64, created_at: u64, record_type: Symbol) -> bool`

Determine if a record's retention period has expired and deletion is allowed.

**Parameters:**

- `record_id` - Record to check
- `created_at` - Record creation timestamp
- `record_type` - Type of record

**Returns:** `bool` — true if deletion permitted

---

### Business Associate Agreement (BAA)

#### `register_baa_entity(env: Env, caller: Address, entity: Address, entity_type: Symbol) -> Result<(), ComplianceError>`

Register a Business Associate under HIPAA BAA (admin only).

**Parameters:**

- `caller` - Admin address
- `entity` - Entity becoming BAA signatory
- `entity_type` - Type of entity (e.g., `symbol_short!("VENDOR")`)

**Returns:** `Result<(), ComplianceError>`

**Example:**

```rust
client.register_baa_entity(&env, &admin, &vendor_address, &symbol_short!("VENDOR"))?;
```

---

#### `verify_baa_status(env: Env, entity: Address) -> bool`

Check if an entity is a valid BAA signatory.

**Parameters:**

- `entity` - Entity to verify

**Returns:** `bool` — true if BAA registered

---

#### `revoke_baa(env: Env, caller: Address, entity: Address) -> Result<(), ComplianceError>`

Revoke BAA status from an entity.

**Parameters:**

- `caller` - Admin address
- `entity` - Entity to revoke

**Returns:** `Result<(), ComplianceError>`

**Events:**

- `baa_revoked(entity, timestamp)`

---

## Policy Evaluation

### Advanced Access Checks

#### `evaluate_access_policies(env: Env, user: Address, record_id: u64, action: &str) -> Result<bool, ComplianceError>`

Evaluate comprehensive access policies including role, consent, and attributes.

**Parameters:**

- `user` - User requesting access
- `record_id` - Record being accessed
- `action` - Requested action (read, write)

**Returns:** `Result<bool, ComplianceError>` — true if access permitted

**Checks:**

1. Role-based access (user's role permission)
2. Consent verification (for patient access)
3. Attribute-based policies (sensitivity level, access time)
4. Time-of-access restrictions

**Errors:**

- `Unauthorized` - User lacks permission for action

**Example:**

```rust
if client.evaluate_access_policies(&env, &user, &record_id, "read")? {
    // Access granted
}
```

---

## Data Types

### AuditEntry

```rust
pub struct AuditEntry {
    pub user: Address,
    pub record_id: u64,
    pub action: String,
    pub result: bool,          // success/failure
    pub timestamp: u64,
}
```

---

## Storage Keys

| Key                | Purpose                   |
| ------------------ | ------------------------- |
| Per-user role      | User role assignment      |
| Audit trail        | Access history per record |
| Retention policies | Policy per record type    |
| BAA registry       | Registered BAA entities   |

---

## Error Codes

| Error                 | Code | Description                             |
| --------------------- | ---- | --------------------------------------- |
| `NotInitialized`      | 1    | Contract not initialized                |
| `AlreadyInitialized`  | 2    | Contract already initialized            |
| `Unauthorized`        | 3    | Caller lacks required permission        |
| `InvalidRole`         | 4    | Invalid role assignment                 |
| `RecordNotFound`      | 5    | Record does not exist                   |
| `RetentionNotExpired` | 6    | Cannot delete — retention period active |
| `InvalidPolicyConfig` | 7    | Invalid policy configuration            |

---

## Events

| Event            | Parameters                          | Description      |
| ---------------- | ----------------------------------- | ---------------- |
| `role_assigned`  | `(user, role)`                      | Role assigned    |
| `role_revoked`   | `(user)`                            | Role revoked     |
| `access_logged`  | `(user, record_id, action, result)` | Access recorded  |
| `baa_registered` | `(entity, entity_type)`             | BAA entity added |
| `baa_revoked`    | `(entity, timestamp)`               | BAA revoked      |
| `access_denied`  | `(user, record_id, reason)`         | Access denied    |

---

## Compliance Standards

| Standard   | Coverage                                         |
| ---------- | ------------------------------------------------ |
| **HIPAA**  | Role-based access, audit logging, BAA management |
| **GDPR**   | Data retention policies, right to be forgotten   |
| **HITECH** | Breach notification triggers via audit           |

---

## Related Documentation

- [RBAC Design](../docs/governance.md#rbac)
- [Audit Logging](../docs/audit-logging.md)
- [Data Retention](../docs/data-portability.md#retention)
- [Security Audit Checklist](../docs/security-audit-checklist.md)
