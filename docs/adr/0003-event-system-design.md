# ADR-0003: Event and Observability Design

- **Status**: Accepted
- **Date**: 2026-02-23

## Context

Stellar Teye is a multi-contract system that must be:

- **Observable** in production (errors, access events, provider changes, staking activity).
- Integrable with:
  - Off-chain indexers
  - Monitoring stacks (Prometheus, Grafana)
  - Security tooling
- Compliant with healthcare auditing needs:
  - Who did what, when, and to which resources?

## Decision

- Use Soroban **events** as the primary on-chain signal for:
  - Successful business operations (record created, access granted/revoked, provider verified).
  - Contract-level configuration changes (initialisation, parameter updates).
  - Security and error conditions (structured error events).
- Define **structured event types** per contract:
  - `contracts/vision_records/src/events.rs` for:
    - `InitializedEvent`
    - `UserRegisteredEvent`
    - `RecordAddedEvent`
    - `AccessGrantedEvent`
    - `AccessRevokedEvent`
    - Provider lifecycle events
    - `publish_error` with `ErrorContext`
  - `contracts/staking/src/events.rs` for:
    - `StakedEvent`, `UnstakeRequestedEvent`, `WithdrawnEvent`
    - `RewardClaimedEvent`, `RewardRateSetEvent`, `LockPeriodSetEvent`
  - `contracts/cross_chain/src/events.rs` for:
    - Cross-chain mapping and message processing events
- Pair events with **error logging** where appropriate:
  - On certain failures, log to an in-contract error ring buffer and emit an error event.
- Integrate with off-chain monitoring:
  - Index events into the Prometheus/Grafana stack described in `docs/monitoring.md`.
  - Use events as a source for:
    - Contract health dashboards
    - Alert rules (e.g., error rate anomalies, failed cross-chain messages).

## Rationale

- Events are:
  - Cheap to emit and easy to index.
  - Naturally suited for append-only audit trails.
- Structured events make it:
  - Easier to filter and aggregate by type, category, or resource.
  - Easier to evolve contracts while keeping observability stable.
- Combining events with instance-level error logs:
  - Supports both deep forensic inspection and real-time monitoring.

## Consequences

- **Positive**:
  - Strong auditability of user actions and contract operations.
  - Rich telemetry surface for monitoring, anomaly detection, and business analytics.
  - Easier correlation between contract calls, errors, and external system metrics.
- **Negative / Trade-offs**:
  - Slight increase in contract code for event definitions and publisher helpers.
  - Indexers must understand and maintain mappings from event schemas to storage/metrics models.

## Implementation Notes

- Event definitions and publishers:
  - `contracts/vision_records/src/events.rs`
  - `contracts/staking/src/events.rs`
  - `contracts/cross_chain/src/events.rs`
- Error logging and categorisation:
  - `contracts/vision_records/src/errors.rs`
  - Surfaces error logs and metrics via contract methods and events.
- Monitoring setup:
  - `docs/monitoring.md`
  - `scripts/monitor/*` (Prometheus, Grafana, Alertmanager, health checks)

