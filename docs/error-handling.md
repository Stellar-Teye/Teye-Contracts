# Error Handling and Recovery

## Overview

The Vision Records contract implements comprehensive error handling with detailed error types, logging, events, and recovery mechanisms to ensure robust operation and observability.

## Error Types

### Error Categories

Errors are categorized into seven types:

- **Validation** (1): Invalid input parameters, format errors
- **Authorization** (2): Permission and access control failures
- **NotFound** (3): Resource lookup failures
- **StateConflict** (4): Duplicate registrations, expired delegations
- **Storage** (5): Storage operation failures
- **Transient** (6): Temporary failures that may succeed on retry
- **System** (7): Contract-level issues like pausing

### Error Severity Levels

- **Low** (1): Non-critical errors, informational
- **Medium** (2): Important but recoverable errors
- **High** (3): Significant errors requiring attention
- **Critical** (4): System-level failures requiring immediate action

## Error Codes

| Code | Error | Category | Severity | Retryable |
|------|-------|----------|----------|-----------|
| 1 | NotInitialized | Validation | Low | No |
| 2 | AlreadyInitialized | Validation | Low | No |
| 3 | Unauthorized | Authorization | Medium | No |
| 4 | UserNotFound | NotFound | Low | No |
| 5 | RecordNotFound | NotFound | Low | No |
| 6 | InvalidInput | Validation | Low | No |
| 7 | AccessDenied | Authorization | Medium | No |
| 8 | Paused | System | Critical | No |
| 9 | ProviderNotFound | NotFound | Low | No |
| 10 | ProviderAlreadyRegistered | StateConflict | Medium | No |
| 11 | InvalidVerificationStatus | Validation | Low | No |
| 12 | InvalidAddress | Validation | Low | No |
| 13 | InvalidTimestamp | Validation | Low | No |
| 14 | StorageError | Storage | High | Yes |
| 15 | RateLimitExceeded | Transient | Medium | Yes |
| 16 | ExpiredAccess | Authorization | Medium | No |
| 17 | InvalidRole | Validation | Low | No |
| 18 | InvalidPermission | Validation | Low | No |
| 19 | DelegationExpired | StateConflict | Medium | No |
| 20 | InvalidDataHash | Validation | Low | No |
| 21 | DuplicateRecord | StateConflict | Low | No |
| 22 | InvalidRecordType | Validation | Low | No |
| 23 | ContractPaused | System | Critical | No |
| 24 | InsufficientPermissions | Authorization | Medium | No |
| 25 | TransientFailure | Transient | High | Yes |

## Error Logging

### Error Log Structure

Each error is logged with:

- **Error Code**: Numeric identifier
- **Category**: Error classification
- **Severity**: Impact level
- **Message**: Human-readable description
- **User**: Address of user who triggered the error (if applicable)
- **Resource ID**: Identifier of the resource involved (if applicable)
- **Timestamp**: When the error occurred
- **Retryable**: Whether the operation can be retried

### Error Log Management

The contract maintains an error log with a maximum size of 100 entries. When the log is full, the oldest entries are removed.

**Functions:**
- `get_error_log()`: Retrieve all error log entries
- `get_error_count()`: Get total number of errors logged
- `clear_error_log(caller)`: Clear error log (admin only)

## Error Events

All errors trigger events for off-chain monitoring and indexing:

**Event Structure:**
```rust
ErrorEvent {
    error_code: u32,
    category: ErrorCategory,
    severity: ErrorSeverity,
    message: String,
    user: Option<Address>,
    resource_id: Option<String>,
    retryable: bool,
    timestamp: u64,
}
```

**Event Topics:**
- Topic 0: `ERROR`
- Topic 1: Error category
- Topic 2: Error severity

## Recovery Mechanisms

### Retry Mechanism

For transient failures, the contract provides a retry mechanism:

**Functions:**
- `retry_operation(caller, operation, max_retries)`: Check if operation can be retried
- `reset_retry_count(caller, operation)`: Reset retry counter for an operation

**Retry Rules:**
- Maximum retries: 3 per operation
- Retry count is tracked per user and operation
- Retry count resets on successful operation or manual reset

### Graceful Error Handling

The contract implements graceful error handling:

1. **Error Logging**: All errors are logged with full context
2. **Event Publishing**: Errors trigger events for monitoring
3. **State Preservation**: Errors do not corrupt contract state
4. **Clear Messages**: Detailed error messages help diagnose issues

## Best Practices

### For Integrators

1. **Check Error Codes**: Always check return values and handle errors appropriately
2. **Retry Transient Errors**: Retry operations marked as `retryable: true`
3. **Monitor Error Events**: Subscribe to error events for monitoring
4. **Respect Rate Limits**: Handle `RateLimitExceeded` with exponential backoff
5. **Validate Inputs**: Validate inputs before calling contract functions

### For Developers

1. **Use Error Context**: Always provide context when logging errors
2. **Categorize Correctly**: Use appropriate error categories
3. **Set Severity Appropriately**: Match severity to error impact
4. **Mark Retryable Errors**: Mark transient errors as retryable
5. **Test Error Paths**: Include error scenarios in tests

## Error Recovery Strategies

### Transient Failures

For `TransientFailure`, `StorageError`, or `RateLimitExceeded`:

1. Wait before retrying (exponential backoff recommended)
2. Check `retry_operation()` before attempting retry
3. Reset retry count after successful operation

### Authorization Errors

For `Unauthorized`, `AccessDenied`, or `InsufficientPermissions`:

1. Verify user has required permissions
2. Check if permissions have expired
3. Request appropriate permissions from admin

### State Conflicts

For `ProviderAlreadyRegistered`, `DuplicateRecord`, or `DelegationExpired`:

1. Check current state before operation
2. Update expired delegations
3. Handle duplicates appropriately

## Monitoring

### Error Metrics

Monitor these metrics:

- Error count over time
- Error rate by category
- Error rate by severity
- Retry success rate
- Most common error codes

### Alerting

Set up alerts for:

- Critical severity errors
- High error rates
- Repeated transient failures
- Storage errors

## Examples

### Handling a Transient Error

```rust
match client.add_record(...) {
    Ok(record_id) => {
        client.reset_retry_count(&caller, &String::from_str(&env, "add_record"));
        Ok(record_id)
    }
    Err(ContractError::TransientFailure) => {
        if client.retry_operation(&caller, &String::from_str(&env, "add_record"), 3)? {
            // Retry the operation
            client.add_record(...)
        } else {
            Err(ContractError::TransientFailure)
        }
    }
    Err(e) => Err(e),
}
```

### Checking Error Log

```rust
let error_log = client.get_error_log();
let error_count = client.get_error_count();

for entry in error_log.iter() {
    if entry.context.severity == ErrorSeverity::Critical {
        // Handle critical error
    }
}
```

## Testing

All error scenarios are tested to ensure:

- Errors are logged correctly
- Error events are published
- Recovery mechanisms work
- Retry logic functions properly
- Error context is preserved

See `contracts/vision_records/tests/errors.rs` for comprehensive error scenario tests.
