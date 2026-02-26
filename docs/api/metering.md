# Metering Contract API Reference

## Contract Purpose

Tracks per-tenant resource consumption and enforces quotas across a hierarchy of organizations, clinics, providers, and patients. Enables fair cost allocation and prevents resource abuse in multi-tenant healthcare systems.

## Initialization

### `initialize(env: Env, admin: Address) -> Result<(), MeteringError>`

Initialize the metering contract with an administrator.

**Parameters:**

- `admin` - Administrator address (must authenticate)

**Returns:** `Result<(), MeteringError>`

**Errors:**

- `AlreadyInitialized` - Contract already initialized

**Example:**

```rust
client.initialize(&env, &admin_address)?;
```

---

## Public Functions

### Tenant Management

#### `register_tenant(env: Env, caller: Address, tenant_address: Address, level: TenantLevel, parent: Address) -> Result<Tenant, MeteringError>`

Register a new tenant in the hierarchy.

**Parameters:**

- `caller` - Admin address (must authenticate)
- `tenant_address` - Address of new tenant
- `level` - Hierarchical level (Organization, Clinic, Provider, Patient)
- `parent` - Parent tenant address (self for root orgs)

**Returns:** `Tenant` struct

**Errors:**

- `Unauthorized` - Caller is not admin
- `TenantAlreadyExists` - Tenant address already registered
- `NotInitialized` - Contract not initialized

**Hierarchy:**

```
Organization (parent = self)
  └─ Clinic (parent = Organization)
      └─ Provider (parent = Clinic)
          └─ Patient (parent = Provider)
```

**Example:**

```rust
let org = client.register_tenant(
    &env,
    &admin,
    &org_address,
    &TenantLevel::Organization,
    &org_address  // Self-parent
)?;
```

---

#### `get_tenant(env: Env, tenant_address: Address) -> Result<Tenant, MeteringError>`

Retrieve tenant information.

**Parameters:**

- `tenant_address` - Tenant to query

**Returns:** `Tenant` struct

**Errors:**

- `TenantNotFound` - Tenant not registered

---

#### `list_tenants(env: Env, level: Option<TenantLevel>) -> Vec<Tenant>`

List all tenants, optionally filtered by level.

**Parameters:**

- `level` - Optional filter (e.g., TenantLevel::Clinic)

**Returns:** Vector of matching tenants

---

### Quota Management

#### `set_tenant_quota(env: Env, caller: Address, tenant_address: Address, quota: TenantQuota) -> Result<(), MeteringError>`

Assign usage quota to a tenant (admin only).

**Parameters:**

- `caller` - Admin address
- `tenant_address` - Tenant to configure
- `quota` - Quota configuration:
  - `total_units` - Maximum gas units
  - `burst_units` - Emergency burst allowance
  - `reset_period` - Period for quota reset (seconds)

**Returns:** `Result<(), MeteringError>`

**Errors:**

- `Unauthorized` - Caller not admin
- `TenantNotFound` - Tenant not registered

**Example:**

```rust
let quota = TenantQuota {
    total_units: 1_000_000,
    burst_units: 100_000,
    reset_period: 86400,  // 1 day
};
client.set_tenant_quota(&env, &admin, &clinic_address, &quota)?;
```

---

#### `get_quota(env: Env, tenant_address: Address) -> Result<TenantQuota, MeteringError>`

Retrieve quota settings for a tenant.

**Parameters:**

- `tenant_address` - Tenant to query

**Returns:** `TenantQuota`

**Errors:**

- `TenantNotFound` - Tenant not registered

---

#### `get_quota_usage(env: Env, tenant_address: Address) -> Result<QuotaUsage, MeteringError>`

Get current usage statistics against quota.

**Parameters:**

- `tenant_address` - Tenant to query

**Returns:** `QuotaUsage` struct with current consumption

**Usage Field:**

```rust
pub struct QuotaUsage {
    pub units_consumed: u64,
    pub units_remaining: u64,
    pub percent_utilized: u8,            // 0-100
    pub burst_used: u64,
    pub alert_threshold: u64,            // 80% of quota
}
```

---

### Operation Metering

#### `record_operation(env: Env, tenant_address: Address, op_type: OperationType, cost: u64) -> Result<(), MeteringError>`

Record a single gas-metered operation.

**Parameters:**

- `tenant_address` - Tenant performing operation
- `op_type` - Operation category (Read, Write, Compute, Storage)
- `cost` - Cost in abstract gas units

**Returns:** `Result<(), MeteringError>`

**Errors:**

- `TenantNotFound` - Tenant not registered
- `TenantInactive` - Tenant deactivated
- `QuotaExceeded` - Usage exceeds total quota + burst

**Behavior:**

1. Adds cost to tenant's usage
2. Rolls up cost to all ancestor tenants
3. Emits `QuotaAlertEvent` if usage crosses 80% threshold
4. Rejects operation if quota exhausted

**Example:**

```rust
client.record_operation(
    &env,
    &provider_address,
    &OperationType::Write,
    5  // Write cost = 5 units
)?;
```

---

#### `get_gas_costs(env: Env) -> GasCosts`

Retrieve the current per-operation cost schedule.

**Returns:** `GasCosts` struct

**Default Costs:**

```rust
pub struct GasCosts {
    pub read_cost: u64 = 1,
    pub write_cost: u64 = 5,
    pub compute_cost: u64 = 10,
    pub storage_cost: u64 = 3,
}
```

---

#### `set_gas_costs(env: Env, caller: Address, costs: GasCosts) -> Result<(), MeteringError>`

Update per-operation cost schedule (admin only).

