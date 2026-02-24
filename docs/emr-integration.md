# EMR/EHR System Integration

## Overview

The EMR Bridge contract (`emr_bridge`) provides integration points between the Teye platform and Electronic Medical Records (EMR) / Electronic Health Records (EHR) systems. It enables secure data exchange, provider onboarding, field mapping, and sync verification.

## Architecture

```text
┌──────────────┐     ┌──────────────────┐     ┌──────────────┐
│  EMR System  │◄───►│  EMR Bridge      │◄───►│  Teye        │
│  (Epic,      │     │  Contract        │     │  Contracts   │
│   Cerner,    │     │                  │     │              │
│   etc.)      │     │  - Provider Mgmt │     │  - Vision    │
│              │     │  - Data Exchange │     │  - Identity  │
│              │     │  - Field Mapping │     │  - FHIR      │
│              │     │  - Sync Verify   │     │              │
└──────────────┘     └──────────────────┘     └──────────────┘
```

## Contract Functions

### Initialization

- `initialize(admin)` — Set the contract administrator

### Provider Onboarding

- `register_provider(caller, provider_id, name, emr_system, endpoint_url, data_format)` — Register a new EMR provider
- `activate_provider(caller, provider_id)` — Activate a pending provider
- `suspend_provider(caller, provider_id)` — Suspend an active provider
- `get_provider(provider_id)` — Get provider details
- `list_providers()` — List all registered provider IDs

### Data Exchange

- `record_data_exchange(caller, exchange_id, provider_id, patient_id, direction, data_format, resource_type, record_hash)` — Record a data exchange
- `update_exchange_status(caller, exchange_id, new_status)` — Update exchange status
- `get_exchange(exchange_id)` — Get exchange record
- `get_patient_exchanges(patient_id)` — Get all exchanges for a patient

### Field Mapping

- `create_field_mapping(caller, mapping_id, provider_id, source_field, target_field, transform_rule)` — Create a field mapping
- `get_field_mapping(mapping_id)` — Get a field mapping
- `get_provider_mappings(provider_id)` — Get all mappings for a provider

### Sync Verification

- `verify_sync(caller, verification_id, exchange_id, source_hash, target_hash, discrepancies)` — Verify data consistency
- `get_verification(verification_id)` — Get verification record

## Data Types

### EmrSystem

- `EpicFhir` — Epic Systems (FHIR API)
- `CernerMillennium` — Cerner Millennium
- `Allscripts` — Allscripts
- `Athenahealth` — Athenahealth
- `Custom` — Custom EMR system

### DataFormat

- `FhirR4` — FHIR R4 standard
- `Hl7V2` — HL7 v2 messages
- `CcdA` — C-CDA documents
- `Custom` — Custom format

### ProviderStatus

- `Pending` — Awaiting activation
- `Active` — Active and operational
- `Suspended` — Temporarily suspended
- `Revoked` — Permanently revoked

### SyncStatus

- `Pending` — Exchange initiated
- `InProgress` — Data transfer in progress
- `Completed` — Successfully completed
- `Failed` — Exchange failed
- `PartialSuccess` — Partially completed with discrepancies

## Provider Onboarding Flow

1. Admin registers a provider with `register_provider`
2. Provider starts in `Pending` status
3. Admin activates with `activate_provider` → status becomes `Active`
4. Admin can suspend with `suspend_provider` if needed

## Data Exchange Flow

1. Admin records an exchange with `record_data_exchange`
2. Exchange starts in `Pending` status
3. Status can be updated via `update_exchange_status`
4. After sync, verify with `verify_sync`
5. Verification updates exchange status to `Completed` or `PartialSuccess`

## Error Codes

| Code | Name | Description |
| ---- | ---- | ----------- |
| 1 | NotInitialized | Contract not initialized |
| 2 | AlreadyInitialized | Contract already initialized |
| 3 | Unauthorized | Caller is not admin |
| 4 | ProviderNotFound | Provider ID not found |
| 5 | ProviderAlreadyExists | Provider ID already registered |
| 6 | ProviderNotActive | Provider is not in Active status |
| 7 | InvalidMapping | Invalid field mapping (empty fields) |
| 8 | ExchangeNotFound | Exchange ID not found |
| 9 | ExchangeAlreadyExists | Exchange ID already exists |
| 10 | SyncFailed | Sync operation failed |
| 11 | InvalidDataFormat | Invalid data format |
| 12 | MappingAlreadyExists | Mapping ID already exists |
| 13 | VerificationNotFound | Verification record not found |
| 14 | VerificationAlreadyExists | Verification ID already exists |

## Security Considerations

- All state-changing operations require admin authorization
- Patient IDs are stored on-chain but NOT emitted in events (PHI protection)
- Data hashes are used for integrity verification without storing actual medical data
- Provider status checks prevent data exchange with inactive providers
- Instance storage TTL is extended on every admin operation to prevent contract lapse
- Duplicate guards prevent overwriting existing records

## Integration with Other Teye Contracts

- **FHIR Contract**: The EMR bridge uses FHIR R4 as a primary data format
- **Identity Contract**: Provider addresses can be linked to DID identities
- **Vision Records**: Exchange records can reference vision care data
