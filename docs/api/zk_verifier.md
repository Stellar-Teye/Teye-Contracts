# ZK Verifier Contract API Reference

## Contract Purpose

Implements BN254 (Alt-BN128) Groth16 zero-knowledge proof verification. Enables privacy-preserving access control by verifying proofs of credential possession without revealing underlying credentials on-chain.

## Overview

ZK Verifier is a privacy-first authentication system where:

- Users prove they meet criteria using cryptographic proofs (not credentials)
- Proofs cryptographically guarantee validity without exposing sensitive data
- Access decisions happen on-chain based solely on proof verification results
- Audit trail records verification events without revealing proof content

---

## Initialization

### `initialize(env: Env, admin: Address)`

Set the contract administrator.

**Parameters:**

- `admin` - Administrator address (must authenticate)

**Note:**
Initialization is idempotent — calling twice has no effect.

**Example:**

```rust
client.initialize(&env, &admin_address);
```

---

## Admin Functions

### Admin Transfer

#### `propose_admin(env: Env, current_admin: Address, new_admin: Address) -> Result<(), ContractError>`

Propose a new administrator (current admin only).

**Parameters:**

- `current_admin` - Current admin (must authenticate)
- `new_admin` - Proposed new admin address

**Returns:** `Result<(), ContractError>`

**Errors:**

- `Unauthorized` - Caller is not current admin

**Events:**

- `admin_transfer_proposed(current_admin, new_admin)`

---

#### `accept_admin(env: Env, new_admin: Address) -> Result<(), ContractError>`

Accept the pending admin transfer.

**Parameters:**

- `new_admin` - Proposed new admin (must authenticate)

**Returns:** `Result<(), ContractError>`

**Errors:**

- `Unauthorized` - Caller is not pending admin
- `InvalidConfig` - No pending admin transfer

**Events:**

- `admin_transfer_accepted(old_admin, new_admin)`

---

### Configuration

#### `set_rate_limit_config(env: Env, caller: Address, window_duration_seconds: u64, max_requests_per_window: u64) -> Result<(), ContractError>`

Configure rate limiting for proof verification (admin only).

**Parameters:**

- `caller` - Admin address
- `window_duration_seconds` - Rate limit window (typically 60-3600)
- `max_requests_per_window` - Allowed verifications per window

**Returns:** `Result<(), ContractError>`

**Example:**

```rust
// 10 verifications per 60 seconds
client.set_rate_limit_config(&env, &admin, 60, 10)?;
```

---

#### `set_pause_status(env: Env, caller: Address, paused: bool) -> Result<(), ContractError>`

Pause/unpause the contract (admin only).

**Parameters:**

- `caller` - Admin address
- `paused` - new pause status

**Returns:** `Result<(), ContractError>`

---

#### `is_paused(env: Env) -> bool`

Check if contract is paused.

**Returns:** `bool`

---

## Proof Verification

### Main Verification Endpoint

#### `verify_proof(env: Env, request: AccessRequest, auth_level: u32) -> Result<bool, ContractError>`

Verify a Groth16 ZK proof and check access level requirements.

**Parameters:**

- `request` - Access request containing:
  - `user` - User address requesting access (must authenticate)
  - `resource_id` - 32-byte hash of resource being accessed
  - `proof` - Groth16 proof (points A, B, C on BN254)
  - `public_inputs` - Public signals (1-16 inputs)
- `auth_level` - Required authentication level (1-4):
  - `1` - Basic proof validity
  - `2` - Proof + single attribute
  - `3` - Proof + multiple attributes
  - `4` - Proof + strong requirements (2+ public inputs)

**Returns:** `Result<bool, ContractError>` — true if proof valid and access granted

**Errors:**

- `Unauthorized` - User not authenticated
- `RateLimited` - Rate limit exceeded
- `EmptyPublicInputs` - No public inputs provided
- `TooManyPublicInputs` - More than 16 inputs
- `DegenerateProof` - Proof points all zeros
- `OversizedProofComponent` - Component has invalid encoding
- `MalformedG1Point` - Bad G1 elliptic curve point
- `MalformedG2Point` - Bad G2 elliptic curve point
- `ZeroedPublicInput` - Public input all zeros
- `InvalidAuthLevel` - Auth level not in [1,4]
- `ProofRequiredForAuthLevel` - Insufficient public inputs for level
- `Paused` - Contract is paused

**Verification Steps:**

1. Request shape validation
2. Auth level validation
3. Proof component structural checks
4. Groth16 pairing verification
5. Public input validation
6. Rate limit check
7. Audit trail recording

**Example:**

```rust
let request = AccessRequest {
    user: user_address,
    resource_id: resource_hash,
    proof: Proof { a, b, c },
    public_inputs: credentials_proof,
};

let verified = client.verify_proof(&env, &request, 2)?;
if verified {
    // Grant access
}
```

---

### Whitelist Management

#### `add_to_whitelist(env: Env, caller: Address, address: Address) -> Result<(), ContractError>`

Add an address to the whitelist (admin only).

**Parameters:**

- `caller` - Admin address
- `address` - Address to whitelist

**Returns:** `Result<(), ContractError>`

**Effect:**
Whitelisted addresses bypass rate limiting.

---

#### `remove_from_whitelist(env: Env, caller: Address, address: Address) -> Result<(), ContractError>`

Remove an address from the whitelist.