**Parameters:**

- `caller` - Admin address
- `costs` - New cost configuration

**Returns:** `Result<(), MeteringError>`

**Example:**

```rust
let new_costs = GasCosts {
    read_cost: 2,
    write_cost: 7,
    compute_cost: 15,
    storage_cost: 4,
};
client.set_gas_costs(&env, &admin, &new_costs)?;
```

---

### Billing & Settlements

#### `open_billing_cycle(env: Env, caller: Address, tenant_address: Address) -> Result<u64, MeteringError>`

Open a new billing cycle for a tenant.

**Parameters:**

- `caller` - Admin address
- `tenant_address` - Tenant to bill

**Returns:** Cycle ID

**Errors:**

- `CycleAlreadyOpen` - Billing cycle already open for tenant
- `Unauthorized` - Caller not admin

---

#### `close_billing_cycle(env: Env, caller: Address, tenant_address: Address) -> Result<Invoice, MeteringError>`

Close a billing cycle and generate invoice.

**Parameters:**

- `caller` - Admin address
- `tenant_address` - Tenant being billed

**Returns:** `Invoice` struct with charges

**Errors:**

- `NoCycleOpen` - No open billing cycle
- `Unauthorized` - Caller not admin

**Invoice Data:**

```rust
pub struct Invoice {
    pub cycle_id: u64,
    pub tenant_address: Address,
    pub period_start: u64,
    pub period_end: u64,
    pub usage_record: TenantUsageRecord,
    pub charges: i128,
}
```

---

#### `get_tenant_usage(env: Env, tenant_address: Address) -> Result<TenantUsageRecord, MeteringError>`

Retrieve usage statistics for a tenant.

**Parameters:**

- `tenant_address` - Tenant to query

**Returns:** `TenantUsageRecord` with read/write/compute/storage breakdown

---

## Data Types

### TenantLevel

```rust
pub enum TenantLevel {
    Organization,
    Clinic,
    Provider,
    Patient,
}
```

### OperationType

```rust
pub enum OperationType {
    Read,      // Query operation
    Write,     // Update/create operation
    Compute,   // Heavy computation
    Storage,   // Persistent storage cost
}
```

### GasCosts

```rust
pub struct GasCosts {
    pub read_cost: u64,
    pub write_cost: u64,
    pub compute_cost: u64,
    pub storage_cost: u64,
}
```

### Tenant

```rust
pub struct Tenant {
    pub address: Address,
    pub level: TenantLevel,
    pub parent: Address,
    pub registered_at: u64,
    pub active: bool,
}
```

### TenantQuota

```rust
pub struct TenantQuota {
    pub total_units: u64,
    pub burst_units: u64,      // Emergency allowance
    pub reset_period: u64,     // Seconds
}
```

### QuotaUsage

```rust
pub struct QuotaUsage {
    pub units_consumed: u64,
    pub units_remaining: u64,
    pub percent_utilized: u8,
    pub burst_used: u64,
}
```

---

## Storage Keys

| Key           | Purpose                 |
| ------------- | ----------------------- |
| `ADMIN`       | Administrator address   |
| `INITIALIZED` | Initialization flag     |
| `TENANT_KEY`  | Per-tenant registry     |
| `TENANT_LIST` | All registered tenants  |
| `PARENT_KEY`  | Per-tenant parent link  |
| `GAS_COSTS`   | Operation cost schedule |

---

## Error Codes

| Error                 | Code | Description                  |
| --------------------- | ---- | ---------------------------- |
| `NotInitialized`      | 1    | Contract not initialized     |
| `AlreadyInitialized`  | 2    | Contract already initialized |
| `Unauthorized`        | 3    | Caller lacks permission      |
| `TenantNotFound`      | 4    | Tenant not registered        |
| `TenantAlreadyExists` | 5    | Tenant ID duplicate          |
| `TenantInactive`      | 6    | Tenant deactivated           |
| `QuotaExceeded`       | 7    | Usage exceeds quota          |
| `InvalidInput`        | 8    | Invalid parameter            |
| `CycleAlreadyOpen`    | 9    | Billing cycle open           |
| `NoCycleOpen`         | 10   | No active billing cycle      |
| `InvoiceNotFound`     | 11   | Invoice not found            |
| `AlreadySettled`      | 12   | Invoice already paid         |

---

## Events

| Event                | Parameters                            | Description           |
| -------------------- | ------------------------------------- | --------------------- |
| `tenant_registered`  | `(tenant_address, level)`             | Tenant created        |
| `tenant_deactivated` | `(tenant_address)`                    | Tenant inactive       |
| `quota_set`          | `(tenant_address, total_units)`       | Quota configured      |
| `quota_alert`        | `(tenant_address, percent_used)`      | 80% threshold crossed |
| `quota_exceeded`     | `(tenant_address)`                    | Usage exceeds quota   |
| `operation_recorded` | `(tenant_address, op_type, cost)`     | Operation metered     |
| `cycle_opened`       | `(tenant_address, cycle_id)`          | Billing cycle started |
| `cycle_closed`       | `(tenant_address, cycle_id, charges)` | Billing cycle ended   |

---

## Hierarchical Cost Rollup

When a Patient's provider records a Write operation costing 5 units:

```
Patient: +5 units
  Provider: +5 units
    Clinic: +5 units
      Organization: +5 units
```

All ancestors' usage counters are incremented, enabling top-down quota enforcement.

---

## Related Documentation

- [Rate Limiting](../docs/rate-limiting.md)
- [Cost Allocation](../docs/operations.md#cost-allocation)
- [Governance](../docs/governance.md)
