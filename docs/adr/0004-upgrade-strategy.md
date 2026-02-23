# ADR-0004: Smart Contract Upgrade Strategy

- **Status**: Accepted
- **Date**: 2026-02-23

## Context

Soroban contracts are immutable once deployed. At the same time, Stellar Teye:

- Evolves over time (new features, bug fixes, performance improvements).
- Must preserve long-lived patient data and references across upgrades.
- Needs a predictable way to migrate or route to new contract versions.

## Decision

- Use an **"upgrade by indirection"** strategy:
  - New functionality is shipped as **new contract deployments** (new WASM, new contract IDs).
  - Off-chain clients and orchestration services decide which contract ID to use.
- Maintain **versioned contracts**:
  - Core contracts (e.g., `vision_records`, `staking`, `analytics`, `treasury`) expose a simple `version()` method where appropriate.
  - New versions may extend functionality or change internal schemas while keeping:
    - External APIs as stable as possible.
    - Backwards compatibility for critical flows.
- Separate **governance and routing** from contract logic:
  - The on-chain governor/timelock (EVM-side today, Soroban-native later) controls:
    - When new contracts are deployed.
    - When environment variables, config contracts, or off-chain services are updated to point to new deployments.
- For **data migration**:
  - Prefer leaving historical data in-place and:
    - Indexing both old and new contract instances off-chain.
    - Migrating only necessary state (e.g., active access grants) using explicit migration scripts.

## Rationale

- Direct in-place upgrading is not supported for Soroban Wasm contracts.
- Indirection and versioning:
  - Align with how other ecosystems handle contract upgrades (e.g., proxy patterns without on-chain logic indirection).
  - Keep contract code simpler and more auditable.
- Off-chain clients are already required for:
  - Key management
  - Data storage
  - Analytics and monitoring
  - This makes them natural places to handle contract routing and version selection.

## Consequences

- **Positive**:
  - Clear separation between immutable contract code and mutable configuration.
  - Safer rollout of new versions (can deploy, shadow-test, and switch over).
  - Historical contract instances remain intact for audit and forensic analysis.
- **Negative / Trade-offs**:
  - Clients and services must handle multiple contract IDs and versions.
  - Migration of complex state may require explicit scripts and coordination.

## Implementation Notes

- Versioning:
  - `contracts/vision_records/src/lib.rs` exposes `version()` for consumers.
  - Similar patterns can be adopted in new contracts (`analytics`, `treasury`) as needed.
- Release and deployment:
  - Automated release artifacts are produced by:
    - `.github/workflows/release.yml`
    - `scripts/build_release_artifacts.sh`
  - Testnet deployments are orchestrated by:
    - `.github/workflows/deploy-testnet.yml`
    - `scripts/deploy.sh`, `scripts/deploy_testnet.sh`
- Governance:
  - EVM-side governance is documented in `docs/governance.md`.
  - Future Soroban-native governance contracts are expected to manage:
    - Mapping from logical service names to contract IDs.
    - Timelocked upgrades and configuration changes.

