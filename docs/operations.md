# Operations Guide

This runbook covers day-2 operations for deployed Teye contracts.

## 1. Monitoring Setup

The repository includes a Prometheus + Alertmanager + Grafana stack in `scripts/monitor/`.

### 1.1 Start monitoring stack

```bash
cd scripts/monitor
docker-compose up -d
```

Endpoints:

- Grafana: `http://localhost:3000` (default `admin/admin`)
- Prometheus: `http://localhost:9090`
- Alertmanager: `http://localhost:9093`

### 1.2 Configure contract health source

1. Set environment for the health checker:

```bash
export RPC_URL="https://rpc-futurenet.stellar.org"
export CONTRACT_ID="<deployed-contract-id>"
```

2. Run health probe:

```bash
./scripts/monitor/health_check.sh
```

3. Ensure your metrics endpoint/exporter is reachable by Prometheus at the target in `scripts/monitor/prometheus/prometheus.yml`.

### 1.3 Alert policy basics

Configured alerts (`scripts/monitor/prometheus/alerts.yml`):

- `RPCDown` (critical)
- `HighErrorRate` (warning)
- `SlowRPCResponse` (warning)

Recommended response targets:

- Critical: acknowledge within 5 minutes
- Warning: investigate within 30 minutes

## 2. Backup Procedures

## 2.1 What to back up

1. Deployment descriptors in `deployments/`
2. Release artifacts and checksums in `dist/`
3. Monitoring config under `scripts/monitor/`
4. Environment/config values used by integrators (contract IDs, network aliases)

## 2.2 Backup cadence

- After every deployment
- Daily snapshot of `deployments/` and `dist/` for active networks
- Before and after each upgrade/migration event

## 2.3 Backup commands

Create deterministic build artifacts:

```bash
./scripts/build_release_artifacts.sh
```

Archive deployment metadata:

```bash
tar -czf backups/deployments-$(date +%Y%m%d-%H%M%S).tgz deployments/
```

Archive release artifacts:

```bash
tar -czf backups/dist-$(date +%Y%m%d-%H%M%S).tgz dist/
```

## 2.4 Restore process

1. Restore descriptor backup to a staging path
2. Validate contract IDs and checksums
3. Repoint applications/indexers to restored contract ID
4. Run smoke checks (`version`, read-only method invocations)

## 3. Troubleshooting Guide

## 3.1 Deployment failures

Symptoms:

- `WASM file not found`
- `DEPLOYMENT_CONTRACT_ID` missing
- contract invoke verification fails

Actions:

1. Confirm build output exists in `target/wasm32-unknown-unknown/release/`
2. Validate Soroban network and key config (`soroban network ls`, `soroban keys ls`)
3. Re-run deployment with shell tracing:

```bash
bash -x ./scripts/deploy.sh testnet vision_records
```

## 3.2 Elevated authorization failures

Symptoms:

- Spikes in unauthorized tx failures
- New `AccessViolationEvent` volume increases

Actions:

1. Identify `caller`, `action`, and `required_permission` from emitted violation events
2. Correlate with release time and caller source
3. Verify role/admin-tier assignments and whitelist state
4. If malicious pattern is confirmed, rotate admin keys and enforce stricter allowlists

## 3.3 RPC health incidents

Symptoms:

- `RPCDown` alert firing
- timeouts in client flows

Actions:

1. Run `./scripts/monitor/health_check.sh`
2. Check RPC availability and latency from your environment
3. Fail over to alternate RPC endpoint if configured
4. Reprocess failed transactions after RPC recovers

## 4. Performance Tuning Tips

## 4.1 Contract and transaction patterns

1. Prefer batch APIs where available (for example, batch record/access updates)
2. Avoid unnecessary write calls; use read-only checks before mutating
3. Keep request payloads bounded to reduce simulation and execution overhead

## 4.2 RPC and client tuning

1. Use `simulateTransaction` before submission to size resource budgets
2. Use request retry with exponential backoff for transient RPC errors
3. Avoid concurrent submits from one source account without sequence coordination

## 4.3 Monitoring-driven optimization

1. Track p95/p99 invocation latency
2. Track per-method error rates and failure causes
3. Alert on abrupt changes in `AccessViolationEvent` frequency

## 5. Upgrade Procedures

## 5.1 Pre-upgrade

1. Freeze non-essential writes for the target contract
2. Run full test suite and upgrade tests under `test/upgrade/`
3. Build artifacts and capture checksums:

```bash
./scripts/build_release_artifacts.sh
```

4. Confirm rollback descriptor and previous contract ID are available

## 5.2 Execute upgrade rollout

1. Deploy new contract version to staging/testnet
2. Verify read/write compatibility and auth behavior
3. Run canary traffic against the new contract ID
4. Promote to production and update consumer configs atomically

If state migration logic is involved, validate it with the simulation scaffold in `scripts/simulate_upgrade.rs` and migration tests before production rollout.

## 5.3 Post-upgrade verification

1. Validate core read/write flows
2. Validate authorization enforcement and emitted violation events
3. Monitor error rate/latency for at least one full observation window
4. Publish release notes with version, contract IDs, and checksum references

## 5.4 Rollback decision triggers

Rollback immediately when any of the following are true:

1. sustained critical alerts (`RPCDown` or equivalent service outage)
2. authorization bypass or unexpected permission grants
3. persistent elevated error rates after the normal warm-up window
4. data consistency or migration correctness failures

Rollback action: repoint clients/indexers to the previous known-good contract ID using deployment backup metadata.
