# ZK Integration Guide

This document defines the Zero-Knowledge architecture and integration details for the `zk_verifier` Soroban smart contract, tracking Issue #43.

## Cryptographic Primitives

The Stellar-Teye ZK Verifier employs:
1.  **Curve Environment:** BN254 (Alt-BN128) representing efficient pairing-friendly elliptic curves operations suitable for smart contract operations.
2.  **Proof System:** Groth16. Chosen due to its very small verification key size, fast on-chain execution with constant pairing configurations, and optimized prover/verifier ratio.
3.  **Hashing Function:** Poseidon over the BN254 scalar field. It ensures public inputs can be natively validated without introducing excessive R1Cs constraints.

## Architecture

1. **Access Requests:**
   Off-chain components leverage the `ZkAccessHelper` to format binary proofs (A, B, C points in Groth16 format) alongside the scalar Poseidon hashes of state boundaries into an interoperable Soroban Wasm execution format (`AccessRequest`).

2. **On-Chain Verifier:**
   The `ZkVerifierContract` handles the verification. It first requires caller authorization. Next, it validates the Groth16 proof pairing against the given `public_inputs` via the abstracted `Bn254Verifier` module.

3. **Audit Trails:**
   Upon successful verification, the state is persisted into the ledger and an event log occurs tracking: `(user, resource_id, proof_hash, timestamp)`. The actual details behind the verification stay completely zero-knowledge, yielding a privacy-first infrastructure.

## Interaction

**Generating the Proof:**
Utilize standard tools like Circom / SnarkJS off-chain to write the ZK circuits and derive the proving keys and witness outputs. Generate the Groth16 proofs corresponding to the BN254 verifying keys.

**Smart Contract Hook Integration:**

```rust
use zk_verifier::{ZkVerifierContractClient, AccessRequest};

// Inside another Soroban Wasm runtime:
let verifier_client = ZkVerifierContractClient::new(&env, &verifier_address);
let valid = verifier_client.verify_access(&access_request);
if valid {
    // Proceed with restricted resource access
}
```
