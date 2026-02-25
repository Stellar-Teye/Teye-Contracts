# ADR-0011: FHIR and Healthcare Interoperability

- **Status**: Accepted
- **Date**: 2026-02-25

## Context

Healthcare interoperability requires standardized data exchange. The system must map on-chain types to FHIR resources and support integration with EMR systems. Data portability requirements demand a consistent export and import format.

## Decision

- Map core contract types to FHIR resource shapes for exchange.
- Use an EMR bridge contract for external system integration.
- Define a canonical portability format that aligns with FHIR where possible.

## Rationale

- FHIR is a widely adopted standard and reduces integration friction.
- A dedicated EMR bridge isolates external integration logic.
- Canonical formats keep exports consistent across partners.

## Consequences

- Positive: improved interoperability with healthcare systems.
- Positive: standardized exports simplify third-party integration.
- Negative: mapping layers add transformation overhead.
- Negative: some FHIR features may not be fully supported on-chain.
- Negative: version drift between FHIR revisions requires ongoing maintenance.

## References

- contracts/fhir/
- contracts/emr_bridge/
- docs/fhir.md
- docs/emr-integration.md
- docs/data-portability.md

## Implementation Notes

- Resource mappings are versioned to allow incremental updates.
- Portability exports include metadata for downstream validation.
