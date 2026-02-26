# Analytics Contract API Reference

## Contract Purpose

Provides privacy-preserving analytics on health metrics using homomorphic encryption and differential privacy. Enables secure aggregation of sensitive health data across multiple providers without exposing individual records.

## Initialization

### `initialize(env: Env, admin: Address, aggregator: Address, pub_key: PaillierPublicKey, priv_key: Option<PaillierPrivateKey>) -> Result<(), ContractError>`

Initialize the analytics contract with admin credentials and encryption keys.

**Parameters:**

- `admin` - Address with admin privileges
- `aggregator` - Address authorized to aggregate and decrypt metrics
- `pub_key` - Paillier public key for homomorphic operations
- `priv_key` - Optional Paillier private key (stored if provided)

**Returns:** `Result<(), ContractError>`

**Errors:**

- `AlreadyInitialized` - Contract has been initialized before
- `NotInitialized` - Required storage not initialized

**Example:**

```rust
let pub_key = PaillierPublicKey { /* ... */ };
let priv_key = Some(PaillierPrivateKey { /* ... */ });
client.initialize(&env, &admin, &aggregator, &pub_key, &priv_key)?;
```

---

## Public Functions

### Cryptographic Operations

#### `encrypt(env: Env, m: i128) -> i128`

Encrypt a plaintext metric value using the Paillier public key.

**Parameters:**

- `m` - Plaintext metric value to encrypt

**Returns:** Encrypted value (i128)

**Example:**

```rust
let metric_value = 42i128;
let encrypted = client.encrypt(&env, &metric_value);
```

---

#### `add_ciphertexts(env: Env, c1: i128, c2: i128) -> i128`

Add two encrypted values using homomorphic encryption properties. Result remains encrypted.

**Parameters:**

- `c1` - First encrypted value
- `c2` - Second encrypted value

**Returns:** Sum of encrypted values (encrypted)

**Example:**

```rust
let encrypted_sum = client.add_ciphertexts(&env, &encrypted_value_1, &encrypted_value_2);
```

---

#### `decrypt(env: Env, caller: Address, c: i128) -> Result<i128, ContractError>`

Decrypt a ciphertext. Only the registered aggregator can decrypt.

**Parameters:**

- `caller` - Requesting address (must be aggregator)
- `c` - Encrypted value to decrypt

**Returns:** Decrypted plaintext value

**Errors:**

- `Unauthorized` - Caller is not the aggregator or private key not available

**Example:**

```rust
let plaintext = client.decrypt(&env, &aggregator, &encrypted_value)?;
```

---

### Aggregation & Metrics

#### `aggregate_records(env: Env, caller: Address, kind: Symbol, dims: MetricDimensions, ciphertexts: Vec<i128>) -> Result<(), ContractError>`

Aggregate encrypted metric records with differential privacy noise.

**Parameters:**

- `caller` - Requesting address (must be aggregator)
- `kind` - Metric category (e.g., `symbol_short!("BP")` for blood pressure)
- `dims` - Metric dimensions: region, age band, condition, time bucket
- `ciphertexts` - Vector of encrypted metric values to aggregate

**Returns:** `Result<(), ContractError>`

**Storage Update:**
Stores computed aggregate count and sum with Laplace noise applied for differential privacy.

**Errors:**

- `Unauthorized` - Caller is not authorized

**Example:**

```rust
let dims = MetricDimensions {
    region: Some(symbol_short!("US_WEST")),
    age_band: Some(symbol_short!("50_60")),
    condition: Some(symbol_short!("MYOPIA")),
    time_bucket: 1707000000,
};
let encrypted_records = vec![encrypted_val_1, encrypted_val_2];
client.aggregate_records(&env, &caller, &symbol_short!("BP"), &dims, &encrypted_records)?;
```

---

#### `get_metric(env: Env, kind: Symbol, dims: MetricDimensions) -> MetricValue`

Retrieve aggregated metric statistics for given dimensions.

**Parameters:**

- `kind` - Metric category
- `dims` - Metric dimensions (region, age band, condition, time bucket)

**Returns:** `MetricValue { count: i128, sum: i128 }`

**Example:**

```rust
let metric = client.get_metric(&env, &symbol_short!("BP"), &dims);
println!("Count: {}, Sum: {}", metric.count, metric.sum);
```

---

#### `get_trend(env: Env, kind: Symbol, region: Option<Symbol>, age_band: Option<Symbol>, condition: Option<Symbol>, start_bucket: u64, end_bucket: u64) -> Vec<TrendPoint>`

Retrieve time-series trend data across multiple time buckets.

**Parameters:**

- `kind` - Metric category
- `region` - Optional regional filter
- `age_band` - Optional age band filter
- `condition` - Optional condition filter
- `start_bucket` - Starting timestamp bucket
- `end_bucket` - Ending timestamp bucket (inclusive)

**Returns:** Vector of `TrendPoint` { time_bucket, value: MetricValue }

**Example:**

```rust
let trend = client.get_trend(&env, &symbol_short!("BP"), None, None, None, 1706000000, 1708000000);
for point in trend {
    println!("Bucket {}: count={}, sum={}", point.time_bucket, point.value.count, point.value.sum);
}
```

---

### Admin Functions

#### `get_admin(env: Env) -> Address`

Retrieve the admin address.

**Returns:** Admin's Address

---

#### `get_aggregator(env: Env) -> Address`

Retrieve the aggregator address.

**Returns:** Aggregator's Address

---

## Data Types

### MetricDimensions

```rust
pub struct MetricDimensions {
    pub region: Option<Symbol>,       // Geographic region
    pub age_band: Option<Symbol>,     // Age bracket
    pub condition: Option<Symbol>,    // Medical condition
    pub time_bucket: u64,             // Timestamp bucket
}
```

### MetricValue

```rust
pub struct MetricValue {
    pub count: i128,     // Number of records
    pub sum: i128,       // Aggregated sum (with DP noise)
}
```

### TrendPoint

```rust
pub struct TrendPoint {
    pub time_bucket: u64,        // Timestamp of this bucket
    pub value: MetricValue,      // Aggregated metrics
}
```

### PaillierPublicKey / PaillierPrivateKey

Cryptographic keys for homomorphic encryption (structure defined in `homomorphic.rs`)

---

## Storage Keys

| Key          | Symbol       | Purpose                               |
| ------------ | ------------ | ------------------------------------- |
| `ADMIN`      | `"ADMIN"`    | Contract administrator address        |
| `AGGREGATOR` | `"AGGR"`     | Authorized aggregator address         |
| `PUB_KEY`    | `"PUB_KEY"`  | Paillier public key                   |
| `PRIV_KEY`   | `"PRIV_KEY"` | Paillier private key (if stored)      |
| `METRIC`     | `"METRIC"`   | Aggregated metrics by dimensional key |

---

## Error Codes

| Error                | Code | Description                       |
| -------------------- | ---- | --------------------------------- |
| `AlreadyInitialized` | 1    | Contract already initialized      |
| `NotInitialized`     | 2    | Contract not yet initialized      |
| `Unauthorized`       | 3    | Caller lacks required permissions |

---

## Events

**No explicit events defined** — audit trail maintained through persistent metric storage.

---

## Related Documentation

- [Homomorphic Encryption Design](../docs/ZK_INTEGRATION.md#homomorphic)
- [Differential Privacy](../docs/ZK_INTEGRATION.md#differential-privacy)
- [Storage Key Conventions](../storage-key-conventions.md)
- [Indexer Guide](../indexer.md) — for metric query patterns
