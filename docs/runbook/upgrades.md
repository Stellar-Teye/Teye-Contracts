# Contract Upgrade Playbook

This playbook extends `docs/upgrade_procedures.md` with executable upgrade and migration workflows.

## 1. Upgrade Readiness Checklist

- [ ] Pre-deployment checklist completed (`pre-deployment.md`)
- [ ] Previous known-good contract IDs documented
- [ ] `./scripts/build_release_artifacts.sh` completed
- [ ] Migration version target approved
- [ ] Rollback owner assigned

## 2. State Migration Simulation

Use the included simulation scaffold before on-chain migration.

1. Run local simulation:

```bash
rustc scripts/simulate_upgrade.rs -o /tmp/simulate_upgrade && /tmp/simulate_upgrade
```

2. Confirm output includes:
- `Old State:`
- `New State:`
- `Upgrade simulation complete.`

3. Validate assumptions against real migration logic/tests in `test/upgrade/`.

## 3. Canonical Upgrade Workflow

1. Build candidate artifacts.
2. Deploy candidate to testnet via rollback wrapper:
   - `./scripts/deploy_with_rollback.sh testnet <contract> --admin <ADMIN_ADDRESS>`
3. Run migration dry run:
   - `TEYE_CONTRACT_ID=<id> ./scripts/migrate.sh dry-run --to <version> --network testnet`
4. Execute forward migration after approval:
   - `TEYE_CONTRACT_ID=<id> ./scripts/migrate.sh forward --to <version> --network testnet`
5. Validate on-chain version:
   - `TEYE_CONTRACT_ID=<id> ./scripts/migrate.sh status --network testnet`
6. Run smoke/regression tests against migrated state.

## 4. Multi-Contract Coordinated Upgrade Sequence

For coupled contracts, use this sequence:

1. Core data/auth contracts: `identity`, `events`, `vision_records`
2. Interop/compliance: `fhir`, `emr_bridge`, `cross_chain`
3. Governance/economic controls: `staking`, `treasury`, `governor`
4. ZK and analytics: `zk_verifier`, `zk_voting`, `analytics`, `ai_integration`, `metering`

Rules:
- Do not advance to next stage until current stage migration and smoke checks pass.
- Keep old and new contract ID mappings for each stage for rollback.

## 5. Canary Strategy

1. Deploy to testnet and validate all migration checks.
2. Enable canary routing in migrated contract:

```bash
TEYE_CONTRACT_ID=<id> ./scripts/migrate.sh canary --pct 10 --version <version>
```

3. Observe metrics for one full monitoring window.
4. Increase canary only if no critical/warning regression.
5. Disable canary quickly if needed:

```bash
TEYE_CONTRACT_ID=<id> ./scripts/migrate.sh canary-off
```

## 6. Mainnet Promotion

1. Rebuild artifacts and verify hashes.
2. Deploy mainnet via rollback wrapper.
3. Execute migration dry-run and forward migration.
4. Verify schema version and post-upgrade smoke tests.
5. Publish release notes and updated contract IDs.

## 7. Emergency Upgrade Procedure (Critical Security Scenario)

Use when waiting full governance/timelock path is unacceptable due to active exploitation risk.

1. Incident Commander declares emergency and logs reason.
2. Pause affected contracts (`pause_contract` or equivalent admin controls).
3. Use emergency admin/multisig authority to deploy patched contract immediately.
4. Run minimum safe migration path:
- `dry-run`
- `forward`
- core smoke tests
5. Re-enable operations in controlled phases.
6. Within 24h, publish post-emergency governance/legal documentation and full incident report.

Note:
- If protocol policy requires strict timelock bypass approvals, obtain signer quorum first and archive signed approvals in incident artifacts.

## 8. Rollback Playbook for Upgrades

1. Trigger conditions:
- migration validation failure
- critical functional regression
- sustained critical alerts after upgrade
2. Roll back state version:

```bash
TEYE_CONTRACT_ID=<id> ./scripts/migrate.sh rollback --to <previous_version> --network <network>
```

3. Repoint integrations to previous known-good contract IDs if contract replacement was also part of rollout.
4. Run smoke checks and keep incident channel open until stable.
