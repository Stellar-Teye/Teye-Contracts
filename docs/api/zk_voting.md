# ZK Voting Contract API Reference

## Contract Purpose

Implements privacy-preserving voting using zero-knowledge proofs. Enables anonymous ballot casting where voters prove membership in an authorized voter set without revealing their identity. Uses Groth16 proofs and merkle tree membership proofs.

## Core Concepts

- **Anonymous Voting:** Voters prove they're on the voting roll without revealing their identity
- **Nullifiers:** One-time use tokens prevent double-voting while maintaining privacy
- **Merkle Root:** Defines the set of authorized voters (root of merkle tree)
- **ZK Proof:** Proves membership in the merkle tree without revealing voter address

---

## Initialization

### `initialize(env: Env, admin: Address, option_count: u32)`

Initialize the ballot with administrative settings.

**Parameters:**

- `admin` - Ballot administrator (must authenticate)
- `option_count` - Number of voting options (≥ 2)

**Returns:** None (asserts on error)

**Panics:**

- If already initialized
- If option_count < 2

**Example:**

```rust
client.initialize(&env, &admin, 3)?;  // 3 voting options
```

---

## Public Functions

### Ballot Configuration

#### `set_merkle_root(env: Env, caller: Address, root: BytesN<32>) -> Result<(), VoteError>`

Set the merkle root defining authorized voters (admin only).

**Parameters:**

- `caller` - Admin address (must authenticate and not paused)
- `root` - Merkle root hash (typically from tree of voter addresses)

**Returns:** `Result<(), VoteError>`

**Effect:**
Enables voters to prove membership by submitting merkle proofs of their inclusion in this root.

**Example:**

```rust
// Merkle root computed from authorized voter list
let root = merkle_tree.compute_root();
client.set_merkle_root(&env, &admin, &root)?;
```

---

#### `set_verification_key(env: Env, caller: Address, vk: VerificationKey) -> Result<(), VoteError>`

Set the Groth16 verification key for ZK proof validation (admin only).

**Parameters:**

- `caller` - Admin address
- `vk` - Verification key for Groth16 proofs

**Returns:** `Result<(), VoteError>`

**Note:**
VK must match the constraints used in the circuit (typically embedding voter address and ballot option).

---

#### `get_verification_key(env: Env) -> Option<VerificationKey>`

Retrieve the currently configured verification key.

**Returns:** `Option<VerificationKey>`

---

#### `close_ballot(env: Env, caller: Address) -> Result<(), VoteError>`

Close the ballot to prevent further voting (admin only).

**Parameters:**

- `caller` - Admin address

**Returns:** `Result<(), VoteError>`

**Effect:**
Sets ballot to closed. Subsequent `cast_vote` calls rejected.

---

### Voting

#### `cast_vote(env: Env, nullifier: BytesN<32>, option_index: OptionIndex, proof: Proof, public_inputs: Vec<BytesN<32>>) -> Result<(), VoteError>`

Submit an anonymous vote with proof of voter eligibility.

**Parameters:**

- `nullifier` - 32-byte one-time token (prevents double-voting)
  - Derived from voter's private key
  - Used only once; reuse is rejected
  - Doesn't reveal voter identity
- `option_index` - Index of option to vote for (0-based, < option_count)
- `proof` - Groth16 proof of merkle membership
- `public_inputs` - Public signals including:
  - First input: merkle root (must match stored root)
  - Second input: derived public nullifier
  - Additional circuit-dependent inputs

**Returns:** `Result<(), VoteError>`

**Errors:**

- `BallotClosed` - Voting closed
- `InvalidOption` - option_index >= option_count
- `NullifierAlreadyUsed` - Voter already voted
- `MerkleRootNotSet` - Root not yet configured
- `InvalidProof` - Proof verification failed

**Verification Steps:**

1. Check ballot is open
2. Validate option index
3. Check nullifier freshness
4. Verify merkle root is set
5. Get verification key
6. Verify Groth16 proof
7. Check public inputs match expectations
8. Record vote and nullifier

**Privacy Guarantees:**

- Ballot link to voter not recorded
- Only vote count and content visible
- No timing correlation possible (async voting)

**Example:**

```rust
let nullifier = keccak256(&voter_private_key, &ballot_nonce);
let vote_result = client.cast_vote(
    &env,
    &nullifier,
    2,           // Vote for option 2
    &groth16_proof,
    &public_signals
)?;
```

