# Staking & Rewards System

The `StakingContract` provides on-chain staking for governance participation and incentivisation on the Stellar/Soroban platform. Users deposit stake tokens to earn reward tokens proportional to their share of the total pool, with a configurable timelock on withdrawals.

---

## Algorithm: Accumulated Reward-Per-Token

The contract uses the **Synthetix-style accumulated reward-per-token** model — the same algorithm used by Uniswap v3, Compound, and the broader DeFi ecosystem. It is the canonical O(1)-per-user solution to proportional reward distribution.

### Core invariant

A global counter `reward_per_token_stored` (RPT) accumulates monotonically:

```
RPT += reward_rate × elapsed_seconds × PRECISION / total_staked
```

Each user stores a snapshot `reward_per_token_paid` taken at their last interaction. Their **instantaneous pending reward** is:

```
pending = staked × (RPT − reward_per_token_paid) / PRECISION + earned_so_far
```

`PRECISION = 1_000_000_000_000` (10¹²) keeps 12 decimal places of accuracy without floating-point arithmetic. All intermediate products use `i128` with saturating arithmetic to prevent overflow.

### Complexity

| Operation | Time | Space |
|-----------|------|-------|
| `stake` | O(1) | O(1) |
| `request_unstake` | O(1) | O(1) |
| `withdraw` | O(1) | O(1) |
| `claim_rewards` | O(1) | O(1) |
| `set_reward_rate` | O(1) | O(1) |

No iteration over stakers — ever.

---

## Contract Location

```
contracts/staking/
├── Cargo.toml
└── src/
    ├── lib.rs        ← public contract interface
    ├── rewards.rs    ← pure reward-per-token math engine
    ├── timelock.rs   ← unstake queue
    ├── events.rs     ← on-chain event publishers
    └── test.rs       ← full test suite (19 tests)
```

---

## Deployment

```bash
# Build WASM
cargo build --target wasm32-unknown-unknown --release -p staking

# Deploy to local network
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/staking.wasm \
  --source default \
  --network local

# Initialise
soroban contract invoke --id <CONTRACT_ID> --source default --network local \
  -- initialize \
  --admin <ADMIN_ADDRESS> \
  --stake_token <STAKE_SAC_ADDRESS> \
  --reward_token <REWARD_SAC_ADDRESS> \
  --reward_rate 10 \
  --lock_period 86400
```

---

## Public API

### `initialize`

```
initialize(admin, stake_token, reward_token, reward_rate, lock_period) → Result<(), ContractError>
```

One-time bootstrap. `reward_rate` is tokens-per-second emitted across all stakers. `lock_period` is seconds between `request_unstake` and eligible `withdraw`.

---

### `stake`

```
stake(staker, amount) → Result<(), ContractError>
```

Transfer `amount` stake tokens from `staker` into the contract. The reward accumulator is synced before the balance change, so the staker earns from _this point forward_ on the full new balance.

| Error | Condition |
|-------|-----------|
| `InvalidInput` | `amount ≤ 0` |
| `NotInitialized` | Contract not yet bootstrapped |

---

### `request_unstake`

```
request_unstake(staker, amount) → Result<u64, ContractError>
```

Queue `amount` tokens for withdrawal. The staked balance is reduced immediately; the tokens are held in escrow until `unlock_at = now + lock_period`. Returns the `request_id`.

| Error | Condition |
|-------|-----------|
| `InsufficientBalance` | `staked < amount` |
| `InvalidInput` | `amount ≤ 0` |

---

### `withdraw`

```
withdraw(staker, request_id) → Result<(), ContractError>
```

Return escrowed tokens to `staker` after the timelock. Implements **checks-effects-interactions**: the request is marked `withdrawn = true` before the token transfer.

| Error | Condition |
|-------|-----------|
| `TimelockNotExpired` | `now < unlock_at` |
| `AlreadyWithdrawn` | Already called for this `request_id` |
| `RequestNotFound` | Invalid `request_id` |
| `Unauthorized` | `staker ≠ request.staker` |

---

### `claim_rewards`

```
claim_rewards(staker) → Result<i128, ContractError>
```

Transfer all accumulated reward tokens to `staker`. Returns the amount transferred (0 if nothing pending — does not revert).

---

### View Functions

