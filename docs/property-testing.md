# Property-Based Testing

Property-based testing (PBT) automatically generates hundreds of random inputs to verify that contract invariants always hold — going far beyond hand-crafted examples. The Stellar Teye contracts use [`proptest`](https://docs.rs/proptest) for this.

For comprehensive testing strategy including property testing patterns, see the [Testing Strategy Guide](testing-strategy.md#testing-taxonomy).

## Why Property-Based Tests for Smart Contracts?

Smart contracts handle real assets and sensitive healthcare data. A bug that triggers only on specific input combinations can be catastrophic. Property-based tests uncover these edge cases by:

- Running each property with **256 randomly generated inputs** (configurable)
- **Shrinking** any failing input to its minimal reproducing form
- Covering combinations that manual test authors would never think to write

## Test Locations

All property tests live in the contract crate at:

```
contracts/vision_records/tests/
├── property.rs          # Entry point (loads all modules)
└── property/
    ├── core.rs          # Core record management invariants
    ├── access.rs        # Access control invariants
    ├── rbac.rs          # Role-Based Access Control invariants
    ├── emergency.rs     # Emergency access invariants
    └── state_machine.rs # Contract lifecycle / state machine invariants
```

## Running Property Tests

```bash
# Run all property tests
cargo test --test property

# Run with more cases for higher confidence
PROPTEST_CASES=512 cargo test --test property

# Run a specific module
cargo test --test property access

# Run a specific test by name
cargo test --test property prop_grant_then_revoke_returns_none
```

## Documented Invariants

### Core Contract (`core.rs`)

| Property | Invariant |
|----------|-----------|
| `prop_record_id_monotonic` | Record IDs are always `1, 2, …, N` — never skipped or repeated |
| `prop_get_record_matches_store` | `get_record(id)` always returns exactly what was stored by `add_record` |
| `prop_patient_records_always_includes_new` | After `add_record`, `get_patient_records` always contains the new ID |
| `prop_record_count_increments` | `get_record_count()` always equals the number of successful `add_record` calls |
| `prop_patient_records_isolated` | Records for patient A never appear in patient B's list and vice versa |

### Access Control (`access.rs`)

| Property | Invariant |
|----------|-----------|
| `prop_no_access_before_grant` | `check_access` always returns `None` before any grant is made |
| `prop_grant_then_check_matches_level` | After `grant_access(level)`, `check_access` always returns that exact level |
| `prop_grant_then_revoke_returns_none` | Grant → revoke always results in `AccessLevel::None` |
| `prop_idempotent_revoke_never_panics` | Revoking a non-existent grant is always a safe no-op |
| `prop_regrant_overwrites_previous` | A second grant always overwrites the first level |
| `prop_grants_are_isolated` | Revoking one grantee never affects another grantee's access |

### RBAC (`rbac.rs`)

| Property | Invariant |
|----------|-----------|
| `prop_admin_always_has_all_perms` | Admin role always holds every permission |
| `prop_patient_has_no_system_perms` | Patient role never holds `WriteRecord`, `ManageUsers`, or `SystemAdmin` |
| `prop_staff_permissions_correct` | Staff has `ManageUsers` but not `WriteRecord` or `SystemAdmin` |
| `prop_optometrist_permissions_correct` | Optometrist has write/read/access perms but not `SystemAdmin` |
| `prop_custom_grant_adds_permission` | Granting a custom permission always makes `check_permission` return `true` |
| `prop_custom_revoke_overrides_base` | A custom revoke always overrides the base role's permission |
| `prop_expired_delegation_denied` | Any expired delegation is always denied when used |
| `prop_active_delegation_allowed` | A valid (non-expired) delegation always allows the delegatee to act |

### Emergency Access (`emergency.rs`)

| Property | Invariant |
|----------|-----------|
| `prop_empty_attestation_always_fails` | Empty attestation always returns `Err(InvalidInput)` |
| `prop_revoked_grant_always_invalid` | After revocation, `is_emergency_access_valid` always returns `false` |
| `prop_admin_can_always_revoke` | Admin can always revoke any active emergency grant |
| `prop_unauthorized_revoke_always_fails` | Random (non-patient, non-admin) address always receives `Err(Unauthorized)` |
| `prop_active_grant_is_valid` | A freshly created grant is always valid for all four conditions |
| `prop_contacts_stored` | Contacts passed to a grant are always stored on the grant struct |

### State Machine (`state_machine.rs`)

| Property | Invariant |
|----------|-----------|
| `prop_double_initialize_always_fails` | A second `initialize` always returns `Err(AlreadyInitialized)` |
| `prop_is_initialized_after_init` | `is_initialized()` always returns `true` after successful init |
| `prop_get_admin_matches_initializer` | `get_admin()` always returns the exact address passed to `initialize` |
| `prop_full_lifecycle_consistent` | The full init → register → record → grant → revoke sequence is always consistent |
| `prop_reregister_overwrites_role` | Re-registering a user overwrites their role (no panic) |
| `prop_version_always_returns_one` | `version()` always returns `1` |

## Configuration

The number of test cases per property can be configured with:

```bash
PROPTEST_CASES=<N> cargo test --test property
```

The default in CI is **256 cases**. For a more exhaustive local run, use **512** or higher.

Proptest also automatically saves any failing cases to `.proptest-regressions/` for reproducibility.
