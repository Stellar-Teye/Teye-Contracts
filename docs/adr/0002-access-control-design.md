# ADR-0002: Access Control and Delegation Design

- **Status**: Accepted
- **Date**: 2026-02-23

## Context

Stellar Teye manages highly sensitive medical data and must:

- Ensure only authorised parties (patients, providers, admins) can access or modify records.
- Support **delegated access** (care teams, assistants, temporary specialists).
- Represent permissions in a way that works well:
  - On-chain for audits and enforcement.
  - Off-chain for integration with healthcare workflows.

## Decision

- Implement a **role-based access control (RBAC)** system in the `vision_records` contract:
  - Roles: `Patient`, `Staff`, `Optometrist`, `Ophthalmologist`, `Admin`.
  - Permissions: `ReadAnyRecord`, `WriteRecord`, `ManageAccess`, `ManageUsers`, `SystemAdmin`.
  - Each role maps to a base permission set.
- Support **per-user customisation** via:
  - `custom_grants`: additional permissions beyond the base role.
  - `custom_revokes`: explicit denials, even if the base role would permit.
- Support **delegation**:
  - A delegator can grant a role to a delegatee with an expiration.
  - Delegated permissions are checked via `has_delegated_permission`.
- Use **explicit capability checks** at contract boundaries:
  - Example: `add_record` requires `WriteRecord` or `SystemAdmin`.
  - Example: `grant_access` requires:
    - Caller is patient, or
    - Delegated `ManageAccess`, or
    - `SystemAdmin`.

## Rationale

- RBAC fits well with healthcare workflows:
  - Roles (patient, clinician, admin) map naturally to existing responsibilities.
  - Regulatory requirements often reference role/permission models.
- Custom grants/revokes allow finer-grained adjustments without role explosion.
- Delegation is essential for:
  - On-call coverage and cross-clinic collaborations.
  - Temporary access during referrals or second opinions.
- Explicit permission checks at call sites:
  - Make business rules visible in code and tests.
  - Reduce risk of accidentally relying on implicit or ad-hoc access patterns.

## Consequences

- **Positive**:
  - Clear, testable access rules enforced on-chain.
  - Easy to audit roles, delegations, and effective permissions.
  - Flexible enough to model complex workflows (e.g., admin assistants, rotating clinicians).
- **Negative / Trade-offs**:
  - Slightly more complex contract logic (RBAC + delegation + access grants).
  - Requires careful design of default role mappings to avoid over-privilege.

## Implementation Notes

- Core RBAC and delegation logic lives in:
  - `contracts/vision_records/src/rbac.rs`
  - Used by `lib.rs` for:
    - `register_user`
    - `add_record`
    - `grant_access` / `revoke_access`
    - Provider registration and verification
- Additional compliance-focused access control patterns are explored in:
  - `contracts/compliance/src/access_control.rs`
  - `docs/error-handling.md`
  - `docs/fhir.md`

