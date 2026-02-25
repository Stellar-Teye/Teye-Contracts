# ZK Integration Guide

This guide explains how to integrate Zero-Knowledge proofs into your Soroban smart contracts using the Teye ZK Verifier.

## Overview

Integrating ZK proofs involves two main parts:
1. **Off-chain**: Generating the proof and formatting it for Soroban.
2. **On-chain**: Calling the `ZkVerifierContract` from your own contract to authorize actions.

## Off-chain Integration

### 1. Circuit Design
Teye uses Groth16 proofs. You should design your circuits using [Circom](https://docs.circom.io/). Ensure your circuit returns public signals that match the `public_inputs` expected by your on-chain logic.

### 2. Generating the Proof
Use `snarkjs` or a similar tool to generate the proof and `public_signals`.

### 3. Formatting for Soroban
Use the `ZkAccessHelper` (available in the `zk_verifier` crate) to format the binary proof data.

```rust
use zk_verifier::ZkAccessHelper;

let request = ZkAccessHelper::create_request(
    &env,
    user_address,
    resource_id,
    proof_a_bytes,   // [u8; 64]
    proof_b_bytes,   // [u8; 128]
    proof_c_bytes,   // [u8; 64]
    &[&public_input_1, &public_input_2] // &[&[u8; 32]]
);
```

## On-chain Integration

### 1. Add Dependency
Add `zk_verifier` to your `Cargo.toml`:

```toml
[dependencies]
zk_verifier = { path = "../zk_verifier" }
```

### 2. Call the Verifier
Use the `ZkVerifierContractClient` to call the verifier from your contract.

```rust
use soroban_sdk::{contract, contractimpl, Address, Env};
use zk_verifier::{ZkVerifierContractClient, AccessRequest};

#[contract]
pub struct MyProtectedContract;

#[contractimpl]
impl MyProtectedContract {
    pub fn do_protected_action(env: Env, verifier_addr: Address, request: AccessRequest) {
        let verifier = ZkVerifierContractClient::new(&env, &verifier_addr);

        // Verify the proof
        if verifier.verify_access(&request) {
            // Proof is valid, proceed with the action
        } else {
            // Proof is invalid
            panic!("Unauthorized access: Invalid ZK proof");
        }
    }
}
```

### 3. Error Handling
The `verify_access` function may return a `ContractError`. You can handle these specific cases:

| Error | Meaning |
|-------|---------|
| `Unauthorized` | User not whitelisted or admin only check failed. |
| `RateLimited` | User has exceeded their allowed request window. |
| `InvalidConfig` | Verifier contract configuration is malformed. |
| `DegenerateProof` | Proof points are improperly formatted (all zeros).|

## Best Practices

- **Resource IDs**: Use unique 32-byte identifiers for different protected actions to prevent proof replay across different resources.
- **Audit Logs**: Successful verifications are automatically logged by the `ZkVerifierContract`. You can query these logs using `get_audit_record`.
- **Witness Privacy**: Never expose private inputs in the `public_inputs` array.
