# STRIDE Threat Model: Healthcare Data System

This document applies the STRIDE (Spoofing, Tampering, Repudiation, Information disclosure, Denial of service, Elevation of privilege) framework to the Teye on-chain healthcare data system.

**In-scope:** Vision records contract (patient data, access control, audit), ZK verifier (proof verification), Staking (tokens and rewards), Identity (DID and recovery).
**Out-of-scope:** Off-chain EMR, frontends, and network/infrastructure outside the contracts.

---

## System Overview

- **Vision Records:** Stores hashed/encrypted vision care records; access via RBAC, consent, and grants; audit log for access events.
- **ZK Verifier:** Verifies Groth16 proofs for privacy-preserving access; rate-limited and whitelisted.
- **Staking:** Stake token, earn reward token; timelock on withdrawals; admin can set reward rate and lock period (with optional multisig).
- **Identity:** DID-like ownership; guardians and M-of-N recovery; optional ZK credential verification.

---

## STRIDE by Threat Type

### S — Spoofing (Impersonation)

| Threat | Component | Mitigation | Residual risk |
|--------|-----------|------------|----------------|
| Caller pretends to be another user | All contracts | Soroban `require_auth()` on sensitive actions; Stellar auth ensures signer is the address. | Low |
| Admin takeover via stolen key | All (admin) | Two-step admin transfer; optional multisig for vision_records and staking. | Medium if single key compromised |
| Guardian spoofing in recovery | Identity | Guardians are addresses; `guardian.require_auth()` on initiate/approve. | Low |
| Proof submitted by wrong user | ZK verifier | `request.user.require_auth()`; audit trail binds user to resource_id and proof_hash. | Low |

---

### T — Tampering (Modification of data or code)

| Threat | Component | Mitigation | Residual risk |
|--------|-----------|------------|----------------|
| Change record content or access rules | Vision records | On-chain state only; writes gated by WriteRecord/ManageAccess; encryption key rotation is admin-only. | Low |
| Alter audit log | Vision records | Append-only audit entries; no delete; events emitted for sync. | Low |
| Change verification key to accept bad proofs | ZK verifier | Only admin can set VK; protect admin key and use two-step transfer. | Medium (key compromise) |
| Tamper with staking balances or reward rate | Staking | State in contract only; admin rate/lock changes via timelock and optional multisig. | Low |
| Tamper with recovery state | Identity | Guardian list and recovery request stored on-chain; only owner/guardians can change. | Low |
| Upgrade contract to malicious WASM | All | Stellar upgrade flow; governance should require multisig or DAO. | Depends on deployment governance |

---

### R — Repudiation (Denying an action)

| Threat | Component | Mitigation | Residual risk |
|--------|-----------|------------|----------------|
| User denies having accessed a record | Vision records | Audit log (actor, patient, record_id, action, result, timestamp); events for off-chain retention. | Low |
| Admin denies config change | All | Admin transfer and config changes emit events; two-step and multisig leave on-chain trail. | Low |
| Guardian denies recovery approval | Identity | Recovery request stores approvals (addresses); execute_recovery only after threshold. | Low |
| ZK access denied | ZK verifier | AuditTrail logs successful verify_access (user, resource_id, proof_hash). | Low |

---

### I — Information Disclosure (Leaking sensitive data)

| Threat | Component | Mitigation | Residual risk |
|--------|-----------|------------|----------------|
| Plaintext PHI on-chain | Vision records | Only hashes/encrypted payloads stored; decryption off-chain with key from contract or separate channel. | Low if key handling is secure |
| Metadata leak (who accessed whom) | Vision records | Audit log is sensitive; restrict read access to audit log (e.g. admin/compliance only) if exposed by view functions. | Medium |
| Record IDs or prescription existence | Vision records | `get_records`, `get_patient_records`, `get_prescription` lack caller checks — can leak “patient X has record IDs / prescription”. | **Medium** (see checklist) |
| Profile hashes re-identify patient | Vision records | get_profile is world-readable; hashes alone may be hard to invert but could link to other data. | Low–Medium |
| Proof public inputs reveal attributes | ZK verifier | Public inputs are on-chain; circuit design must not encode PHI in public inputs. | Design-dependent |
| Staker balances | Staking | View functions are public; acceptable for economic transparency. | Accepted |

---

### D — Denial of Service (Making the system unavailable or unusable)

| Threat | Component | Mitigation | Residual risk |
|--------|-----------|------------|----------------|
| Rate limit exhaustion | Vision records, ZK verifier | Per-address rate limiting; admin can adjust or disable. | Low |
| Gas / CPU exhaustion via large batches | Vision records | add_records / grant_access_batch bounded by transaction limits; no unbounded loops over user-set size. | Low |
| Fill storage to break TTL or cost | All | Soroban storage and TTL; cost model limits growth. | Low |
| Pause entire contract | Vision records | Circuit breaker can pause by scope (e.g. function); only authorized admin. | Operational (admin can DoS intentionally) |
| Lock period or reward rate set to extreme values | Staking | Admin-controlled; multisig and timelock reduce single-point abuse. | Low |
| Guardian list or recovery stuck | Identity | Max 5 guardians; owner can cancel recovery; threshold must be ≤ guardian count. | Low |

---

### E — Elevation of Privilege (Gaining capabilities one shouldn’t have)

| Threat | Component | Mitigation | Residual risk |
|--------|-----------|------------|----------------|
| Regular user gets ReadAnyRecord or SystemAdmin | Vision records | RBAC and admin tiers; grant_custom_permission and promote_admin require ManageUsers / SuperAdmin. | Low |
| User reads another patient’s record | Vision records | get_record enforces patient/provider/consent/grant/ReadAnyRecord; get_records does not — **gap**. | **Medium** |
| Non-provider creates prescriptions | Vision records | add_prescription checks get_user(provider) and role Optometrist/Ophthalmologist. | Low |
| Non-owner changes guardians or recovery | Identity | require_active_owner for add/remove guardian, set_threshold, cancel_recovery. | Low |
| Non-admin sets VK or whitelist | ZK verifier | require_admin on all such functions. | Low |
| Non-admin changes reward rate or lock period | Staking | require_admin or require_admin_tier; multisig when configured. | Low |
| Recovery by non-guardians | Identity | initiate_recovery and approve_recovery require guardian auth; execute_recovery enforces threshold and cooldown. | Low |

---

## Data Flow Summary

1. **Patient data:** Created by provider (vision_records); read by patient, provider, or grantees; consent and grants stored on-chain; access logged in audit module.
2. **ZK access:** User submits proof to zk_verifier; on success, audit trail updated; vision_records may use identity + ZK for credential checks.
3. **Staking:** User stakes → contract holds tokens; rewards accrue; withdraw after timelock; admin sets rate/lock (with optional delay and multisig).
4. **Identity:** Owner sets guardians and threshold; recovery requires M-of-N guardian approvals and cooldown; ZK verifier address configurable by owner.

---

## Recommended Mitigations (from STRIDE)

1. **Vision records:** Add access control to `get_records`, `get_patient_records`, and `get_prescription` (same policy as get_record / patient-scoped access).
2. **Admin keys:** Use multisig for admin operations where available; secure key storage and rotation.
3. **Audit log:** Ensure audit log and events are only queryable by authorized compliance/admin roles if exposed via public view.
4. **ZK circuits:** Ensure public inputs do not contain PHI; document circuit semantics and key rotation.
5. **Emergency access:** If enabled, document and restrict to specific roles/contracts and include in audit.

---

*Document version: 1.0. Revisit when adding new contracts or changing trust boundaries.*
