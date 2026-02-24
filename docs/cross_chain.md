# Cross-Chain Medical Records Access

## Overview
This document describes the architectural design and message passing protocol for the `cross_chain` module in the Stellar-Teye contracts ecosystem. It aims to fulfill interoperability requirements (Issue #40) by enabling foreign blockchains to safely interact with Vision records.

## Message Passing Protocol
The cross-chain bridge is designed around a trusted relayer network. Given the constraints of Soroban's lightweight execution environment, heavy zero-knowledge or light-client proofs of foreign chains are delegated. Instead, relayers—which are authenticated entities—submit validated messages.

Each message contains:
- `source_chain` (String): e.g., "ethereum", "polygon".
- `source_address` (String): e.g., "0xabc123...".
- `target_action` (Symbol): e.g., `GRANT` for granting access to records.
- `payload` (Bytes): Additional payload data for the action.

The `process_message` endpoint additionally accepts a separate `message_id: Bytes` argument (a unique identifier or hash of the message) used to prevent replay attacks.

## Identity Mapping
Because users on foreign chains use different wallet addressing formats (e.g., Ethereum 20-byte addresses vs. Stellar/Soroban 32-byte Ed25519 public keys or contract addresses), we provide an identity mapping endpoint:
```rust
fn map_identity(admin, foreign_chain, foreign_address, local_address)
```
This maps `(foreign_chain, foreign_address)` directly to the user's `local_address` on Soroban. When a cross-chain message is processed, this map is consulted so the `cross_chain` contract can act on the correct local patient's behalf.

## Access Control and Verification
- **Relayers**: Only explicitly whitelisted relayers (added via `add_relayer`) can invoke `process_message`.
- **Replay Protection**: The message ID is saved in storage (`PROCESSED_MESSAGES`) upon execution. If a relayer submits the same message twice, the transaction simply reverts.
- **Cross-Contract Authorization**: When granting access to a third-party (`GRANT`), the bridge forwards the action to the `vision_records` contract. Standard RBAC authorization applies; the patient must have delegated permissions to the bridge or the bridge must be recognized as a system administrator to manage access on their behalf.
