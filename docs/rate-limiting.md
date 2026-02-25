# Rate Limiting and Anti-Spam Measures

## Overview

The Vision Records contract implements comprehensive rate limiting to prevent abuse and spam attacks. The system provides per-address rate limits with configurable windows, automatic bypass for verified providers, and detailed monitoring capabilities.

## Features

- **Per-Address Rate Limits**: Each address has independent rate limits per operation type
- **Configurable Windows**: Admin-configurable time windows and request limits
- **Verified Provider Bypass**: Automatically grants rate limit bypass to verified healthcare providers
- **Rate Limit Events**: Comprehensive event emission for monitoring and alerting
- **Dashboard Data**: Query functions for rate limit status and configuration

## Rate Limit Configuration

### Setting Rate Limits

Only system administrators can configure rate limits. Each operation type can have its own rate limit configuration:

```rust
set_rate_limit_config(
    caller: Address,
    operation: String,      // e.g., "add_record", "get_record"
    max_requests: u32,       // Maximum requests allowed
    window_seconds: u64,     // Time window in seconds
) -> Result<(), ContractError>
```

**Example:**
- `add_record`: 10 requests per hour (3600 seconds)
- `get_record`: 100 requests per hour
- `grant_access`: 5 requests per hour

### Default Behavior

- If no rate limit is configured for an operation, the operation is not rate-limited
- Rate limits are checked after authentication but before the main operation logic
- Rate limit violations return `ContractError::RateLimitExceeded`

## Rate Limit Windows

Rate limits use a sliding window approach:

1. **Window Start**: When the first request is made, a time window begins
2. **Request Counting**: Each request increments a counter for that window
3. **Window Expiration**: When the window expires, the counter resets automatically
4. **Window Reset**: New requests after expiration start a new window

### Example

If a rate limit is set to 5 requests per 3600 seconds (1 hour):
- Requests 1-5: Allowed
- Request 6: Denied (rate limit exceeded)
- After 1 hour: Window resets, requests 1-5 are allowed again

## Verified Provider Bypass

Healthcare providers with `VerificationStatus::Verified` automatically receive rate limit bypass:

1. **Automatic Grant**: When a provider is verified, bypass is automatically enabled
2. **Automatic Revocation**: If verification status changes from Verified to another status, bypass is removed
3. **Manual Override**: Admins can manually grant or revoke bypass for any address

### Bypass Functions

```rust
// Check if an address has bypass
has_rate_limit_bypass(address: Address) -> bool

// Manually set bypass (admin only)
set_rate_limit_bypass(
    caller: Address,
    address: Address,
    bypass: bool,
) -> Result<(), ContractError>
```

## Rate Limit Status

Query the current rate limit status for any address and operation:

```rust
get_rate_limit_status(
    address: Address,
    operation: String,
) -> Option<RateLimitStatus>
```

**RateLimitStatus** includes:
- `current_count`: Current number of requests in the window
- `max_requests`: Maximum allowed requests
- `window_seconds`: Window duration
- `window_start`: When the current window started
- `window_end`: When the current window expires
- `reset_at`: When the rate limit will reset

## Events

### Rate Limit Exceeded Event

Emitted when a rate limit is exceeded:

```rust
RateLimitExceededEvent {
    address: Address,
    operation: String,
    current_count: u32,
    max_requests: u32,
    reset_at: u64,
    timestamp: u64,
}
```

**Event Topics**: `("RATE_LIMIT", address, operation)`

### Rate Limit Config Updated Event

Emitted when rate limit configuration is updated:

```rust
RateLimitConfigUpdatedEvent {
    operation: String,
    max_requests: u32,
    window_seconds: u64,
    updated_by: Address,
    timestamp: u64,
}
```

**Event Topics**: `("RL_CONFIG", operation)`

### Rate Limit Bypass Updated Event

Emitted when bypass is granted or revoked:

```rust
RateLimitBypassUpdatedEvent {
    address: Address,
    bypass_enabled: bool,
    updated_by: Address,
    timestamp: u64,
}
```

**Event Topics**: `("RL_BYPASS", address)`

## API Reference

### Admin Functions

#### `set_rate_limit_config`
Configure rate limits for an operation.

**Parameters:**
- `caller: Address` - Must be system admin
- `operation: String` - Operation name (e.g., "add_record")
- `max_requests: u32` - Maximum requests allowed
- `window_seconds: u64` - Time window in seconds

**Returns:** `Result<(), ContractError>`

**Errors:**
- `Unauthorized` - Caller is not a system admin

#### `set_rate_limit_bypass`
Manually grant or revoke rate limit bypass.

**Parameters:**
- `caller: Address` - Must be system admin
- `address: Address` - Address to modify
- `bypass: bool` - Enable or disable bypass

**Returns:** `Result<(), ContractError>`

**Errors:**
- `Unauthorized` - Caller is not a system admin

### Query Functions

#### `get_rate_limit_config`
Get rate limit configuration for an operation.

**Parameters:**
- `operation: String` - Operation name

**Returns:** `Option<RateLimitConfig>`

#### `get_rate_limit_status`
Get current rate limit status for an address and operation.

**Parameters:**
- `address: Address` - Address to check
- `operation: String` - Operation name

