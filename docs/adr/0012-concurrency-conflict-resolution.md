# ADR-0012: Concurrency and Conflict Resolution Strategy

- **Status**: Accepted
- **Date**: 2026-02-25

## Context

Multiple actors can update shared records concurrently, creating conflicts and inconsistent state. The system needs a consistent strategy to detect, resolve, and audit conflicts without losing data. This must integrate with contract storage constraints and on-chain execution limits.

## Decision

- Use vector clocks for concurrency tracking.
- Apply an operational transform approach for compatible updates.
- Provide conflict resolution strategies with explicit outcomes.
- Store conflict metadata for auditing and manual review.

## Rationale

- Vector clocks provide a lightweight way to detect causality and divergence.
- Operational transforms reduce manual conflict resolution for common cases.
- Explicit strategies improve predictability and auditability.

## Consequences

- Positive: higher consistency for concurrent updates.
- Positive: reduced manual intervention for compatible changes.
- Negative: additional storage and computation overhead.
- Negative: edge cases may still require manual resolution.
- Negative: conflict metadata increases storage usage.

## References

- contracts/common/src/vector_clock.rs
- contracts/common/src/conflict_resolver.rs
- contracts/common/src/operational_transform.rs
- contracts/common/src/concurrency.rs

## Implementation Notes

- Update operations capture version stamps and field-level changes.
- Conflict resolution emits events for audit trails.
