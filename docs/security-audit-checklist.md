# Security Audit Checklist

This checklist covers all public functions across the Teye healthcare data contracts. Use it for pre-deployment audits and ongoing security reviews.

**Scope:** `vision_records`, `zk_verifier`, `staking`, `identity`.  
**Compliance** is a standard Rust library (not a Soroban contract); audit log integrity on-chain is implemented in `vision_records` (audit module).

---

## 1. Integer Overflow / Arithmetic

| Check | vision_records | zk_verifier | staking | identity |
|-------|----------------|-------------|---------|----------|
| Counters use saturating or checked math | ✓ REC_CTR, RX_CTR, rate window | N/A (no counters in hot path) | ✓ UNSTK_CTR, RPT, earned | N/A |
| Duration/expiry use saturating_add where needed | ✓ grant_consent, grant_access | ✓ rate window | ✓ unlock_at, effective_at | ✓ execute_after |
| Reward math uses PRECISION and saturating ops | N/A | N/A | ✓ rewards.rs | N/A |
| No unchecked `+`/`*` on user-controlled amounts | ✓ | ✓ | ✓ | ✓ |

### Per-contract notes

- **vision_records:** `add_record` uses `unwrap_or(0) + 1` for REC_CTR (could overflow at u64::MAX; low risk). `grant_consent` uses `saturating_add(duration_seconds)`. Rate limit uses `saturating_add`.
- **staking:** `rewards::compute_reward_per_token` divides by `total_staked`; guarded by `if total_staked <= 0 { return stored }`. `earned` and RPT use `saturating_*`. Timelock uses `saturating_add` for unlock_at.
- **identity:** Recovery cooldown and execute_after use fixed constants; guardian list bounded by MAX_GUARDIANS (5).

---

## 2. Access Control

### vision_records (Patient data access)

| Function | Expected access | Checklist |
|----------|-----------------|-----------|
| `initialize` | First deployer (no auth; one-time) | ⬜ Single init guard |
| `get_admin`, `is_initialized` | Anyone (read-only) | ✓ |
| `propose_admin`, `accept_admin`, `cancel_admin_transfer` | Current admin / pending admin | ✓ require_auth + admin check |
| `configure_multisig`, `propose_admin_action`, `approve_admin_action` | Admin / multisig signers | ✓ |
| `set_rate_limit_config`, `set_encryption_key` | Admin / multisig / SystemAdmin | ✓ |
| `set_whitelist_enabled`, `add_to_whitelist`, `remove_from_whitelist` | ContractAdmin tier or legacy admin | ✓ |
| `register_user` | ManageUsers + whitelist | ✓ |
| `get_user` | Anyone (metadata) | ✓ |
| `add_record`, `add_records` | Provider or delegate WriteRecord; whitelist; rate limit | ✓ |
| `get_record` | Patient, provider, consent, grant, or ReadAnyRecord/SystemAdmin | ✓ |
| `get_records` | **No caller auth** — returns records by ID | ⚠️ **See Known Risks** |
| `get_patient_records` | **No caller auth** — returns list of record IDs for any patient | ⚠️ **See Known Risks** |
| `add_eye_examination`, `get_eye_examination` | Same as get_record write/read | ✓ |
| `grant_access`, `grant_access_batch` | Patient or ManageAccess delegate / SystemAdmin | ✓ |
| `check_access`, `check_record_access` | Anyone (read-only) | ✓ |
| `grant_record_access`, `revoke_record_access` | Patient only | ✓ |
| `grant_consent`, `revoke_consent`, `revoke_access` | Patient only | ✓ |
| `purge_expired_grants` | Patient or SystemAdmin | ✓ |
| `get_record_count` | Anyone | ✓ |
| `add_prescription` | Provider; role Optometrist/Ophthalmologist | ✓ |
| `get_prescription`, `get_prescription_history`, `verify_prescription` | get_prescription has no auth; history/verify check user | ⚠️ get_prescription: no access control |
| `create_profile` | Patient or ManageUsers | ✓ |
| `update_demographics`, `update_emergency_contact`, `update_insurance`, `add_medical_history_reference` | Patient only | ✓ |
| `get_profile`, `profile_exists` | No auth (read-only) | ✓ Design choice: profile metadata |
| `grant_custom_permission`, `revoke_custom_permission` | ManageUsers | ✓ |
| `delegate_role` | Delegator (require_auth) | ✓ |
| `pause_contract`, `resume_contract` | Circuit breaker admin | ✓ |
| `create_acl_group`, `add_user_to_group`, `remove_user_from_group` | ManageUsers | ✓ |
| `get_user_groups`, `check_permission` | Anyone (read-only) | ✓ |
| `promote_admin`, `demote_admin` | SuperAdmin only | ✓ |

### zk_verifier (Proof verification)

