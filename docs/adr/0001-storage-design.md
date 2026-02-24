# ADR-0001: Storage Design for Vision Records and Supporting Contracts

- **Status**: Accepted
- **Date**: 2026-02-23

## Context

Stellar Teye manages sensitive vision care data and must:

- Guarantee **immutability and auditability** for record-related metadata.
- Respect healthcare privacy requirements by keeping **PHI off-chain**.
- Operate within Soroban's storage and TTL model, balancing:
  - Cost (storage rent)
  - Performance
  - Data longevity

The platform includes multiple Soroban contracts (vision records, staking, cross-chain, analytics) as well as off-chain services.

## Decision

- **On-chain storage**:
  - Store only **record metadata and hashes** of encrypted data, never raw PHI.
  - Use Soroban **persistent storage** for long-lived state:
    - Vision records (`VisionRecord`)
    - Users and providers
    - Access grants
    - RBAC assignments and delegations
    - Staking balances and reward state
    - Cross-chain identity mappings
    - Analytics aggregates
  - Use Soroban **instance storage** for:
    - Error logs and counters
    - Contract configuration (admin, token addresses, signers)
    - Aggregate statistics (e.g., treasury allocations by category)
- **TTL and retention**:
  - Use dedicated helpers (`extend_ttl_*`) to keep hot keys alive above a threshold.
  - Accept that **cold data may expire** on-chain, while off-chain archives preserve full history.
- **Keying strategy**:
  - Use small `Symbol` prefixes plus typed keys (e.g., `u64`, `Address`, structs) to:
    - Keep serialization predictable
    - Avoid collisions
  - Prefer tuples such as `(PREFIX, id)` or `(PREFIX, address)` for clarity.

## Rationale

- Keeping PHI off-chain:
  - Minimizes compliance and breach impact surface.
  - Allows migration between storage providers without on-chain changes.
- Using persistent storage for contract-critical state ensures:
  - Deterministic behaviour across invocations.
  - Straightforward querying by indexers and off-chain services.
- Instance storage is well-suited for:
  - Bounded logs (error rings, retry counters).
  - Configuration values that are read frequently but rarely changed.
- TTL helpers centralize the pattern of:
  - Extending only when necessary.
  - Avoiding accidental early expiration of active data.

## Consequences

- **Positive**:
  - Strong separation between on-chain identifiers/hashes and off-chain PHI.
  - Predictable, explicit storage patterns across contracts.
  - Easier reasoning for indexers (keys follow consistent schemes).
  - Error and monitoring infrastructure can rely on instance-local logs.
- **Negative / Trade-offs**:
  - Cold data might require off-chain archives for long-term retention.
  - No generic on-chain key iteration—indexers must track keys externally.
  - Additional code complexity for TTL management and helper functions.

## Implementation Notes

- The storage design is reflected in:
  - `contracts/vision_records/src/lib.rs`, `rbac.rs`, `provider.rs`, `errors.rs`
  - `contracts/staking/src/lib.rs`
  - `contracts/cross_chain/src/lib.rs`
  - `contracts/analytics/src/lib.rs`
  - `contracts/treasury/src/lib.rs`
- Retention and archival strategies are further explored in:
  - `contracts/compliance/src/retention.rs`
  - `docs/data-portability.md`
  - `docs/monitoring.md`

