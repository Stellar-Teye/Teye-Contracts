# Production Deployment & Operations Runbook

This runbook is the operational source of truth for production deployments, upgrades, monitoring, and incident response.

## Runbook Contents

1. [Pre-Deployment Checklist](pre-deployment.md)
2. [Deployment Procedures](deployment.md)
3. [Monitoring & Alerting Setup](monitoring.md)
4. [Contract Upgrade Playbook](upgrades.md)
5. [Incident Response Procedures](incident-response.md)

## Scope

This runbook consolidates operational procedures previously spread across:

- `docs/deployment.md`
- `docs/operations.md`
- `docs/monitoring.md`
- `docs/upgrade_procedures.md`
- `docs/incident-response-plan.md`
- `docs/emergency-protocol.md`

## Operating Principles

- Use least-privilege deployment (`--admin` with immediate handoff).
- Promote by environment (`local -> testnet -> futurenet -> mainnet`).
- Capture every deployment artifact (`deployments/`, `dist/SHA256SUMS.txt`).
- Never skip post-deploy smoke tests and alert validation.
- Keep rollback metadata current before every production action.

## Required Inputs Before Any Production Action

- Approved release version and change summary.
- On-call roster and incident commander assignment.
- Current known-good contract IDs for target environment.
- Confirmed access to admin and multisig signers.
