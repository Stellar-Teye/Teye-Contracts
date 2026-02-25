# Identity Contract API Reference

## Contract Purpose

Manages user identity recovery and social recovery mechanisms through guardian-based multi-signature approval. Enables secure identity ownership transfer and ZK credential verification without exposing sensitive credentials on-chain.

## Initialization

### `initialize(env: Env, owner: Address) -> Result<(), RecoveryError>`

Initialize the identity contract with an owner address.

**Parameters:**

- `owner` - The initial identity owner and administrator

**Returns:** `Result<(), RecoveryError>`

**Errors:**

- `AlreadyInitialized` - Contract has been initialized before

**Example:**

```rust
client.initialize(&env, &owner_address)?;
```

---

## Public Functions

### Guardian Management

#### `add_guardian(env: Env, caller: Address, guardian: Address) -> Result<(), RecoveryError>`

Register a new guardian for social recovery (maximum 5 guardians per owner).

**Parameters:**

- `caller` - The identity owner (must authenticate)
- `guardian` - Address of guardian to add

**Returns:** `Result<(), RecoveryError>`

**Errors:**

- `Unauthorized` - Caller is not the active owner
- `GuardianLimitExceeded` - Already have 5 guardians

**Events:**

- `guardian_changed(caller, guardian, added=true)`

**Example:**

```rust
client.add_guardian(&env, &owner, &guardian_address)?;
```

---

#### `remove_guardian(env: Env, caller: Address, guardian: Address) -> Result<(), RecoveryError>`

Remove a guardian from the recovery list.

**Parameters:**

- `caller` - The identity owner (must authenticate)
- `guardian` - Address of guardian to remove

**Returns:** `Result<(), RecoveryError>`

**Errors:**

- `Unauthorized` - Caller is not the active owner

**Events:**

- `guardian_changed(caller, guardian, added=false)`

**Example:**

```rust
client.remove_guardian(&env, &owner, &guardian_address)?;
```

---

#### `get_guardians(env: Env, owner: Address) -> Vec<Address>`

Retrieve all guardians for an owner.

**Parameters:**

- `owner` - The identity owner's address

**Returns:** Vector of guardian addresses

**Example:**

```rust
let guardians = client.get_guardians(&env, &owner);
println!("Guardians: {:?}", guardians);
```

---

#### `is_guardian(env: Env, owner: Address, guardian: Address) -> bool`

Check if an address is a registered guardian for an owner.

**Parameters:**

- `owner` - The identity owner's address
- `guardian` - Address to check

**Returns:** `bool` — true if registered guardian

---

### Recovery Management

#### `set_recovery_threshold(env: Env, caller: Address, threshold: u32) -> Result<(), RecoveryError>`

Set the M-of-N approval threshold for recovery (M must be ≤ number of guardians).

**Parameters:**

- `caller` - The identity owner (must authenticate)
- `threshold` - Number of required approvals (1 to 5)

**Returns:** `Result<(), RecoveryError>`

**Errors:**

- `Unauthorized` - Caller is not the active owner
- `InvalidThreshold` - Threshold exceeds number of guardians

**Example:**

```rust
client.set_recovery_threshold(&env, &owner, 3)?;  // Require 3-of-5 approval
```

---

#### `initiate_recovery(env: Env, guardian: Address, owner: Address, new_address: Address) -> Result<(), RecoveryError>`

Guardian initiates recovery by proposing a new identity address.

**Parameters:**

- `guardian` - The guardian initiating recovery (must authenticate)
- `owner` - Current identity owner being recovered
- `new_address` - Proposed new identity address

**Returns:** `Result<(), RecoveryError>`

**Errors:**

- `Unauthorized` - Caller is not a registered guardian
- `RecoveryAlreadyActive` - Recovery request already in progress

**Events:**

- `recovery_initiated(owner, new_address, guardian)`

**Storage Update:**

- Creates new `RecoveryRequest` with initiating guardian's approval

**Example:**

```rust
client.initiate_recovery(&env, &guardian, &owner, &new_address)?;
```

---

#### `approve_recovery(env: Env, guardian: Address, owner: Address) -> Result<(), RecoveryError>`

Guardian votes to approve an active recovery request.

**Parameters:**

- `guardian` - The approving guardian (must authenticate)
- `owner` - The owner whose recovery is being approved

**Returns:** `Result<(), RecoveryError>`

**Errors:**

- `Unauthorized` - Caller is not a registered guardian
- `RecoveryNotFound` - No active recovery request for this owner
- `AlreadyApproved` - Guardian already approved this request

**Example:**

```rust
client.approve_recovery(&env, &guardian, &owner)?;
```

---

#### `execute_recovery(env: Env, caller: Address, owner: Address) -> Result<Address, RecoveryError>`

Execute recovery after cooldown period and sufficient approvals.

**Parameters:**

- `caller` - Address calling recovery (must authenticate)
- `owner` - The owner address being recovered

**Returns:** `Result<Address, RecoveryError>` — The new identity address

**Errors:**

- `RecoveryNotFound` - No active recovery request
- `CooldownNotExpired` - Must wait before executing
- `InsufficientApprovals` - Not enough guardian signatures
- `Unauthorized` - Caller not authorized

**Events:**

- `recovery_executed(owner, new_address)`

**Storage Update:**

- Transfers ownership to new address
- Deactivates old identity
- Clears recovery request

**Example:**

```rust
let new_address = client.execute_recovery(&env, &caller, &owner)?;
println!("Identity recovered to: {}", new_address);
```

---

