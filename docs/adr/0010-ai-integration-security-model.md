# ADR-0010: AI Integration Security Model

- **Status**: Accepted
- **Date**: 2026-02-25

## Context

AI-assisted vision analysis can improve diagnostics but introduces third-party processing risks. The system must integrate AI providers while maintaining patient consent, data minimization, and verifiable outputs. We need a controlled request and verification workflow to avoid untrusted model outputs or data leakage.

## Decision

- Maintain a provider registry with verification status and capabilities.
- Use a request/response lifecycle that binds requests to consent and scope.
- Require provider response verification and optional anomaly flagging.
- Record AI usage and outputs in audit logs.

## Rationale

- A registry allows governance to approve and revoke providers.
- Scoped requests reduce data exposure and limit use to consented workflows.
- Verification and anomaly tracking enable trust monitoring and QA.

## Consequences

- Positive: stronger trust model for AI providers.
- Positive: auditable AI access and output lineage.
- Negative: verification adds latency to AI workflows.
- Negative: false positive anomaly flags may cause operational noise.
- Negative: provider onboarding requires governance oversight.

## References

- contracts/ai_integration/
- docs/ai_integration.md

## Implementation Notes

- Provider status and keys are stored in the contract registry.
- Responses include attestations used by the verification step.
