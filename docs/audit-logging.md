# Access Audit Logging

This document provides overview of audit logging in Stellar Teye platform. For comprehensive audit trail documentation, see [Compliance Audit Trail](compliance/audit-trail.md).

## Quick Start

See [Compliance Documentation](compliance/) for complete audit trail implementation and HIPAA compliance details.

The Vision Records contract implements comprehensive audit logging for all access events. This system tracks who accessed what records, when, and whether the access was successful or denied. Audit logs are immutable and provide a complete trail of all access attempts for compliance and security purposes.

## Features

- **Comprehensive Logging**: All record access attempts are logged, including reads, writes, access grants, and revocations
- **Success/Failure Tracking**: Each audit entry records whether the access attempt succeeded, failed, was denied, or encountered an error
- **Multiple Query Options**: Audit logs can be queried by record, user, patient, action type, result, or time range
- **Immutable History**: Audit entries are never deleted, providing a permanent record of all access events
- **Event Publishing**: All audit entries trigger on-chain events for off-chain indexing and monitoring

## Data Structures

### AccessAction

Enumeration of access action types:

```rust
pub enum AccessAction {
    Read = 1,              // Reading a record
    Write = 2,             // Creating or updating a record
    Delete = 3,            // Deleting a record (future)
    GrantAccess = 4,        // Granting access to a user
    RevokeAccess = 5,      // Revoking access from a user
    EmergencyAccess = 6,   // Accessing via emergency protocol
    Query = 7,             // Querying records
}
```

### AccessResult

Enumeration of access attempt results:

```rust
pub enum AccessResult {
    Success = 1,    // Access was successful
    Failure = 2,    // Access failed due to error
    Denied = 3,     // Access was denied due to permissions
    NotFound = 4,   // Record was not found
    Expired = 5,    // Access grant expired
}
```

### AuditEntry

Structure representing a single audit log entry:

```rust
pub struct AuditEntry {
    pub id: u64,                    // Unique audit entry ID
    pub timestamp: u64,              // Unix timestamp of the access
    pub actor: Address,              // Address of the user who performed the action
    pub patient: Address,            // Address of the patient whose record was accessed
    pub record_id: Option<u64>,     // Record ID if applicable
    pub action: AccessAction,        // Type of action performed
    pub result: AccessResult,        // Result of the access attempt
    pub reason: Option<String>,     // Failure reason or additional context
    pub ip_address: Option<String>,  // Optional IP address (for off-chain tracking)
    pub user_agent: Option<String>,  // Optional user agent (for off-chain tracking)
}
```

## API Reference

### Logging Functions

Audit logging is automatically performed by the contract when access functions are called. No manual logging is required.

### Query Functions

#### Get Audit Entry by ID

```rust
pub fn get_audit_entry(env: Env, entry_id: u64) -> Result<AuditEntry, ContractError>
```

Retrieves a specific audit entry by its ID.

**Parameters:**
- `entry_id`: The unique ID of the audit entry

**Returns:**
- `Ok(AuditEntry)` if the entry exists
- `Err(ContractError::RecordNotFound)` if the entry doesn't exist

#### Get Record Audit Log

```rust
pub fn get_record_audit_log(env: Env, record_id: u64) -> Vec<AuditEntry>
```

Retrieves all audit entries for a specific record, ordered by timestamp (oldest first).

**Parameters:**
- `record_id`: The ID of the record

**Returns:**
- `Vec<AuditEntry>`: All audit entries for the record

#### Get User Audit Log

```rust
pub fn get_user_audit_log(env: Env, user: Address) -> Vec<AuditEntry>
```

Retrieves all audit entries for a specific user (actor), showing all actions performed by that user.

**Parameters:**
- `user`: The address of the user

**Returns:**
- `Vec<AuditEntry>`: All audit entries where the user was the actor

#### Get Patient Audit Log

```rust
pub fn get_patient_audit_log(env: Env, patient: Address) -> Vec<AuditEntry>
```

Retrieves all audit entries for a specific patient, showing all access attempts to their records.

**Parameters:**
- `patient`: The address of the patient

**Returns:**
- `Vec<AuditEntry>`: All audit entries for the patient's records

#### Get Audit Log by Action

```rust
pub fn get_audit_log_by_action(env: Env, action: AccessAction) -> Vec<AuditEntry>
```

Retrieves all audit entries filtered by action type.

**Parameters:**
- `action`: The type of action to filter by

**Returns:**
- `Vec<AuditEntry>`: All audit entries matching the action type

#### Get Audit Log by Result

```rust
pub fn get_audit_log_by_result(env: Env, result: AccessResult) -> Vec<AuditEntry>
```

Retrieves all audit entries filtered by result.