| Function | Expected access | Checklist |
|----------|-----------------|-----------|
| `initialize` | One-time; require_auth on admin | ✓ |
| `propose_admin`, `accept_admin`, `cancel_admin_transfer` | Admin / pending admin | ✓ |
| `set_rate_limit_config`, `set_verification_key` | Admin | ✓ |
| `set_whitelist_enabled`, `add_to_whitelist`, `remove_from_whitelist` | Admin | ✓ |
| `verify_access` | request.user require_auth; whitelist; rate limit | ✓ |
| `get_audit_record`, `verify_audit_chain` | Anyone (read-only) | ✓ |

### staking (Token handling and rewards)

| Function | Expected access | Checklist |
|----------|-----------------|-----------|
| `initialize` | One-time; no auth (deployer) | ✓ Single init guard |
| `stake`, `request_unstake`, `withdraw`, `claim_rewards` | staker require_auth | ✓ |
| `get_staked`, `get_pending_rewards`, `get_staker_info`, `get_reward_rate`, `get_total_staked`, `get_lock_period`, `get_rate_change_delay`, `get_pending_rate_change`, `get_unstake_request` | Anyone (read-only) | ✓ |
| `propose_admin`, `accept_admin`, `cancel_admin_transfer` | Admin / pending | ✓ |
| `configure_multisig`, `propose_admin_action`, `approve_admin_action` | Admin / signers | ✓ |
| `set_reward_rate`, `apply_reward_rate`, `set_rate_change_delay`, `set_lock_period` | Admin / multisig / tier | ✓ |
| `promote_admin`, `demote_admin` | SuperAdmin | ✓ |

### identity (DID management)

| Function | Expected access | Checklist |
|----------|-----------------|-----------|
| `initialize` | One-time (no auth) | ✓ |
| `add_guardian`, `remove_guardian`, `set_recovery_threshold` | Active owner | ✓ |
| `initiate_recovery`, `approve_recovery` | Guardian require_auth | ✓ |
| `execute_recovery` | Any (caller); recovery module enforces state | ✓ |
| `cancel_recovery` | Active owner | ✓ |
| `set_zk_verifier` | Active owner | ✓ |
| `verify_zk_credential` | user require_auth | ✓ |
| `is_owner_active`, `get_guardians`, `get_recovery_threshold`, `get_recovery_request`, `get_zk_verifier` | Anyone (read-only) | ✓ |

---

## 3. Storage Key Collisions

| Contract | Key pattern | Collision risk |
|----------|-------------|----------------|
| vision_records | `(Symbol, Address)`, `(Symbol, u64)`, `(Symbol, Address, Address)`, `(Symbol, u64, Address)` | Low; symbols and types differ. Consistent prefixes (USER, RECORD, PAT_REC, ACCESS, CONSENT, REC_ACC, PAT_PROF, etc.). |
| zk_verifier | `(RATE_TRACK, user)`, instance ADMIN, PENDING_ADMIN, RATE_CFG, VK | Low. |
| staking | `(USER_STAKE|USER_RPT_PAID|USER_EARNED, staker)`, instance keys | Low. |
| identity | `(GUARDIANS|REC_THR|REC_REQ|OWN_ACT, owner)` | Low. |

**Checklist:** ⬜ Confirm no two key types share the same Symbol + same value type in the same namespace (instance vs persistent).  
**Status:** Key design uses distinct symbols per entity; no known collisions.

---

## 4. Reentrancy

| Contract | State-changing entry points | Mitigation |
|----------|----------------------------|------------|
| vision_records | `add_record`, `grant_access` | ReentrancyGuard used |
| vision_records | `add_records`, `grant_access_batch`, token/transfer flows | No external token transfer; batch does not call back into contract |
| zk_verifier | `verify_access` | No token transfer; cross-contract to verifier lib only |
| staking | `stake`, `request_unstake`, `withdraw`, `claim_rewards` | ReentrancyGuard on stake, withdraw, claim_rewards; withdraw marks request withdrawn before transfer |
| identity | `execute_recovery` | No token transfer in identity contract |

**Checklist:** ⬜ All external token transfers (staking) occur after state updates (checks-effects-interactions).  
**Status:** Withdraw sets `withdrawn = true` before transfer; stake/claim update state then transfer.

---

## 5. Input Validation

| Contract | Function / area | Validation |
|----------|------------------|------------|
| vision_records | `register_user` | validate_name (length, printable ASCII) |
| vision_records | `add_record` | validate_data_hash (length, alphanumeric + -_) |
| vision_records | `grant_access`, `grant_record_access` | validate_duration (min/max bounds) |
| vision_records | `set_rate_limit_config` | Reject 0/0 |
| vision_records | `add_records`, `grant_access_batch` | Non-empty list |
| vision_records | `grant_consent` | duration_seconds != 0 |
| vision_records | Prescription / profile | Role and profile existence checks |
| zk_verifier | `verify_access` | validate_request: non-empty public_inputs, len ≤ MAX_PUBLIC_INPUTS, degenerate proof checks |
| zk_verifier | Proof components | validate_proof_components (zeroed, 0xFF, malformed G1/G2) |
| zk_verifier | `set_rate_limit_config` | Reject 0/0 |
| staking | `initialize` | reward_rate ≥ 0, stake_token != reward_token |
| staking | `stake`, `request_unstake` | amount > 0 |
| staking | `request_unstake` | prev_stake ≥ amount |
| staking | `set_reward_rate` | new_rate ≥ 0 |
| identity | `add_guardian` | len < MAX_GUARDIANS, no duplicate |
| identity | `set_recovery_threshold` | threshold ≤ guardian count, ≥ 1 |

