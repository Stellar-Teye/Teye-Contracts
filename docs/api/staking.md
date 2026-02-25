# Staking Contract API Reference

## Contract Purpose

Manages token staking with reward accumulation and timelocked withdrawal. Integrates with admin tier system and multisig for governance participation. Supports time-weighted loyalty calculations for DAO governance voting power.

## Initialization

### `initialize(env: Env, admin: Address, stake_token: Address, reward_token: Address, reward_rate: i128, lock_period: u64) -> Result<(), ContractError>`

Bootstrap the staking contract with token addresses and parameters.

**Parameters:**

- `admin` - Administrator address (becomes SuperAdmin)
- `stake_token` - Token contract address for deposits
- `reward_token` - Token contract address for reward distribution
- `reward_rate` - Tokens emitted **per second** across all stakers (must be ≥ 0)
- `lock_period` - Withdrawal wait period in seconds (typical: 604800 for 1 week)

**Returns:** `Result<(), ContractError>`

**Errors:**

- `AlreadyInitialized` - Contract already initialized
- `InvalidInput` - Negative reward rate
- `TokensIdentical` - Stake and reward tokens are the same

**Example:**

```rust
const LOCK_PERIOD: u64 = 604800;  // 1 week in seconds
client.initialize(
    &env,
    &admin,
    &stake_token_address,
    &reward_token_address,
    1000i128,  // 1000 tokens per second
    LOCK_PERIOD
)?;
```

---

## Public Functions

### Staking Operations

#### `stake(env: Env, staker: Address, amount: i128) -> Result<(), ContractError>`

Deposit tokens into the staking pool.

**Parameters:**

- `staker` - Address depositing tokens (must authenticate)
- `amount` - Quantity of stake tokens (must be > 0)

**Returns:** `Result<(), ContractError>`

**Behavior:**

1. Requires `staker` authentication
2. Updates global reward accumulator (prevents retroactive reward on new deposit)
3. Transfers `amount` stake tokens from `staker` to contract
4. Records first-stake timestamp for loyalty age tracking (one-time only)
5. Increases user and global staked balance

**Errors:**

- `NotInitialized` - Contract not initialized
- `InvalidInput` - Amount ≤ 0
- `Paused` - Contract is paused
- Token transfer failures

**Events:**

- `staked(staker, amount, new_total_staked)`

**Storage Updates:**

- `USER_STAKE` - User's staked balance
- `TOTAL_STAKED` - Global staked total
- `USER_SINCE` - Timestamp of first stake (written once)
- `USER_RPT_PAID` - User's reward-per-token snapshot
- `USER_EARNED` - User's accumulated rewards

**Example:**

```rust
client.stake(&env, &user_address, 1000i128)?;
```

---

#### `request_unstake(env: Env, staker: Address, amount: i128) -> Result<u64, ContractError>`

Queue tokens for withdrawal after the timelock expires.

**Parameters:**

- `staker` - Address requesting withdrawal (must authenticate)
- `amount` - Quantity of tokens to unstake (must be > 0, ≤ staked balance)

**Returns:** `Result<u64, ContractError>` — ID of the unstake request

**Behavior:**

1. Updates global rewards before reducing stake
2. Reduces user's staked balance immediately (prevents reward accrual on queued amount)
3. Creates timelock request with expiration timestamp
4. Queued amount is no longer eligible for rewards

**Errors:**

- `NotInitialized` - Contract not initialized
- `InvalidInput` - Amount ≤ 0
- `InsufficientBalance` - Amount exceeds staked balance
- `Paused` - Contract is paused

**Events:**

- `unstake_requested(staker, amount, request_id, expires_at)`

**Storage Updates:**

- `USER_STAKE` - Reduced by amount
- Unstake requests stored with expiration

**Example:**

```rust
let request_id = client.request_unstake(&env, &user_address, 500i128)?;
println!("Unstake request created with ID: {}", request_id);
```

---

#### `withdraw(env: Env, staker: Address, request_id: u64) -> Result<(), ContractError>`

Withdraw tokens after the timelock has expired.

**Parameters:**

- `staker` - Address withdrawing (must authenticate)
- `request_id` - ID from `request_unstake`

**Returns:** `Result<(), ContractError>`

**Behavior:**