**Returns:** `Option<RateLimitStatus>`

#### `has_rate_limit_bypass`
Check if an address has rate limit bypass.

**Parameters:**
- `address: Address` - Address to check

**Returns:** `bool`

#### `get_all_rate_limit_configs`
Get all configured rate limit configurations.

**Returns:** `Vec<RateLimitConfig>`

## Integration Points

Rate limiting is integrated into the following contract functions:

1. **`add_record`**: Prevents spam record creation
2. **`get_record`**: Prevents excessive read operations
3. **`register_user`**: Prevents user registration spam
4. **`grant_access`**: Prevents access grant abuse

Rate limit checks occur:
- After authentication (`require_auth()`)
- Before the main operation logic
- Before permission checks

## Best Practices

### Configuration

1. **Set Appropriate Limits**: Balance security with usability
   - Write operations (add_record): Lower limits (5-10 per hour)
   - Read operations (get_record): Higher limits (50-100 per hour)
   - Administrative operations: Very low limits (1-5 per hour)

2. **Window Sizes**: Choose windows that match usage patterns
   - Short windows (60-300 seconds): For burst protection
   - Medium windows (3600 seconds): For hourly limits
   - Long windows (86400 seconds): For daily limits

3. **Monitor Events**: Track `RateLimitExceeded` events to identify:
   - Legitimate users hitting limits (may need adjustment)
   - Potential abuse attempts
   - System load patterns

### Provider Verification

1. **Verify Providers Promptly**: Verified providers get bypass automatically
2. **Monitor Bypass Usage**: Track which providers have bypass enabled
3. **Review Verification Status**: Regularly audit provider verification status

### Error Handling

When rate limits are exceeded:
1. Error is logged with context
2. `RateLimitExceeded` event is emitted
3. `ContractError::RateLimitExceeded` is returned
4. Client should wait until `reset_at` before retrying

## Security Considerations

### Attack Vectors

1. **Spam Attacks**: Rate limiting prevents rapid-fire requests
2. **Resource Exhaustion**: Limits prevent excessive storage operations
3. **DDoS Mitigation**: Per-address limits reduce impact of distributed attacks

### Limitations

1. **Per-Address Limits**: Attackers can use multiple addresses
   - Mitigation: Combine with IP-based filtering (off-chain)
   - Mitigation: Monitor for suspicious patterns

2. **Bypass Abuse**: Verified providers have unlimited access
   - Mitigation: Regular audit of verified providers
   - Mitigation: Monitor provider activity patterns

3. **Window Reset**: Limits reset after window expiration
   - Mitigation: Use shorter windows for critical operations
   - Mitigation: Implement progressive rate limiting (off-chain)

## Compliance Notes

### HIPAA Considerations

- Rate limiting does not affect patient data access requirements
- Verified providers receive bypass to ensure timely care
- Rate limit events are logged for audit purposes

### Audit Requirements

- All rate limit events are emitted and can be tracked off-chain
- Rate limit status queries support compliance reporting
- Bypass grants/revocations are logged with admin attribution

## Example Scenarios

### Scenario 1: Normal User Operation

1. User makes 5 `add_record` requests within an hour
2. All requests succeed (within limit of 10/hour)
3. User makes 6 more requests
4. Requests 6-10 succeed, request 11 fails with `RateLimitExceeded`
5. User waits 1 hour, window resets
6. User can make requests again

### Scenario 2: Verified Provider

1. Provider is registered with `Pending` status
2. Provider makes 10 `add_record` requests (within limit)
3. Provider makes 11th request, fails with `RateLimitExceeded`
4. Admin verifies provider (`VerificationStatus::Verified`)
5. Bypass is automatically granted
6. Provider can make unlimited requests

### Scenario 3: Rate Limit Configuration

1. Admin sets `add_record` limit to 5 requests per 3600 seconds
2. Multiple users hit the limit
3. Admin reviews `RateLimitExceeded` events
4. Admin adjusts limit to 10 requests per 3600 seconds
5. `RateLimitConfigUpdated` event is emitted
6. New limit applies to all future requests

## Monitoring and Alerts

### Key Metrics

1. **Rate Limit Exceeded Count**: Number of `RateLimitExceeded` events
2. **Top Rate-Limited Addresses**: Addresses hitting limits most frequently
3. **Bypass Usage**: Number of addresses with bypass enabled
4. **Configuration Changes**: Track `RateLimitConfigUpdated` events

### Alert Thresholds

- High rate limit exceeded rate (>10% of requests)
- Sudden spike in rate limit events
- Unusual bypass grant patterns
- Configuration changes outside business hours

## Troubleshooting

### Common Issues

1. **Legitimate Users Hitting Limits**
   - Solution: Increase limits for specific operations
   - Solution: Verify providers who need higher limits

2. **Rate Limits Too Restrictive**
   - Solution: Review usage patterns and adjust limits
   - Solution: Consider different limits for different user roles

3. **Bypass Not Working**
   - Check: Provider verification status
   - Check: Bypass was actually granted (query `has_rate_limit_bypass`)
   - Check: Events for bypass grant/revoke

4. **Rate Limit Status Incorrect**
   - Check: Current ledger timestamp
   - Check: Window expiration time
   - Check: Request count vs. max requests