**Checklist:** ⬜ All public functions that take user-controlled data validate bounds and format where applicable.

---

## 6. Event Emission

| Contract | Critical actions | Event emitted |
|----------|------------------|----------------|
| vision_records | Init, admin transfer | publish_initialized, publish_admin_transfer_* |
| vision_records | User registration | publish_user_registered |
| vision_records | Record add | publish_record_added / publish_batch_records_added |
| vision_records | Access grant/revoke/expire | publish_access_granted, publish_access_revoked, publish_access_expired |
| vision_records | Consent | publish_consent_granted, publish_consent_revoked |
| vision_records | Audit (access attempts) | publish_audit_log_entry |
| vision_records | Errors | publish_error (with context) |
| zk_verifier | Admin, whitelist, rate limit | publish_admin_transfer_*, etc. |
| zk_verifier | verify_access | publish_access_rejected on failure; AuditTrail on success |
| staking | Init, stake, unstake, withdraw, claim, admin, rate/lock | events::publish_* for each |
| identity | Recovery lifecycle | Via recovery module state; consider explicit events for execute_recovery |

**Checklist:** ⬜ Every state-changing admin and user action that affects assets or access has a corresponding event for off-chain indexing and auditing.

---

## Known Risks and Mitigations

| Risk | Severity | Contract | Mitigation |
|------|----------|----------|------------|
| `get_records(record_ids)` returns records by ID without caller auth; leaks metadata (patient, provider, record_type) and encrypted data_hash. | **Medium** | vision_records | Add caller parameter and enforce same access logic as `get_record` per record; or deprecate and use `get_record` in a loop. |
| `get_patient_records(patient)` returns list of record IDs for any patient without auth. | **Medium** | vision_records | Require caller auth and enforce that caller is patient, provider for that patient, or has consent/grant/ReadAnyRecord. |
| `get_prescription(rx_id)` has no access control; returns prescription for any rx_id. | **Medium** | vision_records | Add caller and check patient/provider/consent or role before returning. |
| `get_profile` / `profile_exists` are world-readable; profile holds hashed PII. | **Low** | vision_records | Acceptable if only hashes are stored; ensure no re-identification from hashes. Document as design choice. |
| Record counter in vision_records could theoretically overflow at u64::MAX. | **Low** | vision_records | Use saturating_add or checked add; or document as acceptable for lifespan of system. |
| initialize() on vision_records does not call admin.require_auth(). | **Low** | vision_records | Document deployer trust; or add require_auth for init. |
| ZK verification key replacement by admin could invalidate existing proofs. | **Low** | zk_verifier | Operational: coordinate key rotation and proof circuit versioning. |
| Staking reward rate and lock period changes affect user value; timelock and multisig mitigate. | **Low** | staking | Rate change delay and multisig for set_reward_rate / set_lock_period. |
| Identity recovery: M-of-N guardians can take over DID. | **Medium** | identity | Rely on guardian selection and threshold; consider recovery cooldown and notifications. |
| Emergency access in vision_records (if used) can bypass normal consent. | **Medium** | vision_records | Ensure emergency module is gated (e.g. specific role/contract) and fully audited. |

---

## Compliance / Audit Log Integrity (on-chain)

Audit log integrity is implemented **in vision_records** (contracts/vision_records/src/audit.rs), not in the separate `compliance` crate (which is a standard Rust library). Checklist:

- Audit entries are written on access attempts (success, denied, not found).
- Keys use distinct symbols (AUD_ENT, AUD_REC, AUD_USR, AUD_PAT) and TTL extension.
- No deletion of audit entries (append-only).
- Events publish_audit_log_entry for off-chain sync.

⬜ Verify that all code paths that read/write/grant/revoke patient data call `audit::add_audit_entry` and `events::publish_audit_log_entry` as required.

---

## Severity Ratings (for documented risks)

- **Critical:** Unauthorized access to plaintext PHI, theft of funds, or permanent loss of control.
- **High:** Unauthorized access to metadata or hashed data, or significant economic impact.
- **Medium:** Information disclosure (e.g. record IDs, prescription existence), or privilege escalation with constraints.
- **Low:** Design tradeoffs, operational considerations, or theoretical overflow.

---

*Last updated: 2025. Re-run checklist when adding or changing public functions.*