1. Verifies timelock expired
2. Transfers queued tokens + accrued rewards to staker
3. Marks request as withdrawn

**Errors:**

- `NotInitialized` - Contract not initialized
- `RequestNotFound` - Invalid request_id
- `TimelockNotExpired` - Must wait longer
- `AlreadyWithdrawn` - Request already processed
- Token transfer failures

**Events:**

- `withdrawn(staker, amount, rewards_claimed)`

**Example:**

```rust
client.withdraw(&env, &user_address, request_id)?;
```

---

### Reward Management

#### `claim_rewards(env: Env, staker: Address) -> Result<(), ContractError>`

Claim accumulated rewards without unstaking.

**Parameters:**

- `staker` - Address claiming rewards (must authenticate)

**Returns:** `Result<(), ContractError>`

**Behavior:**

1. Updates global reward accumulator
2. Transfers earned rewards to staker
3. Resets user's earned rewards to zero

**Errors:**

- `NotInitialized` - Contract not initialized
- `InsufficientBalance` - No rewards to claim
- Token transfer failures

**Events:**

- `rewards_claimed(staker, amount)`

**Example:**

```rust
client.claim_rewards(&env, &user_address)?;
```

---

#### `get_staker_info(env: Env, staker: Address) -> StakerInfo`

Retrieve current staking position and pending rewards.

**Parameters:**

- `staker` - Address to query

**Returns:** `StakerInfo { staked: i128, pending_rewards: i128 }`

**Example:**

```rust
let info = client.get_staker_info(&env, &user_address);
println!("Staked: {}, Pending Rewards: {}", info.staked, info.pending_rewards);
```

---

#### `get_staker_since(env: Env, staker: Address) -> Option<u64>`

Get the timestamp of a staker's first deposit (for loyalty age calculation).

**Parameters:**

- `staker` - Address to query

**Returns:** `Option<u64>` — Timestamp or None if never staked

---

### Reward Rate Management

#### `set_reward_rate(env: Env, caller: Address, new_rate: i128) -> Result<(), ContractError>`

Update the per-second reward emission rate (admin only).

**Parameters:**

- `caller` - Admin address (must authenticate & have admin tier)
- `new_rate` - New reward rate (tokens per second)

**Returns:** `Result<(), ContractError>`

**Errors:**

- `Unauthorized` - Caller lacks admin privileges
- `InvalidInput` - Negative rate

**Events:**

- `reward_rate_changed(old_rate, new_rate)`

---

#### `request_rate_change(env: Env, caller: Address, new_rate: i128) -> Result<(), ContractError>`

Propose a reward rate change (multisig path).

**Parameters:**

- `caller` - MultiSig proposer
- `new_rate` - Proposed reward rate

**Returns:** `Result<(), ContractError>`

**Errors:**

- `MultisigRequired` - Insufficient approvals
- `InvalidInput` - Invalid rate

---

#### `approve_rate_change(env: Env, caller: Address) -> Result<(), ContractError>`

Approve a pending rate change (multisig voting).

**Parameters:**

- `caller` - MultiSig signer (must authenticate)

**Returns:** `Result<(), ContractError>`

---

#### `execute_rate_change(env: Env, caller: Address) -> Result<(), ContractError>`

Execute a rate change after multisig threshold met.

**Parameters:**

- `caller` - Executer address

**Returns:** `Result<(), ContractError>`

**Errors:**

- `NoPendingRateChange` - No rate change pending
- `RateChangeNotReady` - Threshold not met or delay not expired

---

### Global State Queries

#### `get_total_staked(env: Env) -> i128`

Retrieve total tokens staked across all users.

**Returns:** Total staked amount

---

#### `get_reward_rate(env: Env) -> i128`

Get the current per-second reward emission rate.

**Returns:** Reward rate (tokens per second)

---

#### `get_lock_period(env: Env) -> u64`

Get the unstake timelock duration in seconds.

**Returns:** Lock period duration

---

### Contract State

#### `pause(env: Env, caller: Address) -> Result<(), ContractError>`

Pause staking/unstaking operations (admin only).

**Parameters:**

- `caller` - Admin address

**Returns:** `Result<(), ContractError>`

---

#### `unpause(env: Env, caller: Address) -> Result<(), ContractError>`

