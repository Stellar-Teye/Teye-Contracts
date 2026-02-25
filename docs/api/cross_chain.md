# Cross-Chain Contract API Reference

## Contract Purpose

Manages cross-chain message validation and relay. Enables secure identity and credential verification across different blockchain networks using merkle proofs and verified state roots.

## Initialization

### `initialize(env: Env, admin: Address) -> Result<(), CrossChainError>`

Initialize the cross-chain bridge with an admin address.

**Parameters:**

- `admin` - Bridge administrator (must authenticate)

**Returns:** `Result<(), CrossChainError>`

**Errors:**

- `AlreadyInitialized` - Contract already initialized

**Example:**

```rust
client.initialize(&env, &admin_address)?;
```

---

## Public Functions

### Relay Management

#### `add_relayer(env: Env, caller: Address, relayer: Address) -> Result<(), CrossChainError>`

Register a trusted relayer for cross-chain message submission.

**Parameters:**

- `caller` - Admin address (must authenticate)
- `relayer` - Address to be authorized as relayer

**Returns:** `Result<(), CrossChainError>`

**Errors:**

- `NotInitialized` - Contract not initialized
- `Unauthorized` - Caller is not admin

**Storage:**

- Persists relayer address with TTL (30 days)

**Events:**

- `relayer_added(relayer)`

**Example:**

```rust
client.add_relayer(&env, &admin, &relayer_address)?;
```

---

#### `is_relayer(env: Env, address: Address) -> bool`

Check if an address is a trusted relayer.

**Parameters:**

- `address` - Address to check

**Returns:** `bool` — true if relayer authorized

**Note:**
Automatically extends TTL on successful check.

---

#### `remove_relayer(env: Env, caller: Address, relayer: Address) -> Result<(), CrossChainError>`

Remove a relayer's authorization.

**Parameters:**

- `caller` - Admin address (must authenticate)
- `relayer` - Address to deauthorize

**Returns:** `Result<(), CrossChainError>`

**Errors:**

- `Unauthorized` - Caller is not admin

---

### Message Validation

#### `register_foreign_chain(env: Env, caller: Address, chain_id: String) -> Result<(), CrossChainError>`

Register a foreign blockchain as a valid message source.

**Parameters:**

- `caller` - Admin address (must authenticate)
- `chain_id` - Chain identifier (e.g., "ethereum-mainnet")

**Returns:** `Result<(), CrossChainError>`

**Errors:**

- `Unauthorized` - Caller is not admin

**Example:**

```rust
client.register_foreign_chain(&env, &admin, &"ethereum-mainnet")?;
```

---

#### `unregister_foreign_chain(env: Env, caller: Address, chain_id: String) -> Result<(), CrossChainError>`

Remove a foreign chain from trusted sources.

**Parameters:**

- `caller` - Admin address
- `chain_id` - Chain to remove

**Returns:** `Result<(), CrossChainError>`

---

## Message Processing

#### `submit_message(env: Env, relayer: Address, message: CrossChainMessage, merkle_proof: MerkleProof) -> Result<(), CrossChainError>`

Submit a validated cross-chain message with merkle proof.

**Parameters:**

- `relayer` - Submitting relayer (must authenticate and be authorized)
- `message` - Cross-chain message struct:
  - `source_chain` - Originating blockchain ID
  - `source_address` - Sending contract on foreign chain
  - `target_action` - Action symbol (e.g., "GRANT" for access grant)
  - `payload` - Serialized action data (encrypted off-chain)
- `merkle_proof` - Merkle proof authenticating message on source chain

**Returns:** `Result<(), CrossChainError>`

**Errors:**

- `NotInitialized` - Contract not initialized
- `Unauthorized` - Caller not a relayer
- `UnknownIdentity` - Source address not recognized
- `UnsupportedAction` - Target action not supported
- `AlreadyProcessed` - Message already executed

**Events:**

- `message_received(source_chain, source_address, target_action)`

**Processing:**

1. Verifies relayer authorization
2. Validates merkle proof against state root
3. Deserializes payload
4. Executes target action (cross-contract call pattern)
5. Records execution to prevent replay

**Example:**

```rust
let message = CrossChainMessage {
    source_chain: "ethereum-mainnet".into(),
    source_address: "0x123...".into(),
    target_action: symbol_short!("GRANT"),
    payload: action_bytes,
};
client.submit_message(&env, &relayer, &message, &proof)?;
```

---

### State Root Management

#### `update_state_root(env: Env, caller: Address, chain_id: String, root: BytesN<32>, validated_at: u64) -> Result<(), CrossChainError>`

Update the verified state root for a foreign chain.

**Parameters:**

- `caller` - Admin or oracle address (must authenticate)
- `chain_id` - Foreign chain identifier
- `root` - Merkle root from foreign chain
- `validated_at` - Timestamp of validation

