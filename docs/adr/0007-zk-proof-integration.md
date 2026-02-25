# ADR-0007: Zero-Knowledge Proof Integration

- **Status**: Accepted
- **Date**: 2026-02-25

## Context

The protocol needs privacy-preserving verification for sensitive workflows such as eligibility checks, role attestations, and ZK-enabled voting. These requirements demand proofs that reveal minimal information while still being verifiable on-chain. The system must balance proof size, verification cost, and developer ergonomics.

## Decision

- Separate proof generation (off-chain) from proof verification (on-chain).
- Support proof schemes that are efficient for on-chain verification (e.g., Groth16 and Plonk-style proofs).
- Provide a dedicated verifier contract and an SDK-based prover for client tooling.
- Keep heavy computation off-chain; only verification runs on-chain.

## Rationale

- On-chain proof generation is infeasible due to gas and runtime limits.
- A verifier contract centralizes verification logic and simplifies auditability.
- SDK-based provers allow optimized tooling and iteration without contract upgrades.

## Consequences

- Positive: privacy-preserving validation without exposing PHI.
- Positive: predictable on-chain costs for verification.
- Negative: proof size constraints impact transaction payloads.
- Negative: verifier costs add overhead to sensitive workflows.
- Negative: proof scheme changes require careful backward compatibility planning.

## References

- contracts/zk_verifier/
- contracts/zk_voting/
- sdk/zk_prover/
- docs/ZK_INTEGRATION.md
- docs/zk-architecture.md
- docs/zk-integration-guide.md
- docs/post-quantum-zk-research.md

## Implementation Notes

- Proofs are constructed off-chain and submitted with public inputs.
- Verifier contract validates proofs and emits events for auditing.
