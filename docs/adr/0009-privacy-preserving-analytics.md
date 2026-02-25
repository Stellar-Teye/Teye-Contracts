# ADR-0009: Privacy-Preserving Analytics

- **Status**: Accepted
- **Date**: 2026-02-25

## Context

The platform needs analytics and reporting over sensitive vision care data without exposing PHI. The system must enable aggregate insights while complying with privacy and regulatory expectations. Solutions must be compatible with on-chain constraints and off-chain processing.

## Decision

- Use differential privacy for aggregate metrics.
- Apply homomorphic encryption for select computations over encrypted values.
- Keep privacy budget tracking explicit and enforceable.

## Rationale

- Differential privacy provides formal privacy guarantees for aggregates.
- Homomorphic encryption enables limited analytics without raw data exposure.
- Combining both approaches balances usability and compliance risk.

## Consequences

- Positive: reduced risk of PHI leakage in analytics outputs.
- Positive: analytics can be shared with lower compliance burden.
- Negative: privacy budget limits the frequency and precision of queries.
- Negative: computation overhead increases runtime and cost.
- Negative: results may be less accurate due to noise and constraints.

## References

- contracts/analytics/
- docs/analytics.md

## Implementation Notes

- Privacy budgets are tracked per metric domain and time window.
- Encrypted aggregates are stored with associated metadata for auditing.
