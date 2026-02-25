# ADR-0006: Governance and Staking Architecture

- **Status**: Accepted
- **Date**: 2026-02-25

## Context

The protocol includes governance, staking, and treasury functions that affect protocol upgrades and economic security. We need a model that supports transparent proposals, delayed execution, and strong protection against rushed or malicious changes. The system must also align incentives for long-term participation while keeping operational control auditable and reversible.

## Decision

- Use a governor + timelock pattern for proposal lifecycle control.
- Require a 4% quorum for proposal validity.
- Support delegation to enable participation without custody transfer.
- Use staking for voting weight and governance alignment.
- Route funded actions through a treasury contract controlled by governance.
- Enforce a timelock delay before execution to enable review and veto windows.

## Rationale

- Alternatives like direct admin control or multisig-only governance centralize power and reduce transparency.
- A governor + timelock pattern is widely studied and provides clear checkpoints.
- Delegation improves participation without forcing active voters to custody keys.
- A treasury contract isolates funds from execution logic and reduces blast radius.

## Consequences

- Positive: stronger upgrade safety through delayed execution and quorum rules.
- Positive: clear, auditable proposal lifecycle with on-chain records.
- Negative: higher latency to implement urgent changes due to timelock delays.
- Negative: governance capture risk if voting power concentrates.
- Negative: additional operational overhead for proposal creation and monitoring.

## References

- contracts/governor/
- contracts/staking/
- contracts/treasury/
- docs/governance.md
- docs/staking.md
- docs/treasury.md

## Implementation Notes

- The governor defines proposal states (created, active, queued, executed, canceled).
- The timelock is the only executor for governance actions.
- The treasury accepts execution only from the timelock.