#### `cancel_recovery(env: Env, caller: Address) -> Result<(), RecoveryError>`

Owner cancels an active recovery request.

**Parameters:**

- `caller` - The identity owner (must authenticate)

**Returns:** `Result<(), RecoveryError>`

**Errors:**

- `Unauthorized` - Caller is not the active owner
- `RecoveryNotFound` - No active recovery request

**Events:**

- `recovery_cancelled(caller)`

**Example:**

```rust
client.cancel_recovery(&env, &owner)?;
```

---

#### `get_recovery_request(env: Env, owner: Address) -> Option<RecoveryRequest>`

Retrieve the active recovery request for an owner.

**Parameters:**

- `owner` - The identity owner's address

**Returns:** `Option<RecoveryRequest>` — Details of active recovery or None

**Example:**

```rust
if let Some(req) = client.get_recovery_request(&env, &owner) {
    println!("Recovery in progress: {}", req.new_address);
}
```

---

#### `get_recovery_threshold(env: Env, owner: Address) -> u32`

Get the current recovery threshold for an owner.

**Parameters:**

- `owner` - The identity owner's address

**Returns:** M value from M-of-N scheme

---

### Ownership Status

#### `is_owner_active(env: Env, owner: Address) -> bool`

Check if an address is an active identity owner.

**Parameters:**

- `owner` - Address to check

**Returns:** `bool` — true if active owner

---

### ZK Credential Verification

#### `set_zk_verifier(env: Env, caller: Address, verifier_id: Address) -> Result<(), RecoveryError>`

Configure the ZK verifier contract for credential verification.

**Parameters:**

- `caller` - The identity owner (must authenticate)
- `verifier_id` - Address of deployed zk_verifier contract

**Returns:** `Result<(), RecoveryError>`

**Errors:**

- `Unauthorized` - Caller is not the active owner

---

#### `get_zk_verifier(env: Env) -> Option<Address>`

Retrieve the configured ZK verifier contract address.

**Returns:** `Option<Address>`

---

#### `verify_zk_credential(env: Env, user: Address, resource_id: BytesN<32>, proof_a: VkG1Point, proof_b: VkG2Point, proof_c: VkG1Point, public_inputs: Vec<BytesN<32>>) -> Result<bool, CredentialError>`

Verify a ZK credential proof via cross-contract call to zk_verifier.

**Parameters:**

- `user` - Address requesting verification (must authenticate)
- `resource_id` - Hash of resource being accessed
- `proof_a`, `proof_b`, `proof_c` - Groth16 proof points (BN254)
- `public_inputs` - Public inputs for proof verification

**Returns:** `Result<bool, CredentialError>` — true if credential valid

**Errors:**

- `Unauthorized` - ZK verifier not configured
- `CredentialInvalid` - Proof does not validate

**Events:**

- `zk_credential_verified(user, verified)`

**Cross-Contract Call:**
Delegates to configured `zk_verifier` contract

**Example:**

```rust
let verified = client.verify_zk_credential(
    &env,
    &user,
    &resource_id,
    &proof_a,
    &proof_b,
    &proof_c,
    &public_inputs
)?;
```

---

## Data Types

### RecoveryRequest

```rust
pub struct RecoveryRequest {
    pub owner: Address,
    pub new_address: Address,
    pub initiator: Address,
    pub approvals: Vec<Address>,      // Guardian approvals received
    pub created_at: u64,              // Initiation timestamp
    pub cooldown_expires_at: u64,     // Earliest execution time
}
```

---

## Storage Keys

| Key                | Symbol                 | Purpose                |
| ------------------ | ---------------------- | ---------------------- |
| `ADMIN`            | `"ADMIN"`              | Primary identity owner |
| `INITIALIZED`      | `"INIT"`               | Initialization flag    |
| Guardian list      | Per-owner persistent   | List of guardians      |
| Recovery threshold | Per-owner instance     | M value for M-of-N     |
| Recovery request   | Per-owner persistent   | Active recovery data   |
| Owner status       | Per-address persistent | Active/inactive flag   |

---

## Error Codes

| Error                   | Code | Description                            |
| ----------------------- | ---- | -------------------------------------- |
| `NotInitialized`        | 1    | Contract not initialized               |
| `AlreadyInitialized`    | 2    | Contract already initialized           |
| `Unauthorized`          | 3    | Caller lacks required permissions      |
| `GuardianLimitExceeded` | 4    | Cannot add more than 5 guardians       |
| `InvalidThreshold`      | 5    | Threshold exceeds guardian count       |
| `RecoveryNotFound`      | 6    | No active recovery request             |
| `CooldownNotExpired`    | 7    | Must wait before executing recovery    |
| `InsufficientApprovals` | 8    | Not enough guardian approvals          |
| `AlreadyApproved`       | 9    | Guardian already approved this request |

---

## Events

| Event                    | Parameters                        | Description                       |
| ------------------------ | --------------------------------- | --------------------------------- |
| `guardian_changed`       | `(owner, guardian, added: bool)`  | Guardian added or removed         |
| `recovery_initiated`     | `(owner, new_address, initiator)` | Recovery process started          |
| `recovery_executed`      | `(owner, new_address)`            | Recovery successfully completed   |
| `recovery_cancelled`     | `(owner)`                         | Recovery request cancelled        |
| `zk_credential_verified` | `(user, verified: bool)`          | ZK credential verification result |

---

## Related Documentation

- [ZK Verifier Integration](./zk_verifier.md)
- [Recovery Mechanisms](../docs/emergency-protocol.md)
- [Cross-Chain Identity](./cross_chain.md)