Resume staking/unstaking operations.

**Parameters:**

- `caller` - Admin address

**Returns:** `Result<(), ContractError>`

---

#### `is_paused(env: Env) -> bool`

Check if contract is paused.

**Returns:** `bool`

---

## Data Types

### StakerInfo

```rust
pub struct StakerInfo {
    pub staked: i128,              // Currently staked tokens
    pub pending_rewards: i128,     // Accrued but unclaimed rewards
}
```

### UnstakeRequest

```rust
pub struct UnstakeRequest {
    pub id: u64,
    pub staker: Address,
    pub amount: i128,
    pub created_at: u64,
    pub expires_at: u64,           // Timelock expiration
}
```

### RateChangeProposal

```rust
pub struct RateChangeProposal {
    pub old_rate: i128,
    pub new_rate: i128,
    pub proposer: Address,
    pub approvals: Vec<Address>,
    pub created_at: u64,
    pub delay_until: u64,          // Execution delay
}
```

---

## Storage Keys

| Key                 | Symbol                   | Purpose                      |
| ------------------- | ------------------------ | ---------------------------- |
| `ADMIN`             | `"ADMIN"`                | Admin/SuperAdmin address     |
| `INITIALIZED`       | `"INIT"`                 | Initialization flag          |
| `STAKE_TOKEN`       | `"STK_TOK"`              | Stake token address          |
| `REWARD_TOKEN`      | `"RWD_TOK"`              | Reward token address         |
| `REWARD_RATE`       | `"RWD_RATE"`             | Per-second emission rate     |
| `TOTAL_STAKED`      | `"TOT_STK"`              | Global staked total          |
| `REWARD_PER_TOKEN`  | `"RPT"`                  | Accumulated reward-per-token |
| `LAST_UPDATE`       | `"LAST_UPD"`             | Last accumulator update time |
| `LOCK_PERIOD`       | `"LOCK_PER"`             | Unstake timelock duration    |
| User stake          | `(USER_STAKE, address)`  | Per-user staked balance      |
| User rewards        | `(USER_EARNED, address)` | Per-user earned rewards      |
| User loyalty marker | `(USER_SINCE, address)`  | First-stake timestamp        |

---

## Error Codes

| Error                 | Code | Description                  |
| --------------------- | ---- | ---------------------------- |
| `NotInitialized`      | 1    | Contract not initialized     |
| `AlreadyInitialized`  | 2    | Contract already initialized |
| `Unauthorized`        | 3    | Caller lacks permissions     |
| `InvalidInput`        | 4    | Invalid parameter value      |
| `InsufficientBalance` | 5    | Insufficient tokens          |
| `TimelockNotExpired`  | 6    | Withdrawal timelock active   |
| `AlreadyWithdrawn`    | 7    | Request already processed    |
| `RequestNotFound`     | 8    | Invalid request ID           |
| `TokensIdentical`     | 9    | Stake and reward tokens same |
| `RateChangeNotReady`  | 10   | Rate change not executable   |
| `NoPendingRateChange` | 11   | No rate change in progress   |
| `MultisigRequired`    | 12   | Multisig threshold not met   |
| `MultisigError`       | 13   | Multisig operations failed   |
| `Paused`              | 14   | Contract is paused           |

---

## Events

| Event                 | Parameters                                              | Description           |
| --------------------- | ------------------------------------------------------- | --------------------- |
| `initialized`         | `(admin, stake_token, reward_token, rate, lock_period)` | Contract initialized  |
| `staked`              | `(staker, amount, new_total)`                           | Tokens staked         |
| `unstake_requested`   | `(staker, amount, request_id, expires_at)`              | Unstake queued        |
| `withdrawn`           | `(staker, amount, rewards_claimed)`                     | Withdrawal executed   |
| `rewards_claimed`     | `(staker, amount)`                                      | Rewards claimed       |
| `reward_rate_changed` | `(old_rate, new_rate)`                                  | Emission rate updated |
| `access_violation`    | `(caller, action, required_permission)`                 | Authorization failure |

---

## Related Documentation

- [Admin Tier System](../docs/governance.md#admin-tiers)
- [MultiSig Pattern](../docs/security.md#multisig)
- [Treasury Integration](./treasury.md) — connected via Governor