**Returns:** `Result<(), CrossChainError>`

**Errors:**

- `Unauthorized` - Caller not authorized
- No errors if successfully updated

**Storage:**

- Persists state root with TTL for merkle proof validation

**Example:**

```rust
let state_root = env.crypto().sha256(&state_data);
client.update_state_root(&env, &admin, &"ethereum-mainnet", &state_root, now)?;
```

---

#### `get_state_root(env: Env, chain_id: String) -> Option<StateRootAnchor>`

Retrieve the latest verified state root for a chain.

**Parameters:**

- `chain_id` - Foreign chain identifier

**Returns:** `Option<StateRootAnchor>` with root and validation timestamp

---

### Proof Verification

#### `verify_merkle_proof(env: Env, proof: MerkleProof, leaf: BytesN<32>, root: BytesN<32>) -> bool`

Verify a merkle proof against a known root.

**Parameters:**

- `proof` - Merkle proof path (field proofs and directions)
- `leaf` - Message hash being proven
- `root` - Known merkle root

**Returns:** `bool` — true if proof valid

**Example:**

```rust
let message_hash = env.crypto().sha256(&message_bytes);
let valid = client.verify_merkle_proof(&env, &proof, &message_hash, &state_root);
```

---

## Data Types

### CrossChainMessage

```rust
pub struct CrossChainMessage {
    pub source_chain: String,          // e.g., "ethereum-mainnet"
    pub source_address: String,        // Sender contract on foreign chain
    pub target_action: Symbol,         // e.g., symbol_short!("GRANT")
    pub payload: Bytes,                // Encrypted/serialized action data
}
```

### MerkleProof

```rust
pub struct MerkleProof {
    pub field_proofs: Vec<FieldProof>, // Path from leaf to root
}

pub struct FieldProof {
    pub hash: BytesN<32>,
    pub direction: u8,                 // 0 = left sibling, 1 = right sibling
}
```

### StateRootAnchor

```rust
pub struct StateRootAnchor {
    pub chain_id: String,
    pub root: BytesN<32>,
    pub validated_at: u64,
}
```

---

## Storage Keys

| Key                | Symbol                      | Purpose                               |
| ------------------ | --------------------------- | ------------------------------------- |
| `ADMIN`            | `"ADMIN"`                   | Bridge administrator                  |
| `INITIALIZED`      | `"INIT"`                    | Initialization flag                   |
| Relayer            | `(RELAYER, address)`        | Relayer authorization                 |
| Foreign chain      | `(CHAIN, chain_id)`         | Registered chains                     |
| State root         | `(ROOT, chain_id)`          | Latest verified state root            |
| Processed messages | `(PROCESSED, message_hash)` | Executed messages (replay prevention) |

---

## Error Codes

| Error                | Code | Description                       |
| -------------------- | ---- | --------------------------------- |
| `NotInitialized`     | 1    | Contract not initialized          |
| `AlreadyInitialized` | 2    | Contract already initialized      |
| `Unauthorized`       | 3    | Caller lacks required permission  |
| `AlreadyProcessed`   | 4    | Message already executed (replay) |
| `UnknownIdentity`    | 5    | Source address not recognized     |
| `UnsupportedAction`  | 6    | Target action not supported       |

---

## Events

| Event                | Parameters                                      | Description                    |
| -------------------- | ----------------------------------------------- | ------------------------------ |
| `initialized`        | `(admin)`                                       | Bridge initialized             |
| `relayer_added`      | `(relayer)`                                     | Relayer authorized             |
| `relayer_removed`    | `(relayer)`                                     | Relayer deauthorized           |
| `chain_registered`   | `(chain_id)`                                    | Chain added to trusted sources |
| `chain_unregistered` | `(chain_id)`                                    | Chain removed                  |
| `state_root_updated` | `(chain_id, root, validated_at)`                | State root updated             |
| `message_received`   | `(source_chain, source_address, target_action)` | Message processed              |

---

## Supported Target Actions

| Action            | Symbol      | Description                |
| ----------------- | ----------- | -------------------------- |
| Grant Access      | `"GRANT"`   | Cross-chain access grant   |
| Verify Credential | `"VERIFY"`  | ZK credential verification |
| Register Patient  | `"REG_PAT"` | Patient registration       |
| Transfer Identity | `"XFER_ID"` | Identity migration         |

---

## Cross-Contract Interactions

This contract typically integrates with:

- **Identity Contract** — For validating cross-chain identities
- **Vision Records** — For granting access to records
- **ZK Verifier** — For credential proofs
- **EMR Bridge** — For healthcare data interoperability

---

## Related Documentation

- [Cross-Chain Design](../docs/cross_chain.md)
- [Merkle Proof Verification](../docs/ZK_INTEGRATION.md#merkle-proofs)
- [Data Portability](../docs/data-portability.md)
