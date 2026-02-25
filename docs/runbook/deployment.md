# Deployment Procedures

This document is the step-by-step deployment playbook for `local`, `testnet`, `futurenet`, and `mainnet`.

## 1. Deployment Order and Dependency Validation

### Canonical Deployment Sequence

`common` is a Rust library crate and is **not deployed**.

Deploy contracts in this order:

1. `identity` -> `events` -> `vision_records`
2. `compliance` -> `fhir` -> `emr_bridge`
3. `staking` -> `governor` -> `timelock` -> `treasury`
4. `analytics` -> `ai_integration`
5. `cross_chain` -> `zk_verifier` -> `zk_voting` -> `metering`

### Validation Against Actual Repository Dependencies

- `zk_voting` has a direct crate dependency on `zk_verifier` (`contracts/zk_voting/Cargo.toml`), so `zk_verifier` must be deployed/configured before enabling `zk_voting` voting flows.
- `identity` references `zk_verifier` for credential verification (`contracts/identity/Cargo.toml`), so configure verifier address before using zk credential paths.
- `governor` and `treasury` are runtime-linked by `treasury.set_governor(...)`; deploy both before activating DAO-controlled spend.
- `timelock` in this repository is represented by governance phase/timelock logic in `governor` plus `contracts/timelock/Timelock.sol` artifact. Use the runtime your environment requires.

## 2. Environment-Specific Procedure

Use this same flow per environment.

### Local

1. Build and deploy:
   - `./scripts/deploy.sh local vision_records --admin <ADMIN_ADDRESS>`
2. Expected output includes:
   - `Contract deployed: <CONTRACT_ID>`
   - `Admin transfer verified.`
   - `DEPLOYMENT_CONTRACT_ID=<CONTRACT_ID>`
3. Verify descriptor:
   - `cat deployments/local_vision_records.json`

### Testnet

1. Safe deploy with descriptor backup/restore:
   - `./scripts/deploy_with_rollback.sh testnet vision_records --admin <ADMIN_ADDRESS>`
2. Expected output includes:
   - `Created descriptor backup:` (if prior deployment exists)
   - `Deployment succeeded:`
   - `Verification succeeded:`
   - `DEPLOYMENT_CONTRACT_ID=<CONTRACT_ID>`

### Futurenet

1. Deploy with rollback wrapper:
   - `./scripts/deploy_with_rollback.sh futurenet vision_records --admin <ADMIN_ADDRESS>`
2. Expected output includes same markers as testnet.

### Mainnet

1. Final gate checks:
   - `cargo test --all`
   - `./scripts/build_release_artifacts.sh`
2. Deploy with rollback wrapper:
   - `./scripts/deploy_with_rollback.sh mainnet vision_records --admin <ADMIN_ADDRESS>`
3. Expected output includes:
   - `Admin transfer verified.`
   - `DEPLOYMENT_CONTRACT_ID=<CONTRACT_ID>`
4. Record descriptor and checksums in release ticket.

## 3. Contract Initialization Parameters

Use this table when initializing newly deployed contracts.

| Contract | Initialize Function | Required Parameters |
|---|---|---|
| `identity` | `initialize` | `owner` |
| `events` | `initialize` | `admin` |
| `vision_records` | `initialize` | `admin` |
| `emr_bridge` | `initialize` | `admin` |
| `staking` | `initialize` | `admin`, `stake_token`, `reward_token`, `reward_rate`, `lock_period` |
| `governor` | `initialize` | `admin`, `staking_contract`, `treasury_contract`, `total_vote_supply` |
| `treasury` | `initialize` | `admin`, `token`, `signers`, `threshold` |
| `analytics` | `initialize` | `admin`, `aggregator`, `pub_key`, `priv_key (optional)` |
| `ai_integration` | `initialize` | `admin`, `anomaly_threshold_bps` |
| `cross_chain` | `initialize` | `admin` |
| `zk_verifier` | `initialize` | `admin` |
| `zk_voting` | `initialize` | `admin`, `option_count` |
| `metering` | `initialize` | `admin` |

### Standard Initialization Command Template

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source default \
  --network <NETWORK> \
  -- \
  initialize \
  --admin <ADMIN_ADDRESS>
