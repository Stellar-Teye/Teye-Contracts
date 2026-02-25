# Treasury Contract API Reference

## Contract Purpose

Manages DAO fund allocation and spending with multisig approval or direct governance authorization. Tracks budget allocations by category and provides governance integration for decentralized fund management.

## Initialization

### `initialize(env: Env, admin: Address, token: Address, signers: Vec<Address>, threshold: u32) -> Result<(), ContractError>`

Initialize the treasury with multisig configuration.

**Parameters:**

- `admin` - Treasury administrator address
- `token` - Token contract for fund transfers (SAC address)
- `signers` - Set of authorized signers (typically 3-7 addresses)
- `threshold` - Required approval count (1 ≤ threshold ≤ signers.len())

**Returns:** `Result<(), ContractError>`

**Errors:**

- `AlreadyInitialized` - Contract already initialized
- `NoSigners` - Empty signers vector
- `InvalidThreshold` - Threshold invalid or exceeds signer count

**Example:**

```rust
let signers = vec![signer1, signer2, signer3];
client.initialize(&env, &admin, &token_address, &signers, 2)?;
```

---

## Public Functions

### Configuration

#### `get_config(env: Env) -> Result<TreasuryConfig, ContractError>`

Retrieve treasury configuration.

**Returns:** `TreasuryConfig` containing admin, token, signers, and threshold

---

#### `set_governor(env: Env, caller: Address, governor: Address) -> Result<(), ContractError>`

Register a Governor contract for direct governance spending (admin only).

**Parameters:**

- `caller` - Admin address (must authenticate)
- `governor` - Governor contract address

**Returns:** `Result<(), ContractError>`

**Errors:**

- `NotAuthorizedCaller` - Caller is not admin

**Note:**
Once set, the Governor contract can use `governor_spend` without multisig approval.

**Example:**

```rust
client.set_governor(&env, &admin, &governor_address)?;
```

---

#### `get_governor(env: Env) -> Option<Address>`

Retrieve the registered Governor contract address.

**Returns:** `Option<Address>`

---

### Governor Integration

#### `governor_spend(env: Env, caller: Address, to: Address, amount: i128) -> Result<(), ContractError>`

Execute a spend authorized by the Governor DAO contract.

**Parameters:**

- `caller` - Must be the registered Governor contract (must authenticate)
- `to` - Recipient address
- `amount` - Amount to transfer (must be > 0)

**Returns:** `Result<(), ContractError>`

**Errors:**

- `NotAuthorizedCaller` - Caller is not the registered Governor
- `PositiveAmountRequired` - Amount ≤ 0
- Token transfer failures

**Storage Update:**

- Increments spending under `"GOVERN"` allocation category

**Example:**

```rust
// Called by Governor contract during proposal execution
client.governor_spend(&env, &governor, &recipient, 1_000_000)?;
```

---

### Multisig Proposals

#### `create_proposal(env: Env, proposer: Address, to: Address, amount: i128, category: Symbol, description: String, expires_at: u64) -> Result<Proposal, ContractError>`

Create a new spending proposal (requires signer status).

**Parameters:**

- `proposer` - Signer proposing spend (must authenticate)
- `to` - Recipient address
- `amount` - Amount to transfer (must be > 0)
- `category` - Budget category (e.g., `symbol_short!("GRANT")`)
- `description` - Proposal description/justification
- `expires_at` - Deadline timestamp (must be in future)

**Returns:** `Proposal` struct with auto-approval by proposer

**Errors:**

- `UnauthorisedProposer` - Caller is not a signer
- `PositiveAmountRequired` - Amount ≤ 0
- `FutureExpiryRequired` - Expiry not in future

**Storage:**
Proposal stored with ID and initial approval from proposer

**Example:**

```rust
let expires_at = env.ledger().timestamp() + 604800;  // 1 week
let proposal = client.create_proposal(
    &env,
    &signer_address,
    &recipient,
    500_000,
    &symbol_short!("GRANT"),
    &"Q1 Research Grant",
    expires_at
)?;
println!("Proposal created with ID: {}", proposal.id);
```

---

#### `get_proposal(env: Env, id: u64) -> Option<Proposal>`

Retrieve a proposal by ID.

**Parameters:**

- `id` - Proposal identifier

**Returns:** `Option<Proposal>` — Full proposal details or None

---

#### `approve_proposal(env: Env, signer: Address, id: u64) -> Result<(), ContractError>`

Sign a proposal (adds approval from signer).

**Parameters:**

- `signer` - Approving signer (must authenticate)
- `id` - Proposal ID

**Returns:** `Result<(), ContractError>`

**Errors:**

- `UnauthorisedSigner` - Caller is not a signer
- `ProposalNotFound` - Invalid proposal ID
- `ProposalNotPending` - Proposal already executed or expired
- `ProposalExpired` - Expiry timestamp exceeded

