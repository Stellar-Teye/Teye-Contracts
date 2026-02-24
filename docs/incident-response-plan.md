# Security Incident Response Plan

This runbook defines the step-by-step procedures for detecting, containing, eradicating, and recovering from security incidents affecting the Teye-Contracts platform. It should be read alongside the [Emergency Access Protocol](emergency-protocol.md), [Security Scanning](security.md), and [Contract Upgrade Procedures](upgrade_procedures.md) documentation.

---

## Table of Contents

1. [Scope and Definitions](#scope-and-definitions)
2. [Roles and Responsibilities](#roles-and-responsibilities)
3. [Severity Classification](#severity-classification)
4. [Phase 1 — Detection and Triage](#phase-1--detection-and-triage)
5. [Phase 2 — Containment](#phase-2--containment)
6. [Phase 3 — Eradication](#phase-3--eradication)
7. [Phase 4 — Recovery](#phase-4--recovery)
8. [Phase 5 — Post-Incident Review](#phase-5--post-incident-review)
9. [Contract-Specific Actions](#contract-specific-actions)
10. [Communication Plan](#communication-plan)
11. [Post-Incident Report Template](#post-incident-report-template)

---

## Scope and Definitions

| Term | Definition |
|---|---|
| **Incident** | Any event that compromises the confidentiality, integrity, or availability of Teye smart contracts or user data. |
| **Responder** | A team member with permissions to execute containment actions (pause, revoke, upgrade). |
| **Incident Commander (IC)** | The designated lead for a given incident; coordinates all response phases. |
| **SystemAdmin** | An on-chain address authorized to invoke administrative contract functions (pause, revoke emergency access, upgrade). |

---

## Roles and Responsibilities

| Role | Responsibilities |
|---|---|
| **Incident Commander** | Coordinates response, makes escalation decisions, owns communication. |
| **Contract Engineer** | Executes on-chain actions: pause contracts, revoke access, deploy patches. |
| **Security Analyst** | Investigates root cause, reviews audit logs and on-chain events. |
| **Communications Lead** | Drafts user-facing notices, coordinates disclosure timeline. |
| **On-Call Maintainer** | First responder; performs initial triage and escalates if needed. |

Refer to [docs/emergency-contacts.md](emergency-contacts.md) for the current contact list and escalation path.

---

## Severity Classification

| Severity | Description | Response Time |
|---|---|---|
| **P0 — Critical** | Active exploit draining funds or leaking protected health data. | Immediate (< 15 min to triage) |
| **P1 — High** | Confirmed vulnerability with a known exploit path but no evidence of active exploitation. | < 1 hour |
| **P2 — Medium** | Vulnerability identified with no known exploit path, or a non-critical contract malfunction. | < 4 hours |
| **P3 — Low** | Minor issue (e.g., informational finding from audit, non-sensitive logging gap). | < 24 hours |

---

## Phase 1 — Detection and Triage

### 1.1 Detection Sources

- **CI Security Scanning**: `cargo-audit`, `clippy` security lints, `gitleaks` (see [security.md](security.md)).
- **On-Chain Monitoring**: Unexpected `EmergencyAccessGrantedEvent`, `EMRG_REV`, or `EMRG_USE` events.
- **External Reports**: Responsible disclosure submissions (see [security.md § Incident Response](security.md#incident-response-and-responsible-disclosure)).
- **Dependency Alerts**: Dependabot / RustSec advisory notifications.
- **Internal Testing**: Findings from fuzzing, property testing, or manual review.

### 1.2 Triage Checklist

1. **Confirm the report** — reproduce or verify the issue independently.
2. **Classify severity** using the table above.
3. **Assign an Incident Commander** from the on-call roster.
4. **Open a private incident channel** (e.g., private Slack/Discord channel, or a confidential GitHub Security Advisory).
5. **Log the incident** with a unique identifier: `INC-YYYY-NNN`.

---

## Phase 2 — Containment

The goal is to limit damage without destroying forensic evidence.

### 2.1 Immediate Containment (P0/P1)

1. **Pause affected contracts** — invoke the emergency pause mechanism via a `SystemAdmin` address. See [Contract-Specific Actions](#contract-specific-actions) below.
2. **Revoke compromised access** — if emergency access grants are involved, call `revoke_emergency_access` for every suspicious `EmergencyAccess.id`. The `EmergencyAccess` struct (defined in `contracts/vision_records/src/emergency.rs`) tracks:
   - `id`, `patient`, `requester`, `condition`, `attestation`
   - `granted_at`, `expires_at`, `status` (`Active`, `Expired`, `Revoked`)
   - `notified_contacts`
3. **Rotate compromised keys** — if a `SystemAdmin` or deployer key is suspected compromised, rotate immediately and update the on-chain admin list.
4. **Snapshot on-chain state** — capture current ledger state for forensic analysis.

### 2.2 Short-Term Containment

1. **Disable CI deployments** — prevent any new contract deployments until the all-clear.
2. **Lock the affected branch** — restrict push access on `master` / release branches.
3. **Notify the core team** via the escalation path in [emergency-contacts.md](emergency-contacts.md).

---

## Phase 3 — Eradication

### 3.1 Root Cause Analysis

1. Review the on-chain **audit trail** using `get_audit_entries(access_id)` for each affected `EmergencyAccess` grant.
2. Correlate with off-chain CI logs (`cargo-audit` results, `gitleaks` output).
3. Trace the attack vector through code review — identify the vulnerable code path.
4. Document the root cause in the incident log.

### 3.2 Patch Development

1. Create a fix on a private branch (or GitHub Security Advisory fork).
2. Ensure the patch passes all existing tests plus a new regression test for the vulnerability.
3. Run the full security scanning suite locally:
   ```bash
   cargo clippy --all-targets --all-features \
     -- -D warnings \
     -W clippy::unwrap_used \
     -W clippy::expect_used \
     -W clippy::panic \
     -W clippy::arithmetic_side_effects

   cargo audit
   gitleaks detect --source . --verbose --redact
   ```
4. Peer-review the patch with at least two maintainers.

---

## Phase 4 — Recovery

### 4.1 Contract Recovery

1. **Deploy the patched contract** following the [Contract Upgrade Procedures](upgrade_procedures.md):
   - Deploy new version.
   - Run migration logic.
   - Validate state correctness.
   - Enable new features / unpause.
2. **Unpause contracts** — only after the patch is verified on a testnet deploy.
3. **Expire stale emergency accesses** — run `expire_emergency_accesses` to clean up any accesses that passed their expiration during the incident.
4. **Re-enable CI deployments**.

### 4.2 Verification

1. Confirm normal contract operation on testnet, then mainnet.
2. Verify that the vulnerability is no longer exploitable.
3. Monitor on-chain events for 48 hours for any recurrence.

---

## Phase 5 — Post-Incident Review

Hold a blameless post-incident review within **72 hours** of resolution.

### 5.1 Review Agenda

1. Timeline reconstruction — what happened and when.
2. What went well — effective detection, fast containment, etc.
3. What could be improved — gaps in monitoring, slow escalation, etc.
4. Action items — concrete tasks with owners and deadlines.

### 5.2 Deliverable

Produce a **Post-Incident Report** using the template at the end of this document. Store completed reports in `docs/post-incident-reports/`.

---

## Contract-Specific Actions

These are the key on-chain actions available during an incident. All require a `SystemAdmin` address.

| Action | Function / Mechanism | When to Use |
|---|---|---|
| **Pause contract** | Emergency pause mechanism (see `emergency.rs`) | Active exploit or unpatched critical vulnerability. |
| **Revoke emergency access** | `revoke_emergency_access(env, access_id)` | Compromised or suspicious `EmergencyAccess` grant. |
| **Expire accesses** | `expire_emergency_accesses(env)` | Bulk cleanup of stale grants after an incident. |
| **Upgrade contract** | Deploy new WASM → migrate state (see [upgrade_procedures.md](upgrade_procedures.md)) | Deploying a security patch. |
| **Revoke admin key** | Update admin list on-chain | Compromised `SystemAdmin` key. |

### EmergencyAccess Reference

The `EmergencyAccess` struct in `contracts/vision_records/src/emergency.rs` is central to access control during incidents:

```rust
pub struct EmergencyAccess {
    pub id: u64,
    pub patient: Address,
    pub requester: Address,
    pub condition: EmergencyCondition,   // LifeThreatening | Unconscious | SurgicalEmergency | Masscasualties
    pub attestation: String,
    pub granted_at: u64,
    pub expires_at: u64,
    pub status: EmergencyStatus,         // Active | Expired | Revoked
    pub notified_contacts: Vec<Address>,
}
```

Key storage functions:
- `set_emergency_access` — stores a grant and indexes by patient.
- `get_emergency_access` — retrieves a grant by ID.
- `has_active_emergency_access` — checks for an active grant between a patient-requester pair.
- `revoke_emergency_access` — sets status to `Revoked`.
- `add_audit_entry` / `get_audit_entries` — immutable audit log per access ID.

---

## Communication Plan

### Internal Communication

| Audience | Channel | Timing |
|---|---|---|
| Core team | Private incident channel | Immediately on detection |
| All maintainers | Email / group chat | Within 1 hour of confirmed P0/P1 |
| Contributors | Private advisory | After containment |

### External Communication

| Audience | Channel | Timing |
|---|---|---|
| Affected users | Status page / in-app notification | After containment, with known impact |
| General public | Blog post / GitHub Advisory | After patch is deployed and verified |
| Security researchers | CVE / advisory update | After public disclosure |

### Communication Principles

- **Be transparent** — share what is known and what is still under investigation.
- **Do not speculate** — only communicate confirmed facts.
- **Provide actionable guidance** — tell users what they should do (e.g., revoke sessions, check records).
- **Set expectations** — provide a timeline for the next update.

---

## Post-Incident Report Template

```markdown
# Post-Incident Report: INC-YYYY-NNN

## Summary
<!-- One-paragraph summary of the incident -->

## Severity
<!-- P0 / P1 / P2 / P3 -->

## Timeline
| Time (UTC) | Event |
|---|---|
| YYYY-MM-DD HH:MM | Incident detected via [source] |
| YYYY-MM-DD HH:MM | Incident Commander assigned: [name] |
| YYYY-MM-DD HH:MM | Containment action taken: [action] |
| YYYY-MM-DD HH:MM | Root cause identified |
| YYYY-MM-DD HH:MM | Patch deployed |
| YYYY-MM-DD HH:MM | Incident resolved |

## Root Cause
<!-- Detailed description of the root cause -->

## Impact
- **Users affected**: [number / scope]
- **Data exposed**: [description or "None"]
- **Funds at risk**: [amount or "None"]
- **Contracts affected**: [list]

## Containment Actions Taken
- [ ] Contracts paused
- [ ] Emergency access grants revoked
- [ ] Admin keys rotated
- [ ] CI deployments disabled

## Resolution
<!-- Description of the fix and how it was deployed -->

## Lessons Learned
### What went well
<!-- List -->

### What could be improved
<!-- List -->

## Action Items
| Action | Owner | Deadline | Status |
|---|---|---|---|
| [action] | [owner] | [date] | Open / Done |
```
