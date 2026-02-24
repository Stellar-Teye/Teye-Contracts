# Deployment to Stellar Networks

This document describes how Teye-Contracts are deployed to Stellar networks, with a focus on automated **testnet** deployments.

## Overview

Contracts can be deployed to:

- **Local**: for development and integration testing
- **Testnet**: for public testing and staging
- **Futurenet/Mainnet**: for advanced testing and production (planned)

Deployment is powered by:

- Shell scripts in the `scripts/` directory
- GitHub Actions workflows in `.github/workflows/`

## Deployment Scripts

### `scripts/deploy.sh`

Generic deployment helper:

- **Usage**:

  ```bash
  ./scripts/deploy.sh <network> [contract]
  ```

  - `network`: one of `local`, `testnet`, `futurenet`, `mainnet`
  - `contract`: folder name under `contracts/` (defaults to `vision_records`)

- **Behavior**:
  - Builds the contract as a `wasm32-unknown-unknown` release binary
  - Deploys the contract using `soroban contract deploy`
  - Prints the new contract ID
  - Writes a deployment descriptor to `deployments/<network>_<contract>.json` containing:
    - Network and contract name
    - Deployed contract ID
    - Deployment timestamp
    - WASM SHA-256 hash

This script is safe to run locally and in CI.

### `scripts/deploy_testnet.sh`

Orchestrates **testnet** deployments with basic verification and rollback handling.

- **Usage**:

  ```bash
  ./scripts/deploy_testnet.sh [contract]
  ```

  - `contract`: folder name under `contracts/` (defaults to `vision_records`)

- **Behavior**:
  1. Backs up the existing testnet deployment descriptor (if any) to:

     ```text
     deployments/testnet_<contract>_previous.json
     ```

  2. Calls `scripts/deploy.sh testnet <contract>` and captures the `DEPLOYMENT_CONTRACT_ID` line.
  3. Runs a **lightweight verification** step:
     - For `vision_records`: invokes the `version` method via `soroban contract invoke`.
     - For unknown contracts: skips verification but still completes with success.
  4. If verification fails:
     - Prints an error
     - Restores the previous deployment descriptor (if present) so monitoring and tools continue to point at the last known-good deployment.
  5. On success, prints `VERIFIED_CONTRACT_ID=<id>` for CI consumption.

This flow ensures that broken deployments do not overwrite the “current” deployment descriptor.

## GitHub Actions: Testnet Deployment

Automated testnet deployment is handled by:

- `.github/workflows/deploy-testnet.yml`

### Triggers

- On push to `master` affecting:
  - `contracts/**`
  - `scripts/**`
  - `.github/workflows/deploy-testnet.yml`
- Manual trigger via **Run workflow** (workflow_dispatch), with an optional `contract` input.

### Workflow Outline

1. **Environment setup**
   - Install Rust `RUST_VERSION` and Soroban CLI `SOROBAN_VERSION`
   - Ensure Soroban `testnet` network is configured (idempotent)

2. **Deployment**
   - Runs:

     ```bash
     ./scripts/deploy_testnet.sh <contract>
     ```

   - Parses `VERIFIED_CONTRACT_ID` from the script output and exposes:
     - `steps.deploy.outputs.contract_name`
     - `steps.deploy.outputs.contract_id`

3. **Deployment report**
   - Writes a human-readable summary to the GitHub Actions **Step Summary**, including:
     - Contract name
     - Network (`testnet`)
     - Deployed contract ID
     - Trigger type (push or manual)

4. **Optional notifications**
   - If `SLACK_WEBHOOK_URL` is configured as a repository secret, posts a message containing:
     - Contract name and network
     - Contract ID
     - Link to the GitHub Actions run

## Rollback Procedures

Because Soroban contracts are immutable after deployment, rollback is handled by switching which contract ID is considered “current” in tooling and clients:

1. Inspect the backup deployment descriptor:

   ```bash
   cat deployments/testnet_<contract>_previous.json
   ```

2. Update any off-chain services, frontends, or environment variables to point back to the previous `contract_id`.

3. Optionally, tag the problematic deployment in Git or document it in an ADR for future reference.

The testnet workflow never deletes old contracts on-chain; it only updates which contract ID is surfaced as the canonical deployment.

## Best Practices

- Always ensure CI (`ci.yml` and `security.yml`) is green before triggering testnet deployment.
- Prefer deploying from `master` only after a successful release build.
- Use manual `workflow_dispatch` runs with a specific `contract` when testing new or experimental contracts.
- Keep `deployments/` committed so that deployment history is visible in Git (unless project policy dictates otherwise).