**Parameters:**

- `caller` - Admin address
- `address` - Address to remove

**Returns:** `Result<(), ContractError>`

---

#### `is_whitelisted(env: Env, address: Address) -> bool`

Check if an address is whitelisted.

**Parameters:**

- `address` - Address to check

**Returns:** `bool`

---

## Audit Trail

### Access Audit

#### `get_access_audit_trail(env: Env, user: Address) -> Vec<AuditRecord>`

Retrieve proof verification audit trail for a user.

**Parameters:**

- `user` - User address

**Returns:** Vector of audit records containing:

- User address
- Resource ID (hash only, no content)
- Verification result (pass/fail)
- Timestamp
- Auth level checked

**Privacy:**
Audit records contain no proof content or credential details — only verification outcome.

---

#### `get_resource_audit(env: Env, resource_id: BytesN<32>) -> Vec<AuditRecord>`

Get all verification attempts on a specific resource.

**Parameters:**

- `resource_id` - Resource hash

**Returns:** Vector of audit records for that resource

---

## Data Types

### AccessRequest

```rust
pub struct AccessRequest {
    pub user: Address,              // User requesting access
    pub resource_id: BytesN<32>,    // Hash of resource
    pub proof: Proof,               // Groth16 proof
    pub public_inputs: Vec<BytesN<32>>,  // 1-16 public signals
}
```

### Proof (Groth16)

```rust
pub struct Proof {
    pub a: G1Point,     // Elliptic curve point on BN254 G1
    pub b: G2Point,     // Elliptic curve point on BN254 G2
    pub c: G1Point,     // Elliptic curve point on BN254 G1
}

pub struct G1Point {
    pub x: BytesN<32>,
    pub y: BytesN<32>,
}

pub struct G2Point {
    pub x: (BytesN<32>, BytesN<32>),  // Quadratic extension
    pub y: (BytesN<32>, BytesN<32>),
}
```

### VerificationKey

Pre-computed Groth16 verification key (BN254 curve parameters)

### AuditRecord

```rust
pub struct AuditRecord {
    pub user: Address,
    pub resource_id: BytesN<32>,
    pub result: bool,           // Verification passed?
    pub timestamp: u64,
    pub auth_level: u32,
}
```

---

## Storage Keys

| Key             | Symbol       | Purpose                      |
| --------------- | ------------ | ---------------------------- |
| `ADMIN`         | `"ADMIN"`    | Current admin address        |
| `PENDING_ADMIN` | `"PEND_ADM"` | Proposed new admin           |
| `RATE_CFG`      | `"RATECFG"`  | Rate limit configuration     |
| `RATE_TRACK`    | `"RLTRK"`    | Per-user rate limit tracking |

---

## Error Codes

| Error                       | Code | Description                           |
| --------------------------- | ---- | ------------------------------------- |
| `Unauthorized`              | 1    | User not authenticated                |
| `RateLimited`               | 2    | Rate limit exceeded                   |
| `InvalidConfig`             | 3    | Configuration error                   |
| `EmptyPublicInputs`         | 4    | No public inputs provided             |
| `TooManyPublicInputs`       | 5    | More than 16 inputs                   |
| `DegenerateProof`           | 6    | Proof contains zero points            |
| `OversizedProofComponent`   | 7    | Component encoding invalid            |
| `MalformedG1Point`          | 8    | G1 point malformed                    |
| `MalformedG2Point`          | 9    | G2 point malformed                    |
| `ZeroedPublicInput`         | 10   | Public input all zeros                |
| `MalformedProofData`        | 11   | Cross-contract deserialization failed |
| `Paused`                    | 12   | Contract is paused                    |
| `InvalidAuthLevel`          | 13   | Auth level not in [1,4]               |
| `ProofRequiredForAuthLevel` | 14   | Insufficient public inputs            |

---

## Events

| Event                     | Parameters                    | Description        |
| ------------------------- | ----------------------------- | ------------------ |
| `admin_transfer_proposed` | `(old_admin, new_admin)`      | Transfer initiated |
| `admin_transfer_accepted` | `(old_admin, new_admin)`      | Transfer completed |
| `proof_verified`          | `(user, resource_id, result)` | Proof verified     |
| `access_rejected`         | `(user, resource_id, reason)` | Access denied      |
| `whitelist_updated`       | `(address, added: bool)`      | Whitelist changed  |

---

## BN254 Curve Details

- **Field size:** 254 bits
- **Subgroup order:** ~254 bits (Fr)
- **Embedding degree:** 12
- **Optimal for:** Groth16 proofs
- **Pairing-friendly:** Yes (optimal ate pairing)

---

## Rate Limiting

Rate limiting prevents abuse while allowing legitimate users:

- **Configuration:** Adjustable window and request limit
- **Tracking:** Per-user consumption tracked per window
- **Bypass:** Admin whitelist for critical applications
- **Events:** Access violation events logged

---

## Cross-Contract Interactions

ZK Verifier is called by:

- **Identity Contract** — For ZK credential verification
- **Vision Records** — For privacy-preserving access
- **Custom Applications** — For general proof verification

---

## Related Documentation

- [ZK Integration Guide](../docs/zk-integration-guide.md)
- [Post-Quantum ZK Research](../docs/post-quantum-zk-research.md)
- [Identity Contract](./identity.md#zk-credential-verification)
