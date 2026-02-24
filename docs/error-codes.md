# Error Codes

Standardised error codes used across all Teye contracts.
Every contract re-exports `CommonError` from the `common` crate as `ContractError` so that error codes are identical everywhere.

## Code Ranges

| Range   | Category                       | Description                                   |
|---------|--------------------------------|-----------------------------------------------|
| 1 – 9   | Lifecycle / Initialisation     | Contract setup and one-time operations         |
| 10 – 19 | Authentication & Authorisation | Caller identity and permission checks          |
| 20 – 29 | Resource Not Found             | Requested entity does not exist in storage     |
| 30 – 39 | Validation / Input             | Invalid parameters or malformed data           |
| 40 – 49 | Contract State                 | Runtime state constraints (paused, locked, …)  |
| 100+    | Contract-Specific              | Reserved for individual contract extensions    |

## Common Errors

| Code | Variant              | When it is returned                                                                                   |
|------|----------------------|-------------------------------------------------------------------------------------------------------|
| 1    | `NotInitialized`     | A function that requires prior initialisation is called before `initialize()`.                        |
| 2    | `AlreadyInitialized` | `initialize()` is called more than once on the same contract instance.                                |
| 10   | `AccessDenied`       | The caller lacks the required role or permission (e.g. not admin, not record owner, expired grant).   |
| 20   | `UserNotFound`       | The requested user address does not exist in contract storage.                                        |
| 21   | `RecordNotFound`     | The requested record ID does not exist in contract storage.                                           |
| 30   | `InvalidInput`       | One or more parameters are invalid (empty list, zero duration, malformed hash, etc.).                 |
| 40   | `Paused`             | The contract is temporarily paused and cannot process the request.                                    |

## Migration Notes

### `Unauthorized` → `AccessDenied` (code 10)

Previous versions of `vision_records` defined two overlapping error variants:
- `Unauthorized` (code 3) — caller is not allowed
- `AccessDenied` (code 7) — caller is not allowed

These have been consolidated into a single `AccessDenied` (code **10**).
Any off-chain code that matched on error code `3` or `7` should now match on `10`.

## Extending Errors

Contracts that need domain-specific error codes should define them in their own `#[contracterror]` enum starting at code **100** to avoid collisions with the common set.

```rust
#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum VisionError {
    /// The emergency access window has expired.
    EmergencyExpired = 100,
    /// Maximum record limit per patient exceeded.
    RecordLimitExceeded = 101,
}
```