---

### Vote Tallying

#### `get_ballot_results(env: Env) -> BallotResults`

Retrieve final vote tallies.

**Parameters:** None

**Returns:** `BallotResults` containing:

- `option_count` - Number of voting options
- `tallies` - Vote count per option
- `closed` - Whether ballot is closed

**Example:**

```rust
let results = client.get_ballot_results(&env);
println!("Results closed: {}", results.closed);
for (i, tally) in results.tallies.iter().enumerate() {
    println!("Option {}: {} votes", i, tally);
}
```

---

#### `get_option_tally(env: Env, option_index: OptionIndex) -> u64`

Get vote count for a specific option.

**Parameters:**

- `option_index` - Option to query

**Returns:** Vote count for that option

---

## Data Types

### BallotResults

```rust
pub struct BallotResults {
    pub option_count: u32,           // Number of voting options
    pub tallies: Vec<u64>,           // Vote count per option
    pub closed: bool,                // Is voting closed?
}
```

### Proof (Groth16)

```rust
pub struct Proof {
    pub a: G1Point,
    pub b: G2Point,
    pub c: G1Point,
}
```

### VerificationKey

Pre-computed Groth16 verification key specific to the voting circuit.

---

## Storage Structures

### DataKey enum

```rust
pub enum DataKey {
    Admin,                          // Admin address
    OptionCount,                    // Number of options
    Closed,                         // Ballot closed?
    MerkleRoot,                     // Voter eligibility root
    VerificationKey,                // Groth16 VK
    Tally(option_index),            // Votes per option
    Nullifier(nullifier_hash),      // Used nullifiers (double-vote prevention)
}
```

---

## Error Codes

| Error                  | Code | Description                       |
| ---------------------- | ---- | --------------------------------- |
| `InvalidOption`        | -    | option_index >= option_count      |
| `NullifierAlreadyUsed` | -    | Voter already voted               |
| `MerkleRootNotSet`     | -    | Merkle root not configured        |
| `InvalidProof`         | -    | Groth16 proof verification failed |
| `BallotClosed`         | -    | Ballot closed to new votes        |

---

## Events

**Note:** For privacy, no events emitted when votes are cast. Tally results visible only after ballot closes.

---

## Typical Workflow

```
1. Admin initializes ballot: initialize(admin, 3)
2. Admin configures merkle root: set_merkle_root(voter_tree.root)
3. Admin sets verification key: set_verification_key(zksnark_vk)
4. Voters submit proofs: cast_vote(nullifier, option, proof, public_inputs)
   - Each vote is anonymous but verified
   - Nullifiers prevent double-voting without revealing identity
5. Admin closes voting: close_ballot()
6. Voter retrieves results: get_ballot_results()
```

---

## Cryptographic Components

### Merkle Tree

- **Leaf:** Hash of voter address (keccak256)
- **Construction:** Binary merkle tree
- **Root:** Public parameter (shared as merkle root)
- **Proof:** Path from leaf to root (typically 15-25 nodes for 32K voters)

### ZK Circuit

Typically enforces:

1. Valid merkle membership proof
2. Nullifier correctly derived from voter secret
3. Vote is in valid option range
4. Public root matches stored root

### Nullifier Scheme

- **Privacy:** Private random value (voter secret + ballot nonce)
- **Derivation:** hash(voter_secret, ballot_id)
- **Double-Vote Prevention:** Reject if nullifier seen before
- **Non-Linkability:** Different nullifiers for different ballots

---

## Privacy Analysis

| Property                   | How Achieved                                               |
| -------------------------- | ---------------------------------------------------------- |
| **Voter Anonymity**        | No link between nullifier and address stored               |
| **Vote Secrecy**           | Tally visible, but not which voter cast which vote         |
| **Double-Vote Prevention** | Nullifier tracking without voter identification            |
| **No Timing Correlation**  | Async voting prevents inferring voter from activity timing |

---

## Cross-Contract Interactions

ZK Voting may integrate with:

- **ZK Verifier** — For proof verification delegation
- **Staking Contract** — To verify voter eligibility (stakeholder voting)
- **Treasury** — For outcome-dependent fund allocation

---

## Related Documentation

- [ZK Integration Guide](../docs/zk-integration-guide.md)
- [Governance](../docs/governance.md)
- [Post-Quantum Research](../docs/post-quantum-zk-research.md)