**Note:**
Duplicate approvals from same signer are no-ops.

**Example:**

```rust
client.approve_proposal(&env, &signer2, proposal_id)?;
client.approve_proposal(&env, &signer3, proposal_id)?;
```

---

#### `execute_proposal(env: Env, signer: Address, id: u64) -> Result<(), ContractError>`

Execute a proposal after threshold approvals received.

**Parameters:**

- `signer` - Executing signer (must authenticate; typically Treasury DAO operator)
- `id` - Proposal ID

**Returns:** `Result<(), ContractError>`

**Errors:**

- `UnauthorisedSigner` - Caller is not a signer
- `ProposalNotFound` - Invalid proposal ID
- `ProposalNotPending` - Already executed
- `InsufficientApprovals` - Approval count < threshold
- `ProposalExpired` - Expiry exceeded
- Token transfer failures

**Behavior:**

1. Verifies threshold met
2. Transfers funds to recipient
3. Marks proposal as executed
4. Updates allocation tracking

**Storage:**

- Updates `ALLOCATION` key with spent amount
- Sets proposal status to Executed

**Example:**

```rust
client.execute_proposal(&env, &signer, proposal_id)?;
```

---

### Allocation Tracking

#### `get_allocation(env: Env, category: Symbol) -> AllocationSummary`

Retrieve total spending for a budget category.

**Parameters:**

- `category` - Budget category symbol

**Returns:** `AllocationSummary { category, total_spent: i128 }`

**Example:**

```rust
let summary = client.get_allocation(&env, &symbol_short!("GRANT"));
println!("Grants spent: {}", summary.total_spent);
```

---

## Data Types

### TreasuryConfig

```rust
pub struct TreasuryConfig {
    pub admin: Address,
    pub token: Address,
    pub signers: Vec<Address>,
    pub threshold: u32,
}
```

### ProposalStatus

```rust
pub enum ProposalStatus {
    Pending,    // Awaiting approval threshold
    Executed,   // Successfully executed
    Expired,    // Deadline passed without execution
}
```

### Proposal

```rust
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub to: Address,
    pub amount: i128,
    pub category: Symbol,
    pub description: String,
    pub approvals: Vec<Address>,       // Signers who approved
    pub status: ProposalStatus,
    pub created_at: u64,
    pub expires_at: u64,
}
```

### AllocationSummary

```rust
pub struct AllocationSummary {
    pub category: Symbol,
    pub total_spent: i128,
}
```

---

## Storage Keys

| Key                 | Symbol                   | Purpose                   |
| ------------------- | ------------------------ | ------------------------- |
| `CONFIG`            | `"CONFIG"`               | Treasury configuration    |
| `GOVERNOR`          | `"GOVERNOR"`             | Governor contract address |
| `PROPOSAL_CTR`      | `"PR_CTR"`               | Proposal ID counter       |
| Proposal storage    | `(PROPOSAL, id)`         | Individual proposal data  |
| Allocation tracking | `(ALLOCATION, category)` | Spending by category      |

---

## Error Codes

| Error                    | Code | Description                   |
| ------------------------ | ---- | ----------------------------- |
| `AlreadyInitialized`     | 1    | Contract already initialized  |
| `NotInitialized`         | 2    | Contract not initialized      |
| `NoSigners`              | 3    | Empty signers list            |
| `InvalidThreshold`       | 4    | Invalid threshold value       |
| `PositiveAmountRequired` | 5    | Amount must be > 0            |
| `UnauthorisedProposer`   | 6    | Caller is not a signer        |
| `FutureExpiryRequired`   | 7    | Expiry must be in future      |
| `UnauthorisedSigner`     | 8    | Caller is not a signer        |
| `ProposalNotFound`       | 9    | Invalid proposal ID           |
| `ProposalNotPending`     | 10   | Proposal not in Pending state |
| `ProposalExpired`        | 11   | Expiry deadline passed        |
| `InsufficientApprovals`  | 12   | Approval count < threshold    |
| `NotAuthorizedCaller`    | 13   | Not the Governor contract     |

---

## Events

**Note:** Events emitted through token contract transfer interface; audit trail maintained in allocation storage.

---

## Typical Workflows

### Multisig Spending Flow

```
1. Signer A: create_proposal(...)     → Proposal created with A's approval
2. Signer B: approve_proposal(id)      → Proposal has 2 approvals
3. Signer C: approve_proposal(id)      → Proposal has 3 approvals (threshold met)
4. Signer A: execute_proposal(id)      → Funds transferred, proposal marked Executed
```

### Governance Spending Flow

```
1. Governor contract calls governor_spend(...) → Funds transferred immediately
2. No multisig approval needed (governance vote served as approval)
```

---

## Related Documentation

- [Governor DAO Integration](./governor.md)
- [Multisig Pattern](../docs/security.md#multisig)
- [Governance](../docs/governance.md)
