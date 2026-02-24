# Deployment Guide

This guide covers repeatable deployment of Teye contracts to local and Stellar networks.
It is aligned with the scripts in `scripts/` and the artifact flow in this repository.

## Scope

- Build contract WASM artifacts
- Deploy a selected contract to a target network
- Initialize/admin handoff flow
- Verify deployment health
- Roll back consumer references to a previous contract ID

## Prerequisites

1. Rust toolchain with `wasm32-unknown-unknown` target
2. Soroban CLI installed and authenticated (`soroban keys` and network configs)
3. Network alias configured in Soroban CLI (`local`, `testnet`, `futurenet`, or `mainnet`)
4. Deployer key available as `default` source account
5. Access to this repository and write permission for `deployments/`

## Deployment Artifacts

- Build output: `target/wasm32-unknown-unknown/release/<contract>.wasm`
- Deployment descriptor: `deployments/<network>_<contract>.json`
- Previous descriptor backup (testnet flow): `deployments/testnet_<contract>_previous.json`

## Standard Deployment Procedure

### 1. Pre-deployment checks

Run checks before deploying:

```bash
cargo test
cargo build --target wasm32-unknown-unknown --release
```

Optional: generate all release artifacts and checksums:

```bash
./scripts/build_release_artifacts.sh
```

### 2. Deploy a contract

Use the base deployment script:

```bash
./scripts/deploy.sh <network> [contract]
```

Examples:

```bash
./scripts/deploy.sh local vision_records
./scripts/deploy.sh testnet staking
```

On success, the script prints:

- `DEPLOYMENT_CONTRACT_ID=<id>`
- a deployment descriptor JSON under `deployments/`

### 3. Initialize contract state

Most contracts require explicit initialization after deployment. Example for `vision_records`:

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source default \
  --network <network> \
  -- \
  initialize \
  --admin <ADMIN_ADDRESS>
```

### 4. Admin handoff (if needed)

Contracts in this repository generally use a two-step admin transfer (`propose_admin`, `accept_admin`).

Step 1 (current admin):

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <CURRENT_ADMIN_SOURCE> \
  --network <network> \
  -- \
  propose_admin \
  --current_admin <CURRENT_ADMIN_ADDRESS> \
  --new_admin <NEW_ADMIN_ADDRESS>
```

Step 2 (new admin):

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <NEW_ADMIN_SOURCE> \
  --network <network> \
  -- \
  accept_admin \
  --new_admin <NEW_ADMIN_ADDRESS>
```

### 5. Post-deployment verification

Minimum required checks:

1. Contract responds to a lightweight read method (for `vision_records`, use `version`)
2. Admin address is correct (`get_admin` where available)
3. Deployment descriptor contains expected `contract_id`, timestamp, and `wasm_hash`

Example:

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source default \
  --network <network> \
  --fn version
```

## Testnet Orchestrated Deployment

Use the testnet wrapper for safer descriptor handling:

```bash
./scripts/deploy_testnet.sh [contract]
```

Behavior:

1. Backs up current descriptor to `_previous.json`
2. Runs `scripts/deploy.sh testnet <contract>`
3. Runs lightweight verification (`version` for `vision_records`)
4. Restores previous descriptor if verification fails
5. Emits `VERIFIED_CONTRACT_ID=<id>` on success

## Rollback Procedure

On Soroban, deployed contracts are immutable. Rollback means repointing off-chain consumers.

### Steps

1. Identify last known-good contract ID from:
   - `deployments/<network>_<contract>_previous.json`
   - release notes or run logs
2. Update all consumers to use that contract ID:
   - backend services
   - indexers
   - frontend/app config
3. Re-run smoke checks against the previous contract ID
4. Record incident and root cause in ops/release notes

## CI/CD Recommendations

1. Build/test gates before deployment
2. One environment promotion path (`local -> testnet -> production`)
3. Persist deployment descriptors in git history
4. Capture contract ID + WASM checksum in release notes
5. Use manual approval for production deployment jobs

## Operational Handoff Checklist

Before marking a deployment complete:

1. Contract ID published to all consuming systems
2. Monitoring updated to track the new contract ID
3. Alerts green for at least one observation window
4. Backup snapshot updated (descriptors, artifacts, checksums)
5. Upgrade path and rollback metadata documented
