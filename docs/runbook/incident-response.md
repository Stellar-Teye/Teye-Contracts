# Incident Response Procedures

This runbook extends `docs/incident-response-plan.md` with contract-level emergency execution steps.

## 1. Severity Classification Matrix

| Severity | Example Trigger | Initial SLA |
|---|---|---|
| P0 Critical | Active exploit, key compromise, confirmed PHI exposure, governance takeover | Triage < 15 min |
| P1 High | Confirmed vuln with viable exploit path, sustained auth bypass attempts | Triage < 1 hour |
| P2 Medium | Significant service degradation, partial monitoring blind spots | Triage < 4 hours |
| P3 Low | Minor security finding, low-risk misconfiguration | Triage < 24 hours |

## 2. Incident Workflow Checklist

1. Detection and triage
- [ ] Assign Incident Commander
- [ ] Create incident ID: `INC-YYYY-NNN`
- [ ] Open private incident channel

2. Containment
- [ ] Pause affected contract(s)
- [ ] Revoke emergency access grants
- [ ] Revoke/rotate compromised keys
- [ ] Snapshot evidence and deployment descriptors

3. Eradication
- [ ] Patch reviewed and tested
- [ ] Upgrade/migration runbook executed

4. Recovery
- [ ] Smoke tests pass
- [ ] Monitoring green for observation window

5. Post-incident review
- [ ] Timeline and root cause finalized within 72h
- [ ] Corrective actions assigned with owners/dates

## 3. Circuit Breaker Activation (Pause Contracts)

For contracts implementing pause controls (for example `vision_records`):

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <SYSTEM_ADMIN_IDENTITY> \
  --network <NETWORK> \
  -- \
  pause_contract \
  --admin <ADMIN_ADDRESS>
```

Recovery (only after patch validation):

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <SYSTEM_ADMIN_IDENTITY> \
  --network <NETWORK> \
  -- \
  resume_contract \
  --admin <ADMIN_ADDRESS>
```

## 4. Emergency Access Revocation (Bulk)

Use this for suspicious emergency grants from `docs/emergency-protocol.md` events (`EMRG_GRT`, `EMRG_USE`, `EMRG_REV`).

1. Identify suspicious `access_id` values from indexer/event logs.
2. Revoke each grant:

```bash
soroban contract invoke \
  --id <VISION_RECORDS_ID> \
  --source <SYSTEM_ADMIN_IDENTITY> \
  --network <NETWORK> \
  -- \
  revoke_emergency_access \
  --revoker <ADMIN_ADDRESS> \
  --access_id <ACCESS_ID>
```

3. Expire stale grants:

```bash
soroban contract invoke \
  --id <VISION_RECORDS_ID> \
  --source <SYSTEM_ADMIN_IDENTITY> \
  --network <NETWORK> \
  -- \
  expire_emergency_accesses
```

## 5. Compromised Key Response

1. Immediate containment
- [ ] Pause affected contracts
- [ ] Revoke compromised signer/admin privileges

2. Rotate admin authority
- [ ] Propose new admin:
  - `propose_admin` or contract equivalent
- [ ] Accept new admin from replacement key:
  - `accept_admin`
- [ ] Verify current admin:
  - `get_admin`

3. Governance/multisig controls
- [ ] Update multisig signer sets and thresholds (staking/treasury/governance paths)
- [ ] Remove compromised keys from all CI/CD secret stores

4. Re-enable operations only after key provenance is verified.

## 6. Data Breach Notification Procedure (HIPAA-Aware)

If protected health data may have been exposed:

1. Immediately involve legal/compliance lead.
2. Preserve forensic evidence and access logs.
3. Determine affected individuals and data scope.
4. Prepare required notifications.
5. Follow HIPAA breach notification timelines (without unreasonable delay and typically no later than 60 days from discovery, subject to legal guidance).
6. Document regulator and user communication timestamps.

## 7. Communication Templates

### Internal Alert Template

```text
[INCIDENT][INC-YYYY-NNN][P0/P1/P2/P3]
Detected: <UTC timestamp>
Impacted contracts: <list>
Current status: <triage/contained/recovering>
Action owner: <name>
Next update: <UTC timestamp>
```

### External Status Template

```text
We are investigating a security incident affecting <system/component>.
Impact: <known impact>
Mitigation in progress: <yes/no>
User action required now: <none / specific steps>
Next update by: <UTC timestamp>
```

### Regulator/Compliance Template (Draft)

```text
Incident ID: INC-YYYY-NNN
Discovery time (UTC): <timestamp>
Incident type: <unauthorized disclosure / integrity / availability>
Systems affected: <contracts/services>
Data potentially affected: <summary>
Containment actions: <summary>
Point of contact: <name/email/phone>
```

## 8. Post-Incident Review Checklist

- [ ] Root cause identified and verified
- [ ] Exact blast radius documented
- [ ] Timeline reconstructed with UTC timestamps
- [ ] Detection gaps identified
- [ ] Runbook updates merged
- [ ] Preventive tests/alerts added
- [ ] Follow-up owners and due dates committed

## 9. References

- `docs/incident-response-plan.md`
- `docs/emergency-protocol.md`
- `docs/emergency-contacts.md`
- `docs/upgrade_procedures.md`
- `docs/security-audit-checklist.md`
