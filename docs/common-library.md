# Common Library Documentation

Comprehensive guide to the shared utilities in `contracts/common/` — a foundational crate providing reusable patterns for authorization, state management, error handling, and security across all Teye contracts.

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture & Dependencies](#architecture--dependencies)
3. [CommonError Reference](#commonerror-reference)
4. [Module Directory](#module-directory)
5. [Feature Flags](#feature-flags)
6. [Integration Patterns](#integration-patterns)

---

## Overview

The `contracts/common` crate standardizes:

- **Error handling** — Shared `CommonError` enum with reserved code ranges
- **Authorization** — Multi-tier admin roles, multisig, and progressive authentication
- **Lifecycle** — Pause/unpause, initialization guards, migration frameworks
- **Security** — Reentrancy protection, rate limiting, consent management
- **State management** — Versioning, conflict resolution, concurrency primitives
- **Performance** — Session caching, vector clocks for distributed ordering

### Philosophy

> **Reusable, composable, safe-by-default**

Each module is designed to be:

- **Zero-cost when unused** — Features/feature gates enable opt-in complexity
- **Soroban-native** — Leverages ledger storage, cryptography, and event patterns
- **Well-tested** — Dedicated test modules in each file
- **Documented with examples** — See each module section below

---

## Architecture & Dependencies

### Module Dependency Graph

```
lib.rs (CommonError, re-exports)
  ├─ admin_tiers ┐
  │              ├─ multisig, progressive_auth (authorization)
  │              └─ pausable
  │
  ├─ pausable (emergency stop)
  │
  ├─ session (request-scoped state)
  ├─ reentrancy_guard (call graph protection)
  ├─ rate_limit (request throttling)
  │
  ├─ whitelist (address filtering)
  │
  ├─ concurrency (Soroban-specific concurrent access)
  │  └─ vector_clock (causal ordering)
  │      └─ conflict_resolver (merge strategies)
  │          └─ operational_transform (OT rewriting)
  │
  ├─ migration (contract upgrade framework)
  │  └─ versioned_storage (lazy-migration wrapper)
  │
  ├─ consent (patient consent, requires `std` feature)
  │
  ├─ meta_tx (gasless txs, transaction relay)
  │
  ├─ metering (gas tracking per tenant)
  │
  ├─ policy_dsl (access policy language)
  │  └─ policy_engine (evaluation engine)
  │
  ├─ progressive_auth (auth challenges, multi-step)
  │
  ├─ risk_engine (transaction risk scoring)
  │
  ├─ keys (key derivation utilities)
  │
  ├─ session (user session lifecycle)
  │
  └─ (generally independent)

Feature Dependencies:
  consent ──requires─→ "std" feature
  (all others work in no_std)
```

### When to Use Each Module

| Module                | Use Case                                                            |
| --------------------- | ------------------------------------------------------------------- |
| **admin_tiers**       | Hierarchical admin roles (SuperAdmin, ContractAdmin, OperatorAdmin) |
| **multisig**          | Critical operations requiring M-of-N approval                       |
| **pausable**          | Emergency stop functionality                                        |
| **progressive_auth**  | Step-by-step authentication (MFA-like escalation)                   |
| **session**           | Request-scoped state (e.g., caller context, ledger height)          |
| **reentrancy_guard**  | Prevent re-entrant calls in cross-contract scenarios                |
| **rate_limit**        | Throttle requests per user/resource                                 |
| **whitelist**         | Simple address filtering                                            |
| **concurrency**       | Concurrent state updates with conflict resolution                   |
| **vector_clock**      | Event ordering in distributed systems                               |
| **conflict_resolver** | Merge conflicting state updates                                     |
| **migration**         | Contract upgrades with data versioning                              |
| **versioned_storage** | Automatic lazy migration during reads                               |
| **consent**           | Patient consent workflows (HIPAA compliance)                        |
| **meta_tx**           | Gasless transaction relaying                                        |
| **metering**          | Gas/cost tracking per tenant                                        |
| **policy_dsl**        | Composable access control policies                                  |
| **policy_engine**     | Evaluate policy DSL at runtime                                      |
| **risk_engine**       | Score transaction risk (anomaly detection)                          |
| **keys**              | Key derivation & management helpers                                 |

---

## CommonError Reference

### Error Code Ranges

```rust
pub enum CommonError {
    // Lifecycle (1–9)
    NotInitialized = 1,        // Contract not yet initialized
    AlreadyInitialized = 2,    // Contract already initialized

    // Authentication & Authorization (10–19)
    AccessDenied = 10,         // Caller lacks permission

    // Not Found (20–29)
    UserNotFound = 20,         // User not in storage
    RecordNotFound = 21,       // Record not in storage

    // Validation (30–39)
    InvalidInput = 30,         // Parameter validation failed

    // Contract State (40–49)
    Paused = 40,               // Contract is paused

    // Reserved (50–99)
    // Future common errors

    // Contract-Specific (100+)
    // Each contract defines errors in 100-199 range
}
```

### Extension Pattern

Contracts extend `CommonError` with domain-specific codes:

```rust
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MyContractError {
    // Inherit common errors implicitly (1-49 range)

    // Domain-specific (100+)
    InsufficientBalance = 100,
    InvalidRecordType = 101,
    ExpiryInPast = 102,
}
```

### Error Handling Pattern

```rust
use common::CommonError;

fn do_operation(env: Env, caller: Address) -> Result<(), CommonError> {
    // Check initialization first
    if !is_initialized(&env) {
        return Err(CommonError::NotInitialized);
    }

    // Then check authorization
    caller.require_auth();
    if !is_admin(&env, &caller) {
        return Err(CommonError::AccessDenied);
    }

    // Then validate inputs
    // ...

    // Then check pause state
    pausable::require_not_paused(&env)?;

    // Then perform operation
    // ...

    Ok(())
}
```

---

## Module Directory

### Admin Tiers

**Purpose:** Three-tier admin hierarchy (SuperAdmin > ContractAdmin > OperatorAdmin) with permission escalation.

**Public API:**

```rust
pub enum AdminTier {
    OperatorAdmin = 1,
    ContractAdmin = 2,
    SuperAdmin = 3
}

// Functions
pub fn set_tier(env: &Env, admin: Address, tier: AdminTier) -> Result<(), CommonError>
pub fn get_tier(env: &Env, admin: Address) -> Result<AdminTier, CommonError>
pub fn require_tier(env: &Env, caller: Address, min: AdminTier) -> Result<(), CommonError>
pub fn set_super_admin(env: &Env, admin: Address)
pub fn track_admin(env: &Env, admin: Address)
```

**Usage Pattern:**

```rust
// 1. Initialize admin on contract startup
admin_tiers::set_super_admin(&env, &initial_admin);

// 2. Require admin for sensitive operations
fn pause_contract(env: Env, caller: Address) -> Result<(), CommonError> {
    caller.require_auth();
    admin_tiers::require_tier(&env, caller, AdminTier::OperatorAdmin)?;
    pausable::set_paused(&env, true);
    Ok(())
}

// 3. Promote admin (SuperAdmin only)
fn promote_to_contract_admin(
    env: Env,
    current_admin: Address,
    new_admin: Address,
) -> Result<(), CommonError> {
    current_admin.require_auth();
    admin_tiers::require_tier(&env, current_admin, AdminTier::SuperAdmin)?;
    admin_tiers::set_tier(&env, new_admin, AdminTier::ContractAdmin)?;
    Ok(())
}
```

**Gotchas:**

- SuperAdmin tier cannot be demoted (by design)
- Tier checks are **not** enforced internally; contracts must call `require_tier()` explicitly
- Admin tier storage uses TTL expiration (30 days); active admins auto-renew

---

### Multisig

**Purpose:** M-of-N proposal tracking for critical operations requiring distributed approval.

**Public API:**

```rust
pub struct MultisigConfig {
    pub threshold: u32,
    pub signers: Vec<Address>,
}

pub struct Proposal {
    pub id: u64,
    pub action: Symbol,
    pub data_hash: BytesN<32>,
    pub proposer: Address,
    pub approvals: Vec<Address>,
    pub created_at: u64,
    pub executed: bool,
}

// Functions
pub fn new_proposal(
    env: &Env,
    action: Symbol,
    data_hash: BytesN<32>,
    proposer: Address,
) -> Result<u64, CommonError>

pub fn approve_proposal(
    env: &Env,
    proposal_id: u64,
    signer: Address,
) -> Result<bool, CommonError>  // Returns true if threshold met

pub fn get_proposal(env: &Env, id: u64) -> Result<Proposal, CommonError>

pub fn execute_proposal(env: &Env, proposal_id: u64) -> Result<(), CommonError>
```

**Usage Pattern:**

```rust
use soroban_sdk::symbol_short;

// 1. Create proposal for reward rate change
fn propose_rate_change(
    env: Env,
    proposer: Address,
    new_rate: i128,
) -> Result<u64, CommonError> {
    proposer.require_auth();

    let data_hash = env.crypto().sha256(&Bytes::from_slice(
        &env,
        &new_rate.to_be_bytes(),
    ));

    multisig::new_proposal(
        &env,
        symbol_short!("RATE"),
        data_hash.into(),
        proposer,
    )
}

// 2. Signers approve
fn approve_rate_change(
    env: Env,
    signer: Address,
    proposal_id: u64,
) -> Result<bool, CommonError> {
    signer.require_auth();

    let threshold_met = multisig::approve_proposal(&env, proposal_id, signer)?;

    if threshold_met {
        env.events().publish((symbol_short!("READY"), proposal_id), ());
    }

    Ok(threshold_met)
}

// 3. Execute once threshold met
fn execute_rate_change(
    env: Env,
    proposal_id: u64,
    new_rate: i128,
) -> Result<(), CommonError> {
    let proposal = multisig::get_proposal(&env, proposal_id)?;

    // Verify data matches
    let expected_hash = env.crypto().sha256(&Bytes::from_slice(
        &env,
        &new_rate.to_be_bytes(),
    ));
    if proposal.data_hash != expected_hash.into() {
        return Err(CommonError::InvalidInput);
    }

    multisig::execute_proposal(&env, proposal_id)?;

    // Perform the actual rate change
    env.storage().instance().set(&REWARD_RATE, &new_rate);

    Ok(())
}
```

**Gotchas:**

- Proposal IDs are **not** reused after execution
- `data_hash` is the contract's responsibility to verify; multisig doesn't validate intent
- Proposer counts as first vote (auto-approval)
- After execution, proposal state is immutable

---

### Pausable

**Purpose:** Emergency stop circuit breaker for contract state-mutating operations.

**Public API:**

```rust
pub fn set_paused(env: &Env, paused: bool)
pub fn is_paused(env: &Env) -> bool
pub fn require_not_paused(env: &Env) -> Result<(), CommonError>
pub fn pause(env: &Env, caller: &Address)
pub fn unpause(env: &Env, caller: &Address)
```

**Usage Pattern:**

```rust
// 1. Check pause state early in sensitive functions
fn add_record(
    env: Env,
    provider: Address,
    record_data: String,
) -> Result<u64, CommonError> {
    provider.require_auth();
    pausable::require_not_paused(&env)?;  // ← Guard at entry

    // ... perform storage operation ...

    Ok(record_id)
}

// 2. Emergency pause (admin only)
fn pause_contract(env: Env, admin: Address) -> Result<(), CommonError> {
    admin.require_auth();
    admin_tiers::require_tier(&env, admin, AdminTier::OperatorAdmin)?;
    pausable::pause(&env, &admin);
    env.events().publish((symbol_short!("ALERT"), "system_paused"), ());
    Ok(())
}
```

**Gotchas:**

- `require_not_paused` **must** be called explicitly; no automatic guard
- Read-only functions should NOT check pause state
- Pause state is stored in **instance** storage (contract-wide, not user-specific)

---

### Session

**Purpose:** Request-scoped state (e.g., caller context, current ledger height) without repeated re-reads.

**Public API:**

```rust
pub struct Session {
    pub caller: Address,
    pub height: u32,
    pub timestamp: u64,
}

pub fn current(env: &Env) -> Session
pub fn with_caller(env: &Env, caller: Address) -> Session
pub fn in_batch(env: &Env, batch: fn(&Session) -> Result<(), CommonError>) -> Result<(), CommonError>
```

**Usage Pattern:**

```rust
// 1. Cache caller context for a batch of operations
fn batch_grant_access(
    env: Env,
    patient: Address,
    grantees: Vec<Address>,
    level: AccessLevel,
) -> Result<(), CommonError> {
    patient.require_auth();

    let session = session::current(&env);
    assert_eq!(session.caller, patient); // Verified

    for grantee in grantees.iter() {
        grant_access_internal(&env, &patient, grantee, level, session.timestamp)?;
    }

    Ok(())
}

// 2. Stateful ledger height checks
fn check_cooldown(
    env: Env,
    created_at: u64,
    min_cooldown_blocks: u32,
) -> Result<(), CommonError> {
    let session = session::current(&env);

    if session.height - created_at < min_cooldown_blocks {
        return Err(CommonError::InvalidInput);
    }

    Ok(())
}
```

**Gotchas:**

- Session data is computed on each call; use `in_batch` to cache across multiple checks
- Caller must authenticate before session is valid

---

### Reentrancy Guard

**Purpose:** Prevent re-entrant calls during cross-contract invocations.

**Public API:**

```rust
pub struct ReentrancyGuard { /* private */ }

impl ReentrancyGuard {
    pub fn new(env: &Env) -> Self
    // Lock is automatically released when guard is dropped (RAII)
}
```

**Usage Pattern:**

```rust
// 1. Guard cross-contract calls
fn call_external_contract(
    env: Env,
    external: Address,
    data: String,
) -> Result<String, CommonError> {
    let _guard = common::ReentrancyGuard::new(&env);  // ← Lock acquired

    let result = external_contract_client.do_something(&env, &data);
    // If result or any operation re-enters current contract, second lock attempt panics

    Ok(result)
}

// 2. Guard state mutations before external calls
fn transfer_and_notify(
    env: Env,
    from: Address,
    to: Address,
    amount: i128,
    notifier: Address,
) -> Result<(), CommonError> {
    let _guard = common::ReentrancyGuard::new(&env);

    // ① Update our state first
    transfer_internal(&env, &from, &to, &amount)?;

    // ② Then make external call (if it re-enters, state is consistent)
    notifier.call_notification_webhook(&env, &from, &to, &amount)?;

    Ok(())
}
```

**Gotchas:**

- Guard uses storage counter; storage fees apply
- If re-entrance detected, panic occurs (unrecoverable)
- Should be used only for cross-contract calls, not intra-contract recursion

---

### Rate Limit

**Purpose:** Fixed-window rate limiting for throttling requests per user/resource.

**Public API:**

```rust
pub fn check_rate_limit(
    env: &Env,
    config: RateLimitConfig,
    caller: Address,
    now: u64,
) -> Result<bool, CommonError>  // true = allowed, false = limited

pub struct RateLimitConfig {
    pub max_requests_per_window: u64,
    pub window_duration_seconds: u64,
}
```

**Usage Pattern:**

```rust
const RATE_LIMIT_CONFIG: RateLimitConfig = RateLimitConfig {
    max_requests_per_window: 100,      // 100 requests
    window_duration_seconds: 60,        // per 60 seconds
};

fn submit_analysis_request(
    env: Env,
    caller: Address,
    request_data: String,
) -> Result<u64, CommonError> {
    caller.require_auth();

    let now = env.ledger().timestamp();

    // Check rate limit
    if !rate_limit::check_rate_limit(
        &env,
        RATE_LIMIT_CONFIG,
        caller.clone(),
        now,
    )? {
        return Err(ContractError::RateLimited);  // Custom error code
    }

    // Process request
    let request_id = submit_request_internal(&env, &caller, &request_data)?;

    Ok(request_id)
}
```

**Gotchas:**

- Window is **fixed**, not sliding (resets at epoch boundaries)
- Per-caller state stored separately for each resource
- Rate limit state persists; manually clear old entries to save storage

---

### Whitelist

**Purpose:** Simple address filtering for authorized participants.

**Public API:**

```rust
pub fn add_to_whitelist(env: &Env, address: Address) -> Result<(), CommonError>
pub fn remove_from_whitelist(env: &Env, address: Address) -> Result<(), CommonError>
pub fn is_whitelisted(env: &Env, address: Address) -> bool
pub fn require_whitelisted(env: &Env, address: &Address) -> Result<(), CommonError>
```

**Usage Pattern:**

```rust
// 1. Add provider to whitelist (admin only)
fn register_provider(
    env: Env,
    admin: Address,
    provider: Address,
) -> Result<(), CommonError> {
    admin.require_auth();
    admin_tiers::require_tier(&env, admin, AdminTier::ContractAdmin)?;

    whitelist::add_to_whitelist(&env, provider.clone())?;
    env.events().publish((symbol_short!("PROV_ADD"), provider), ());

    Ok(())
}

// 2. Require provider is whitelisted
fn submit_provider_result(
    env: Env,
    provider: Address,
    result_data: String,
) -> Result<(), CommonError> {
    provider.require_auth();
    whitelist::require_whitelisted(&env, &provider)?;

    // Process result
    process_result(&env, &provider, &result_data)?;

    Ok(())
}
```

**Gotchas:**

- Whitelist is **additive only** (doesn't enforce exclusivity)
- Storage fees apply per whitelisted entry
- No automatic expiration

---

### Concurrency & Vector Clock

**Purpose:** Causally-ordered concurrent access patterns with conflict resolution.

**Public API:**

```rust
pub struct VectorClock {
    pub replica_id: u32,
    pub clock: u64,
}

pub fn new_clock(env: &Env, replica_id: u32) -> VectorClock

pub struct ConflictEntry {
    pub value: T,
    pub clock: VectorClock,
}

pub fn resolve_conflict<T>(
    local: ConflictEntry<T>,
    remote: ConflictEntry<T>,
    strategy: ResolutionStrategy,
) -> ConflictEntry<T>

pub enum ResolutionStrategy {
    LastWriteWins,
    LargestValue,
    CustomTransform(fn(T, T) -> T),
}
```

**Usage Pattern:**

```rust
// 1. Track causality during concurrent writes
fn concurrent_update(
    env: Env,
    field: String,
    new_value: i128,
) -> Result<(), CommonError> {
    let clock = vector_clock::new_clock(&env, get_replica_id(&env));

    let current: ConflictEntry = env.storage().persistent().get(&field_key)?;

    if clock.happens_after(&current.clock) {
        // We can safely overwrite
        let entry = ConflictEntry {
            value: new_value,
            clock,
        };
        env.storage().persistent().set(&field_key, &entry);
    } else {
        // Conflict: use strategy
        let resolved = conflict_resolver::resolve_conflict(
            ConflictEntry { value: current.value, clock: current.clock },
            ConflictEntry { value: new_value, clock },
            ResolutionStrategy::LastWriteWins,
        );
        env.storage().persistent().set(&field_key, &resolved);
    }

    Ok(())
}
```

**Gotchas:**

- Vector clocks scale linearly with # replicas; for 100+ replicas, consider alternatives
- Causality is not guaranteed across ledger resets
- Conflict resolution is **irreversible**; audit trail recommended

---

### Migration & Versioned Storage

**Purpose:** Contract upgrade framework with lazy data migration and rollback support.

**Public API:**

```rust
pub struct MigrationContext {
    pub version: u32,
    pub data: Bytes,
}

pub fn migrate(
    env: &Env,
    old_version: u32,
    new_version: u32,
    context: MigrationContext,
) -> Result<Bytes, CommonError>

// Versioned storage wrapper
pub fn get_versioned<T>(
    env: &Env,
    key: Symbol,
    current_version: u32,
) -> Result<T, CommonError>  // Automatically migrates on read
```

**Usage Pattern:**

```rust
// 1. Define migration function
fn migrate_v1_to_v2(
    env: &Env,
    old_data: Bytes,
) -> Result<Bytes, CommonError> {
    // Parse old format
    let old_config: OldConfigV1 = deserialize(&old_data)?;

    // Transform to new format
    let new_config = NewConfigV2 {
        admin: old_config.admin,
        new_field: default_value(),  // New field with default
    };

    // Serialize new format
    Ok(serialize(&new_config))
}

// 2. During contract upgrade
#[contractimpl]
impl MyContract {
    pub fn initialize_v2(env: Env, admin: Address) -> Result<(), CommonError> {
        let old_version: u32 = env.storage().instance().get(&VERSION).unwrap_or(1);

        if old_version < 2 {
            migration::migrate(&env, old_version, 2, MigrationContext {
                version: old_version,
                data: get_all_config_bytes(&env),
            })?;
        }

        env.storage().instance().set(&VERSION, &2u32);
        Ok(())
    }
}

// 3. Lazy migration on read
fn get_config(env: Env) -> Result<Config, CommonError> {
    let config = versioned_storage::get_versioned::<Config>(
        &env,
        symbol_short!("CONFIG"),
        2,  // Current version
    )?;

    Ok(config)
}
```

**Gotchas:**

- Migration functions must be deterministic (same input → same output always)
- Storage overhead increases with # versions; clean up old migrations after successful deployment
- Rollback requires deploying old contract code again

---

### Consent

**Purpose:** Patient consent workflows with HIPAA compliance (requires `std` feature).

**Feature Requirement:**

```toml
# In Cargo.toml
common = { path = "../../common", features = ["std"] }
```

**Public API:**

```rust
pub struct ConsentRecord {
    pub patient: Address,
    pub organization: Address,
    pub consent_type: ConsentType,
    pub expires_at: u64,
    pub revoked: bool,
}

pub enum ConsentType {
    Treatment,
    Research,
    HealthCare,
    Custom(String),
}

pub fn grant_consent(
    env: &Env,
    patient: Address,
    organization: Address,
    consent_type: ConsentType,
    duration_seconds: u64,
) -> Result<(), CommonError>

pub fn check_consent(
    env: &Env,
    patient: Address,
    organization: Address,
    consent_type: ConsentType,
) -> bool

pub fn revoke_consent(
    env: &Env,
    patient: Address,
    organization: Address,
) -> Result<(), CommonError>
```

**Usage Pattern:**

```rust
// 1. Patient grants consent
fn consent_to_research(
    env: Env,
    patient: Address,
    research_org: Address,
) -> Result<(), CommonError> {
    patient.require_auth();

    consent::grant_consent(
        &env,
        patient.clone(),
        research_org.clone(),
        ConsentType::Research,
        365 * 24 * 60 * 60,  // 1 year
    )?;

    env.events().publish((symbol_short!("CONSENT"), patient), ("research", research_org));

    Ok(())
}

// 2. Verify consent before sharing data
fn share_records_for_research(
    env: Env,
    patient: Address,
    researcher: Address,
) -> Result<Vec<Record>, CommonError> {
    researcher.require_auth();

    let research_org = get_researcher_org(&env, &researcher)?;

    if !consent::check_consent(
        &env,
        patient.clone(),
        research_org.clone(),
        ConsentType::Research,
    ) {
        return Err(CommonError::AccessDenied);
    }

    let records = get_patient_records(&env, &patient)?;
    Ok(records)
}

// 3. Patient revokes consent
fn revoke_research_consent(
    env: Env,
    patient: Address,
    research_org: Address,
) -> Result<(), CommonError> {
    patient.require_auth();

    consent::revoke_consent(&env, &patient, &research_org)?;

    env.events().publish((symbol_short!("REVOKE"), patient), ("research", research_org));

    Ok(())
}
```

**Gotchas:**

- Requires `std` feature; not available in no_std
- Consent expiry is checked on grant verification (not pre-expired)
- Revocation is immediate; off-chain systems must honor revocations

---

### Policy DSL & Policy Engine

**Purpose:** Declarative access control policies with runtime evaluation.

**Public API:**

```rust
pub enum PolicyExpression {
    And(Box<PolicyExpression>, Box<PolicyExpression>),
    Or(Box<PolicyExpression>, Box<PolicyExpression>),
    Not(Box<PolicyExpression>),
    Predicate(String, String),  // (attribute, expected_value)
}

pub struct PolicyContext {
    pub user_role: String,
    pub resource_sensitivity: String,
    pub time_of_access: u64,
    // ... other context
}

pub fn evaluate(expr: &PolicyExpression, context: &PolicyContext) -> bool
```

**Usage Pattern:**

```rust
// 1. Define policy
let policy = PolicyExpression::And(
    Box::new(PolicyExpression::Predicate("role".to_string(), "clinician".to_string())),
    Box::new(PolicyExpression::Or(
        Box::new(PolicyExpression::Predicate("owns_patient".to_string(), "true".to_string())),
        Box::new(PolicyExpression::Predicate("is_auditor".to_string(), "true".to_string())),
    )),
);

// 2. Evaluate at access time
fn check_record_access(
    env: Env,
    user: Address,
    record_id: u64,
) -> Result<bool, CommonError> {
    let context = PolicyContext {
        user_role: get_user_role(&env, &user)?,
        resource_sensitivity: get_record_sensitivity(&env, record_id)?,
        time_of_access: env.ledger().timestamp(),
    };

    let allowed = policy_engine::evaluate(&policy, &context);

    Ok(allowed)
}
```

**Gotchas:**

- Policy evaluation is **not** persisted; re-evaluate on each access
- Attribute names/values are strings; case-sensitive
- Complex policies (deep nesting) may hit computational limits

---

### Meta-Transaction (Relay)

**Purpose:** Gasless transaction patterns where a relayer pays gas on behalf of user.

**Public API:**

```rust
pub struct MetaTx {
    pub from: Address,
    pub to: Address,
    pub data: Bytes,
    pub nonce: u64,
    pub deadline: u64,
}

pub fn verify_meta_tx(env: &Env, tx: MetaTx, sig: BytesN<64>) -> Result<bool, CommonError>

pub fn increment_nonce(env: &Env, user: &Address) -> Result<(), CommonError>
```

**Usage Pattern:**

```rust
// 1. User signs meta-tx (off-chain)
let meta_tx = MetaTx {
    from: user_address,
    to: contract_address,
    data: abi.encode("stake", [amount]),
    nonce: get_nonce(&env, &user),
    deadline: current_timestamp + 3600,
};

let sig = sign_meta_tx(&user_private_key, &meta_tx);

// 2. Relayer submits on-chain
fn relay_meta_tx(
    env: Env,
    tx: MetaTx,
    sig: BytesN<64>,
) -> Result<(), CommonError> {
    // Verify signature
    meta_tx::verify_meta_tx(&env, tx.clone(), sig)?;

    // Execute on behalf of user (contract enforces user == from)
    stake(&env, tx.from.clone(), amount)?;

    // Increment nonce to prevent replay
    meta_tx::increment_nonce(&env, &tx.from)?;

    Ok(())
}
```

**Gotchas:**

- Relayer pays gas; must incentivize relayers (e.g., fee-sharing)
- Deadline should be reasonable (1 hour is typical)
- Signature verification is caller's responsibility

---

### Metering

**Purpose:** Gas/resource tracking per tenant in hierarchical multi-tenant systems.

**Public API:**

```rust
pub struct TenantMetrics {
    pub cpu_used: u64,
    pub storage_used: u64,
    pub bandwidth_used: u64,
}

pub fn record_usage(
    env: &Env,
    tenant_id: Address,
    usage: TenantMetrics,
) -> Result<(), CommonError>

pub fn get_usage(env: &Env, tenant_id: Address) -> Result<TenantMetrics, CommonError>
```

**Usage Pattern:**

```rust
fn add_patient_record(
    env: Env,
    provider: Address,
    patient: Address,
    data: String,
) -> Result<u64, CommonError> {
    provider.require_auth();

    let record_id = store_record(&env, &patient, &data)?;

    // Track usage
    metering::record_usage(
        &env,
        provider.clone(),
        TenantMetrics {
            cpu_used: 100,
            storage_used: data.len() as u64,
            bandwidth_used: data.len() as u64,
        },
    )?;

    Ok(record_id)
}
```

**Gotchas:**

- Metering has small per-operation overhead
- Usage is not automatically reset (contracts must define billing periods)

---

### Progressive Authentication

**Purpose:** Step-by-step authentication escalation (MFA-like challenges).

**Public API:**

```rust
pub enum AuthLevel {
    Anonymous = 0,
    Basic = 1,           // Password-like
    Interactive = 2,     // MFA-like
    Strong = 3,          // ZK proof required
}

pub fn require_auth_level(
    env: &Env,
    user: Address,
    level: AuthLevel,
) -> Result<(), CommonError>

pub fn set_user_auth_level(
    env: &Env,
    user: Address,
    level: AuthLevel,
) -> Result<(), CommonError>
```

**Usage Pattern:**

```rust
// 1. Require higher auth level for sensitive operations
fn delete_all_records(
    env: Env,
    patient: Address,
) -> Result<(), CommonError> {
    patient.require_auth();

    // Require strong auth for destructive ops
    progressive_auth::require_auth_level(&env, patient.clone(), AuthLevel::Strong)?;

    delete_all_patient_records(&env, &patient)?;

    Ok(())
}

// 2. Escalate auth after challenge
fn escalate_to_strong_auth(
    env: Env,
    user: Address,
    proof: BytesN<32>,
) -> Result<(), CommonError> {
    user.require_auth();

    // Verify ZK proof or MFA challenge
    verify_mfa_challenge(&env, &user, &proof)?;

    progressive_auth::set_user_auth_level(&env, user, AuthLevel::Strong)?;

    Ok(())
}
```

**Gotchas:**

- Auth level is cached per-session; expires on logout
- Challenge verification is contract-specific

---

### Risk Engine

**Purpose:** Transaction risk scoring for anomaly detection and fraud prevention.

**Public API:**

```rust
pub struct RiskProfile {
    pub base_score: u8,
    pub anomaly_flags: Vec<Symbol>,
}

pub fn calculate_risk(
    env: &Env,
    user: Address,
    action: &str,
    amount: i128,
) -> Result<RiskProfile, CommonError>
```

**Usage Pattern:**

```rust
// 1. Score risk before approving large transaction
fn approve_large_transfer(
    env: Env,
    from: Address,
    to: Address,
    amount: i128,
) -> Result<(), CommonError> {
    from.require_auth();

    let risk = risk_engine::calculate_risk(&env, from.clone(), "transfer", amount)?;

    if risk.base_score > 80 {
        // Challenge user or require additional approval
        return Err(ContractError::RiskTooHigh);
    }

    transfer_internal(&env, &from, &to, amount)?;

    Ok(())
}
```

**Gotchas:**

- Risk scoring is statistical; false positives/negatives possible
- No automatic blocking (contracts decide action)

---

## Feature Flags

### `std` Feature

```toml
[features]
default = []
std = ["dep:serde"]
```

**What's gated:**

- `consent` module — Requires Rust standard library for serialization
- All other modules work in `no_std` mode (Soroban contracts)

**Usage:**

```rust
#[cfg(feature = "std")]
pub fn patient_dashboard(patient: Address) -> ConsentSummary {
    // Can use standard library here
    let mut summary = Map::new();
    // ...
}
```

---

## Integration Patterns

### Pattern 1: Authorization Layering

```rust
// Check in order: lifecycle → admin tier → pause state → rate limit → action
fn sensitive_operation(env: Env, admin: Address) -> Result<(), CommonError> {
    // 1. Ensure initialized
    if !is_initialized(&env) {
        return Err(CommonError::NotInitialized);
    }

    // 2. Require admin tier (calls address auth internally)
    admin_tiers::require_tier(&env, admin, AdminTier::ContractAdmin)?;

    // 3. Check pause state
    pausable::require_not_paused(&env)?;

    // 4. Check rate limit
    let now = env.ledger().timestamp();
    if !rate_limit::check_rate_limit(&env, cfg, admin, now)? {
        return Err(CustomError::RateLimited);
    }

    // 5. Perform operation
    do_operation(&env)?;

    Ok(())
}
```

### Pattern 2: Concurrent Updates with Conflict Resolution

```rust
fn update_record_safely(
    env: Env,
    record_id: u64,
    new_data: Data,
) -> Result<(), CommonError> {
    let _guard = reentrancy_guard::new(&env);

    let clock = vector_clock::new_clock(&env, get_replica_id(&env));
    let current: ConflictEntry = load_record(&env, record_id)?;

    if clock.happens_after(&current.clock) {
        // Safe to overwrite
        store_record(&env, record_id, new_data, clock)?;
    } else {
        // Potential conflict; resolve
        let resolved = conflict_resolver::resolve_conflict(
            current,
            ConflictEntry { value: new_data, clock },
            ResolutionStrategy::LastWriteWins,
        );
        store_record(&env, record_id, resolved.value, resolved.clock)?;
    }

    Ok(())
}
```

### Pattern 3: Multisig-Guarded Governance

```rust
fn propose_and_execute_rate_change(
    env: Env,
    proposer: Address,
    signers: Vec<Address>,
    new_rate: i128,
) -> Result<(), CommonError> {
    proposer.require_auth();

    // 1. Create proposal
    let data_hash = hash_rate(&new_rate);
    let proposal_id = multisig::new_proposal(
        &env,
        symbol_short!("RATE"),
        data_hash,
        proposer,
    )?;

    // 2. Gather approvals (off-chain: signers call approve_proposal)
    for signer in signers.iter() {
        if should_approve(signer) {
            multisig::approve_proposal(&env, proposal_id, signer.clone())?;
        }
    }

    // 3. Execute once quorum reached
    multisig::execute_proposal(&env, proposal_id)?;
    env.storage().instance().set(&REWARD_RATE, &new_rate);

    Ok(())
}
```

---

## Best Practices

1. **Always check initialization first** — Before any other guard
2. **Check authorization before rate limiting** — Authorized users shouldn't hit limits
3. **Use reentrancy guards around external calls** — Prevents re-entrance attacks
4. **Document custom error codes** — Reference CommonError ranges in module docs
5. **Test with worst-case concurrency** — Use vector clocks + conflict resolver
6. **Archive old migrations** — Don't keep all historical migrations indefinitely
7. **Expire rate limit entries** — Periodically clean up old windows
8. **Log consent changes** — For HIPAA audit trails

---

## Related Documentation

- [Governance](../governance.md) — Uses admin_tiers, multisig
- [Security](../security.md) — Uses reentrancy_guard, rate_limit, pausable
- [Storage Conventions](../storage-key-conventions.md) — TTL, key naming
- [Architecture](../architecture.md) — Module dependencies overview
