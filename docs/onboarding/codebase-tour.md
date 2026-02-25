# Codebase Orientation Tour

This is a guided walkthrough of the repository structure and how the contract suite fits together.

## Root-Level Files

- `Cargo.toml` - Rust workspace definition and shared dependencies.
- `Makefile` - Common build, test, and deployment commands.
- `deny.toml` - Dependency policy rules for `cargo deny`.
- `.coderabbit.yaml` - Code review automation configuration.
- `.pre-commit-config.yaml` - Pre-commit hooks configuration.
- `setup.sh` - Automated environment setup.

## Contracts Workspace

All contract crates live in `contracts/` and are part of the workspace. Each contract is a separate Rust crate with its own `Cargo.toml`, and they share utilities from the common library.

### Contract Suite (19 total)

- `contracts/ai_integration` - AI-assisted vision analysis and anomaly handling.
- `contracts/analytics` - Aggregated metrics and reporting for vision data.
- `contracts/audit` - Audit trail and access logging.
- `contracts/common` - Shared utilities, error types, and security helpers. See the shared library overview in [contracts/common/src/lib.rs](../../contracts/common/src/lib.rs).
- `contracts/compliance` - Compliance checks and enforcement for policy rules.
- `contracts/cross_chain` - Cross-chain relay and data portability helpers.
- `contracts/emr_bridge` - EMR integration and data interoperability.
- `contracts/events` - Event streaming and contract telemetry.
- `contracts/fhir` - FHIR-compatible data transforms.
- `contracts/governor` - Governance proposals and voting.
- `contracts/identity` - Identity and verification primitives.
- `contracts/key_manager` - Key derivation and key rotation.
- `contracts/metering` - Usage metering and quotas.
- `contracts/staking` - Staking and incentives.
- `contracts/timelock` - Timelock execution for governance actions.
- `contracts/treasury` - Treasury controls and funds management.
- `contracts/vision_records` - Core vision record management contract.
- `contracts/zk_verifier` - ZK proof verification.
- `contracts/zk_voting` - ZK-enabled governance voting.

## Vision Records Contract Deep Dive

The main contract lives in `contracts/vision_records`.

### Module Structure

- `lib.rs` - Contract entry points, shared types, and storage keys.
- `appointment.rs` - Appointment scheduling and visit tracking.
- `audit.rs` - Access auditing and usage tracing.
- `circuit_breaker.rs` - Emergency pause and safety controls.
- `emergency.rs` - Emergency access and break-glass flows.
- `errors.rs` - Contract-specific error definitions.
- `events.rs` - Event emissions for observability.
- `examination.rs` - Vision examination structures and logic.
- `patient_profile.rs` - Patient profile storage and updates.
- `prescription.rs` - Prescription creation and update logic.
- `rbac.rs` - Role-based access control and policy evaluation.
- `types.rs` - Common types and enums (these are currently defined in `lib.rs` in this branch).
- `upgrade.rs` - Upgrade and migration helpers (see shared migration helpers in `contracts/common`).

There are supporting modules such as `provider.rs`, `rate_limit.rs`, `validation.rs`, and internal tests that complement the main flow.

### How the Modules Interconnect

- `lib.rs` provides the public contract API and wires in storage keys.
- `rbac.rs` enforces permissions across patient and provider operations.
- `audit.rs` and `events.rs` record access and emit trace events.
- `emergency.rs` and `circuit_breaker.rs` provide break-glass and pause controls.
- `examination.rs` and `prescription.rs` hold the domain model for care records.

### Storage Key Organization

Storage keys use the Soroban `symbol_short!` convention. See [docs/storage-key-conventions.md](../storage-key-conventions.md).

## Documentation

All documentation lives in `docs/`. Start with:

- [Architecture overview](../architecture.md)
- [API reference](../api.md)
- [Security model](../security.md)

## Scripts

Utility scripts live in `scripts/`:

- `deploy.sh` - Deployment helper for local, testnet, and mainnet.
- `check_storage_keys.sh` - Detects storage key collisions.
- `run_coverage.sh` - Coverage runs for CI.

## Fuzzing

Fuzz testing is in `fuzz/` and uses `cargo-fuzz`. This is how we stress test contract invariants.

## CI/CD

GitHub Actions workflows live in `.github/workflows/` and include CI, security scans, and deployment automation.