**Parameters:**
- `result`: The result type to filter by

**Returns:**
- `Vec<AuditEntry>`: All audit entries matching the result

#### Get Audit Log by Time Range

```rust
pub fn get_audit_log_by_time_range(
    env: Env,
    start_time: u64,
    end_time: u64,
) -> Vec<AuditEntry>
```

Retrieves all audit entries within a specific time range.

**Parameters:**
- `start_time`: Unix timestamp of the start time (inclusive)
- `end_time`: Unix timestamp of the end time (inclusive)

**Returns:**
- `Vec<AuditEntry>`: All audit entries within the time range

#### Get Recent Audit Log

```rust
pub fn get_recent_audit_log(env: Env, limit: u64) -> Vec<AuditEntry>
```

Retrieves the most recent N audit entries.

**Parameters:**
- `limit`: Maximum number of entries to retrieve

**Returns:**
- `Vec<AuditEntry>`: The most recent audit entries

## Events

### AuditLogEntryEvent

Published whenever an audit entry is created:

```rust
pub struct AuditLogEntryEvent {
    pub entry_id: u64,
    pub actor: Address,
    pub patient: Address,
    pub record_id: Option<u64>,
    pub action: AccessAction,
    pub result: AccessResult,
    pub reason: Option<String>,
    pub timestamp: u64,
}
```

**Event Topics:**
- `("AUDIT", actor, patient)`

## Access Points with Audit Logging

The following contract functions automatically create audit entries:

1. **`get_record`**: Logs read access attempts (success or denied)
2. **`add_record`**: Logs write access attempts (success or denied)
3. **`grant_access`**: Logs access grant attempts
4. **`revoke_access`**: Logs access revocation attempts
5. **`access_record_via_emergency`**: Logs emergency access attempts

## Usage Examples

### Querying Audit Logs

```rust
// Get all access attempts for a specific record
let record_audit = client.get_record_audit_log(&record_id);

// Get all actions performed by a user
let user_audit = client.get_user_audit_log(&user_address);

// Get all access attempts to a patient's records
let patient_audit = client.get_patient_audit_log(&patient_address);

// Get all denied access attempts
let denied_attempts = client.get_audit_log_by_result(&AccessResult::Denied);

// Get all read operations
let read_operations = client.get_audit_log_by_action(&AccessAction::Read);

// Get audit entries from the last 24 hours
let start_time = current_time - 86400;
let end_time = current_time;
let recent_audit = client.get_audit_log_by_time_range(&start_time, &end_time);

// Get the 100 most recent audit entries
let recent = client.get_recent_audit_log(&100);
```

### Analyzing Access Patterns

Audit logs can be used to analyze access patterns:

1. **Frequency Analysis**: Count how often records are accessed
2. **User Activity**: Track which users access which records
3. **Failed Access Attempts**: Identify potential security issues
4. **Time-based Analysis**: Understand access patterns over time
5. **Compliance Reporting**: Generate reports for regulatory compliance

## Security Considerations

1. **Immutable Logs**: Audit entries are never deleted, ensuring a complete historical record
2. **Permission Checks**: Audit logging occurs after permission checks, so denied attempts are still logged
3. **Privacy**: Audit logs contain patient addresses and record IDs - handle with care
4. **Storage**: Audit logs consume storage space - consider archival strategies for old entries
5. **Performance**: Querying large audit logs may be slow - use time ranges or limits when possible

## Best Practices

1. **Regular Monitoring**: Regularly review audit logs for suspicious activity
2. **Alert on Failures**: Set up alerts for repeated denied access attempts
3. **Compliance Reporting**: Generate regular compliance reports from audit logs
4. **Access Reviews**: Periodically review who has access to which records
5. **Time-based Queries**: Use time ranges to limit query scope and improve performance

## Compliance

Audit logging supports compliance with:

- **HIPAA**: Access logging requirements for PHI
- **GDPR**: Audit trails for data access
- **SOC 2**: Security monitoring and logging requirements
- **ISO 27001**: Information security audit requirements

## Limitations

1. **Storage Capacity**: Large numbers of audit entries consume storage space
2. **Query Performance**: Querying very large audit logs may be slow
3. **No Deletion**: Audit entries cannot be deleted (by design for compliance)
4. **Limited Context**: Audit entries contain limited context - detailed information may be stored off-chain

## Future Enhancements

Potential future enhancements:

1. **Audit Report Generation**: Automated report generation functions
2. **Anomaly Detection**: Built-in detection of unusual access patterns
3. **Compression**: Compression of old audit entries to save storage
4. **Export Functions**: Functions to export audit logs for external analysis
5. **Retention Policies**: Configurable retention policies for audit logs