```

## 4. Post-Deployment Smoke Tests

Run after each deployed contract.

- [ ] Contract ID is reachable: invoke a lightweight read method
- [ ] Admin ownership is correct: `get_admin` returns permanent admin
- [ ] Descriptor JSON exists and has expected `contract_id`, `deployed_at`, `wasm_hash`
- [ ] Hash in descriptor matches built WASM hash
- [ ] Monitoring target updated to new contract ID

### Example Read Checks

```bash
soroban contract invoke --id <VISION_RECORDS_ID> --source default --network <NETWORK> -- --fn is_initialized
soroban contract invoke --id <AI_INTEGRATION_ID> --source default --network <NETWORK> -- --fn is_initialized
soroban contract invoke --id <TREASURY_ID> --source default --network <NETWORK> -- --fn get_config
```

## 5. Rollback Procedure (`scripts/deploy_with_rollback.sh`)

On Soroban, deployed bytecode is immutable; rollback means reverting the active contract ID and deployment descriptor to the previous known-good deployment.

1. Run deployment with rollback wrapper:
   - `./scripts/deploy_with_rollback.sh <network> <contract> --admin <ADMIN_ADDRESS>`
2. If verification fails, script will:
   - Restore `deployments/<network>_<contract>.json` from backup
   - Exit non-zero
3. If post-release runtime issues appear:
   - Restore prior descriptor backup manually
   - Repoint all consumers/indexers/frontend/backend to previous `contract_id`
   - Re-run smoke checks against previous contract ID

## 6. Deployment Script Reference

### `scripts/deploy.sh`

Purpose:
- Build, deploy, optionally initialize, optionally transfer admin, and write descriptor.

Usage:

```bash
./scripts/deploy.sh <network> [contract] [--admin <address>]
```

Parameters:
- `network`: `local|testnet|futurenet|mainnet`
- `contract`: defaults to `vision_records`
- `--admin`: optional permanent admin address (recommended)

Key environment assumptions:
- Soroban identity `default` exists and is funded for network
- `target/wasm32-unknown-unknown/release/<contract>.wasm` buildable

### `scripts/deploy_testnet.sh`

Purpose:
- Testnet-only orchestrated deployment with descriptor backup and verification.

Usage:

```bash
./scripts/deploy_testnet.sh [contract]
```

Behavior:
- Backs up `deployments/testnet_<contract>.json`
- Calls `scripts/deploy.sh testnet <contract>`
- Performs contract verification (currently contract-specific)
- Restores backup on failure

### `scripts/migrate.sh`

Purpose:
- Execute and validate schema migrations, rollback, and canary traffic controls.

Usage examples:

```bash
TEYE_CONTRACT_ID=C... ./scripts/migrate.sh status --network testnet
TEYE_CONTRACT_ID=C... ./scripts/migrate.sh dry-run --to 3 --network testnet
TEYE_CONTRACT_ID=C... ./scripts/migrate.sh forward --to 3 --network testnet
TEYE_CONTRACT_ID=C... ./scripts/migrate.sh rollback --to 2 --network testnet
TEYE_CONTRACT_ID=C... ./scripts/migrate.sh canary --pct 10 --version 3
```

Key environment variables:
- `TEYE_CONTRACT_ID` (required unless `--contract` is passed)
- `SOROBAN_RPC_URL` (optional override)

### `scripts/build_release_artifacts.sh`

Purpose:
- Build all contract WASM artifacts and generate `dist/SHA256SUMS.txt`.

Usage:

```bash
./scripts/build_release_artifacts.sh
```

Output:
- `dist/*.wasm`
- `dist/SHA256SUMS.txt`

### `scripts/generate_release_notes.sh`

Purpose:
- Generate release notes using changelog section + artifact checksums.

Usage:

```bash
./scripts/generate_release_notes.sh <version> <tag>
```

Example:

```bash
./scripts/generate_release_notes.sh 1.2.2 v1.2.2
```

### `scripts/prepare_release_version.sh`

Purpose:
- Update workspace version in root `Cargo.toml`.

Usage:

```bash
./scripts/prepare_release_version.sh <semver>
```

Example:

```bash
./scripts/prepare_release_version.sh 1.2.3
```

### `scripts/deploy_with_rollback.sh`

Purpose:
- Environment-agnostic safe deploy wrapper with descriptor backup/restore, output parsing, and post-deploy verification hooks.

Usage:

```bash
./scripts/deploy_with_rollback.sh <network> [contract] [--admin <address>]
```

Recommended production usage:

```bash
./scripts/deploy_with_rollback.sh mainnet vision_records --admin <PERMANENT_ADMIN>
```