| Function | Returns | Description |
|----------|---------|-------------|
| `get_staked(staker)` | `i128` | Current staked balance |
| `get_pending_rewards(staker)` | `i128` | Real-time earned rewards (read-only, no state change) |
| `get_staker_info(staker)` | `StakerInfo` | Combined position snapshot |
| `get_total_staked()` | `i128` | Sum of all active stakes |
| `get_reward_rate()` | `i128` | Current tokens-per-second emission |
| `get_lock_period()` | `u64` | Configured unstake lock in seconds |
| `get_unstake_request(id)` | `UnstakeRequest` | Detailed request record |
| `is_initialized()` | `bool` | Whether the contract is live |
| `get_admin()` | `Address` | Stored admin address |

---

### Admin Functions

```
set_reward_rate(caller, new_rate) → Result<(), ContractError>
set_lock_period(caller, new_period) → Result<(), ContractError>
```

Only the stored `admin` may call these. `set_reward_rate` flushes the global accumulator at the **old rate** first, preserving reward correctness across the transition. `set_lock_period` only affects future unstake requests.

---

## Events

All state changes emit typed on-chain events for indexers and front-ends.

| Symbol | Trigger | Key payload fields |
|--------|---------|-------------------|
| `INIT` | `initialize` | `admin, stake_token, reward_token, reward_rate, lock_period` |
| `STAKED` | `stake` | `staker, amount, new_total_staked` |
| `UNSTK_REQ` | `request_unstake` | `request_id, staker, amount, unlock_at` |
| `WITHDRAWN` | `withdraw` | `request_id, staker, amount` |
| `CLMD` | `claim_rewards` | `staker, amount` |
| `RWD_RATE` | `set_reward_rate` | `new_rate` |
| `LOCK_SET` | `set_lock_period` | `new_period` |

---

## Error Codes

| Code | Value | Meaning |
|------|-------|---------|
| `NotInitialized` | 1 | Called before `initialize` |
| `AlreadyInitialized` | 2 | `initialize` called twice |
| `Unauthorized` | 3 | Caller lacks permission for admin function |
| `InvalidInput` | 4 | Zero or negative amount |
| `InsufficientBalance` | 5 | Unstake amount exceeds staked balance |
| `TimelockNotExpired` | 6 | Withdraw called before `unlock_at` |
| `AlreadyWithdrawn` | 7 | Duplicate `withdraw` for the same request |
| `RequestNotFound` | 8 | `request_id` does not exist |

---

## Storage Layout

| Key pattern | Storage tier | Type | Description |
|-------------|-------------|------|-------------|
| `ADMIN` | instance | `Address` | Admin address |
| `INIT` | instance | `bool` | Initialization flag |
| `STK_TOK` | instance | `Address` | Stake token contract |
| `RWD_TOK` | instance | `Address` | Reward token contract |
| `RWD_RATE` | instance | `i128` | Tokens emitted per second |
| `TOT_STK` | instance | `i128` | Global staked total |
| `RPT` | instance | `i128` | Global reward-per-token accumulator |
| `LAST_UPD` | instance | `u64` | Last RPT flush timestamp |
| `LOCK_PER` | instance | `u64` | Unstake timelock seconds |
| `USTK_CTR` | instance | `u64` | Monotonic request ID counter |
| `(STK, user)` | persistent | `i128` | Per-user staked balance |
| `(RPT_PAID, user)` | persistent | `i128` | Per-user RPT snapshot |
| `(ERND, user)` | persistent | `i128` | Per-user accumulated earned |
| `(USTK_REQ, id)` | persistent | `UnstakeRequest` | Unstake queue entry |

---

## Governance Integration Notes

- Staking stake tokens grants governance weight proportional to `get_staked(user)`.
- A governance contract can query `get_staked` to determine voting power.
- The timelock ensures vote-weight cannot be flash-staked and immediately withdrawn.
- The admin can adjust `lock_period` to require longer commitment as the protocol matures.

---

## Testing

```bash
# All tests
cargo test -p staking

# Verbose output
cargo test -p staking -- --nocapture

# All workspace (including staking)
cargo test --all
```

The test suite covers 19 scenarios across initialization, staking, proportional reward accrual, timelock enforcement, double-withdraw prevention, multi-staker fairness, and admin access control.
