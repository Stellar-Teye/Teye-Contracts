# Emergency Contacts

This document lists the key personnel and escalation paths for security incidents affecting the Teye-Contracts platform. Keep this file up to date — review it at least quarterly.

> **Confidentiality**: This file may contain private contact details. If the repository is public, consider storing sensitive values in a private secrets manager and linking to them here instead.

---

## On-Call Rotation

| Week Starting | Primary On-Call | Secondary On-Call |
| ------------- | --------------- | ----------------- |
| YYYY-MM-DD    | @maintainer-1   | @maintainer-2     |
| YYYY-MM-DD    | @maintainer-2   | @maintainer-3     |
| YYYY-MM-DD    | @maintainer-3   | @maintainer-1     |

Update this rotation schedule before the start of each quarter.

---

## Core Maintainers

| Name | GitHub Handle | Role | Email | Signal / Telegram | Timezone |
| ---- | ------------- | ---- | ----- | ----------------- | -------- |
| _TBD_ | @handle | Incident Commander | name@example.com | @signal_handle | UTC+0 |
| _TBD_ | @handle | Contract Engineer | name@example.com | @signal_handle | UTC+0 |
| _TBD_ | @handle | Security Analyst | name@example.com | @signal_handle | UTC+0 |
| _TBD_ | @handle | Communications Lead | name@example.com | @signal_handle | UTC+0 |

---

## SystemAdmin Addresses

These are the on-chain addresses authorized to execute emergency contract actions (pause, revoke, upgrade). See the [Incident Response Plan](incident-response-plan.md) for when and how to use them.

| Label | Stellar Address | Custodian | Notes |
| ----- | --------------- | --------- | ----- |
| Primary Admin | G...  | @maintainer-1 | Main operational key |
| Backup Admin  | G...  | @maintainer-2 | Cold storage; use only if primary is compromised |
| Emergency Multi-sig | G... | Requires 2-of-3 | For critical upgrades and key rotations |

> **Key Rotation**: If any SystemAdmin key is suspected compromised, follow the containment steps in the [Incident Response Plan § Phase 2](incident-response-plan.md#phase-2--containment).

---

## Escalation Path

Follow this order when escalating a confirmed incident:

1. **Primary On-Call Maintainer** — initial triage and severity classification.
2. **Secondary On-Call Maintainer** — if primary is unreachable within 15 minutes.
3. **Incident Commander** — takes ownership for P0/P1 incidents.
4. **All Core Maintainers** — notified for any P0 incident.
5. **External Security Contact** (if applicable) — third-party audit firm or bug-bounty platform.

---

## External Contacts

| Organization | Contact | Purpose | Notes |
| ------------ | ------- | ------- | ----- |
| Audit Firm | _TBD_ | Post-incident audit, vulnerability validation | Retainer / on-demand |
| Stellar Foundation | _TBD_ | Network-level incident coordination | For incidents affecting the Stellar network |
| Bug Bounty Platform | _TBD_ | Researcher coordination, disclosure management | _Link to program_ |
| Legal Counsel | _TBD_ | Regulatory reporting, breach notification | Engage for P0 incidents involving user data |

---

## Communication Channels

| Channel | Purpose | Access |
| ------- | ------- | ------ |
| Private Slack/Discord `#incident-response` | Real-time coordination during incidents | Core maintainers only |
| GitHub Security Advisories | Confidential vulnerability tracking | Repository admins |
| Email distribution list: `security@teye.example` | External reports and formal communication | Routed to on-call maintainer |
| Status page: `status.teye.example` | Public incident status updates | Communications Lead publishes updates |

---

## How to Update This Document

1. Open a PR against `master` modifying this file.
2. Have at least one other core maintainer review and approve.
3. After merging, notify the team in the `#incident-response` channel.
4. **Do not** include raw private keys, seed phrases, or passwords in this file.

---

## Related Documents

- [Incident Response Plan](incident-response-plan.md) — Step-by-step runbook for handling security incidents.
- [Emergency Access Protocol](emergency-protocol.md) — On-chain emergency access mechanism and `EmergencyAccess` types.
- [Security Scanning](security.md) — CI-based security scanning and vulnerability management.
- [Contract Upgrade Procedures](upgrade_procedures.md) — How to deploy patched contracts.
