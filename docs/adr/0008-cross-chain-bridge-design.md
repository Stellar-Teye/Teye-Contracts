# ADR-0008: Cross-Chain Bridge Design

- **Status**: Accepted
- **Date**: 2026-02-25

## Context

The system needs cross-chain data portability and asset movement without compromising security. Interoperability should support verifiable message relay while accommodating chain finality differences and partial trust assumptions. The bridge must keep message formats stable and auditable.

## Decision

- Use a relay-based architecture with Merkle tree proofs for message inclusion.
- Define a canonical message format for cross-chain payloads.
- Rely on a trust model where relays are permissioned but auditable.
- Require finality checks before accepting cross-chain state.

## Rationale

- Merkle proofs provide compact, verifiable inclusion evidence.
- A canonical message format reduces integration risk across chains.
- A permissioned relay set reduces spam and provides operational accountability.

## Consequences

- Positive: verifiable inclusion for cross-chain messages.
- Positive: predictable integration contract interfaces.
- Negative: relay trust introduces an assumption that must be monitored.
- Negative: liveness depends on relay availability.
- Negative: finality delays can slow cross-chain workflows.

## References

- contracts/cross_chain/
- docs/cross_chain.md

## Implementation Notes

- Bridge messages include chain ID, nonce, payload, and inclusion proof.
- Relay operators are subject to governance oversight.
