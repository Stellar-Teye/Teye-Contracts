# EMR Bridge Contract API Reference

## Contract Purpose

Handles interoperability between different Electronic Medical Record (EMR) systems. Manages provider registration, data format mapping, and synchronization between heterogeneous healthcare systems (EHR, HL7, FHIR).

## Initialization

### `initialize(env: Env, admin: Address) -> Result<(), EmrBridgeError>`

Initialize the EMR bridge with an administrator.

**Parameters:**

- `admin` - Administrator address (must authenticate)

**Returns:** `Result<(), EmrBridgeError>`

**Errors:**

- `AlreadyInitialized` - Contract already initialized

**Example:**

```rust
client.initialize(&env, &admin_address)?;
```

---

## Public Functions

### Provider Management

#### `register_provider(env: Env, caller: Address, provider_id: String, name: String, emr_system: EmrSystem, endpoint_url: String, data_format: DataFormat) -> Result<EmrProvider, EmrBridgeError>`

Register a new EMR system provider.

**Parameters:**

- `caller` - Admin address (must authenticate)
- `provider_id` - Unique provider identifier
- `name` - Provider display name
- `emr_system` - EMR system type (EpicEHR, CernerPowerChart, Medidata, etc.)
- `endpoint_url` - API endpoint for data exchange
- `data_format` - Default data format (HL7v2, FHIR, custom)

**Returns:** `EmrProvider` struct with status set to Pending

**Errors:**

- `Unauthorized` - Caller is not admin
- `ProviderAlreadyExists` - Provider ID already registered
- `NotInitialized` - Contract not initialized

**Example:**

```rust
let provider = client.register_provider(
    &env,
    &admin,
    &"epic_001",
    &"Epic Healthcare System",
    &EmrSystem::EpicEHR,
    &"https://api.epic.local",
    &DataFormat::Hl7v2
)?;
```

---

#### `activate_provider(env: Env, caller: Address, provider_id: String) -> Result<(), EmrBridgeError>`

Activate a registered provider (admin only).

**Parameters:**

- `caller` - Admin address
- `provider_id` - Provider to activate

**Returns:** `Result<(), EmrBridgeError>`

**Errors:**

- `Unauthorized` - Caller is not admin
- `ProviderNotFound` - Provider not registered
- `ProviderNotActive` - Provider already active

**Storage:** Updates provider status to Active

---

#### `deactivate_provider(env: Env, caller: Address, provider_id: String) -> Result<(), EmrBridgeError>`

Deactivate an active provider (admin only).

**Parameters:**

- `caller` - Admin address
- `provider_id` - Provider to deactivate

**Returns:** `Result<(), EmrBridgeError>`

---

#### `get_provider(env: Env, provider_id: String) -> Result<EmrProvider, EmrBridgeError>`

Retrieve provider details.

**Parameters:**

- `provider_id` - Provider identifier

**Returns:** `EmrProvider` struct

**Errors:**

- `ProviderNotFound` - Provider not registered

---

### Field Mapping

#### `register_field_mapping(env: Env, caller: Address, source_provider: String, target_provider: String, field_mapping: FieldMapping) -> Result<(), EmrBridgeError>`

Register how fields map between two EMR systems (e.g., Epic Lab Code → FHIR ObservationCode).

**Parameters:**

- `caller` - Admin address
- `source_provider` - Source EMR provider ID
- `target_provider` - Target EMR provider ID
- `field_mapping` - Mapping configuration:
  - `source_field` - Field in source system
  - `target_field` - Field in target system
  - `transform_function` - Optional transformation (e.g., "unit_conversion")

**Returns:** `Result<(), EmrBridgeError>`

**Errors:**

- `Unauthorized` - Caller not admin
- `ProviderNotFound` - Source or target provider not registered
- `MappingAlreadyExists` - Mapping already configured

**Example:**

```rust
let mapping = FieldMapping {
    source_field: "EPIC_LAB_CODE".into(),
    target_field: "FHIR_OBSERVATION_CODE".into(),
    transform_function: Some("epic_to_fhir_codes".into()),
};
client.register_field_mapping(
    &env,
    &admin,
    &"epic_001",
    &"cerner_001",
    &mapping
)?;
```

---

#### `get_field_mapping(env: Env, source_provider: String, target_provider: String) -> Option<FieldMapping>`

Retrieve field mapping between two providers.

**Parameters:**

- `source_provider` - Source provider ID
- `target_provider` - Target provider ID

**Returns:** `Option<FieldMapping>` — Mapping configuration or None

---

### Data Exchange

#### `initiate_data_exchange(env: Env, initiator: Address, source_provider: String, target_provider: String, record_id: u64, exchange_direction: ExchangeDirection) -> Result<DataExchangeRecord, EmrBridgeError>`

Request data exchange between provider systems.

**Parameters:**

- `initiator` - Address initiating exchange (must authenticate)
- `source_provider` - Provider ID exporting data
- `target_provider` - Provider ID importing data
- `record_id` - Medical record to exchange
- `exchange_direction` - Direction: Export, Import, or Bidirectional

**Returns:** `DataExchangeRecord` with ID and status

**Errors:**

- `ProviderNotFound` - Provider not registered
- `ProviderNotActive` - Provider not active
- `ExchangeAlreadyExists` - Exchange already in progress for this record
- `InvalidMapping` - No mapping configured for these providers

**Events:**

- `exchange_initiated(source_provider, target_provider, record_id)`

**Example:**

```rust
let exchange = client.initiate_data_exchange(
    &env,
    &requester,
    &"epic_001",
    &"cerner_001",
    &patient_record_id,
    &ExchangeDirection::Export
)?;
```

---

#### `confirm_sync_completion(env: Env, caller: Address, exchange_id: u64, verification: SyncVerification) -> Result<(), EmrBridgeError>`

Confirm that data sync between providers completed successfully.

**Parameters:**

- `caller` - Provider representative (must authenticate)
- `exchange_id` - Exchange request ID
- `verification` - Verification data:
  - `record_count` - Number of records synced
  - `checksum` - Hash of synced data
  - `sync_timestamp` - When sync completed

**Returns:** `Result<(), EmrBridgeError>`

**Errors:**

- `ExchangeNotFound` - Invalid exchange ID
- `SyncFailed` - Verification failed (checksum mismatch)

**Events:**

- `sync_completed(exchange_id, record_count)`

---

#### `get_exchange_status(env: Env, exchange_id: u64) -> Result<SyncStatus, EmrBridgeError>`

Retrieve the status of a data exchange.

**Parameters:**

- `exchange_id` - Exchange ID

**Returns:** `SyncStatus` enum:

- `Pending` - Exchange created, awaiting start
- `InProgress` - Sync in progress
- `Completed` - Sync successful
- `Failed` - Sync failed

**Errors:**

- `ExchangeNotFound` - Exchange ID not found

---

### Data Format Validation

#### `validate_data_format(env: Env, data_bytes: Bytes, format: DataFormat) -> Result<bool, EmrBridgeError>`

Validate incoming data against expected format specification.

**Parameters:**

- `data_bytes` - Raw data to validate
- `format` - Expected format (HL7v2, FHIR, etc.)

**Returns:** `Result<bool, EmrBridgeError>` — true if format valid

**Supported Formats:**

- FHIR (Fast Healthcare Interoperability Resources)
- HL7v2 (Health Level 7 version 2)
- HL7v3 (Health Level 7 version 3)
- Custom (client-specified schema)

**Example:**

```rust
if client.validate_data_format(&env, &fhir_json, &DataFormat::Fhir)? {
    // Format valid, proceed
}
```

---

## Data Types

### EmrSystem

```rust
pub enum EmrSystem {
    EpicEHR,
    CernerPowerChart,
    Medidata,
    Allscripts,
    CustomAPI,
}
```

### DataFormat

```rust
pub enum DataFormat {
    Hl7v2,              // Health Level 7 v2.x
    Hl7v3,              // Health Level 7 v3.x
    Fhir,               // FHIR JSON/XML
    Custom(String),     // Custom schema ID
}
```

### EmrProvider

```rust
pub struct EmrProvider {
    pub provider_id: String,
    pub name: String,
    pub emr_system: EmrSystem,
    pub endpoint_url: String,
    pub data_format: DataFormat,
    pub status: ProviderStatus,
    pub created_at: u64,
}
```

### FieldMapping

```rust
pub struct FieldMapping {
    pub source_field: String,
    pub target_field: String,
    pub transform_function: Option<String>,
}
```

### DataExchangeRecord

```rust
pub struct DataExchangeRecord {
    pub id: u64,
    pub source_provider: String,
    pub target_provider: String,
    pub record_id: u64,
    pub direction: ExchangeDirection,
    pub status: SyncStatus,
    pub created_at: u64,
}
```

### SyncVerification

```rust
pub struct SyncVerification {
    pub record_count: u64,
    pub checksum: BytesN<32>,
    pub sync_timestamp: u64,
}
```

---

## Storage Keys

| Key                | Purpose                                       |
| ------------------ | --------------------------------------------- |
| `ADMIN`            | Administrator address                         |
| `INITIALIZED`      | Initialization flag                           |
| Provider registry  | `(PROVIDER, provider_id)`                     |
| Field mappings     | `(MAPPING, source_provider, target_provider)` |
| Data exchanges     | `(EXCHANGE, exchange_id)`                     |
| Sync verifications | `(VERIFY, exchange_id)`                       |

---

## Error Codes

| Error                       | Code | Description                      |
| --------------------------- | ---- | -------------------------------- |
| `NotInitialized`            | 1    | Contract not initialized         |
| `AlreadyInitialized`        | 2    | Contract already initialized     |
| `Unauthorized`              | 3    | Caller lacks permission          |
| `ProviderNotFound`          | 4    | Provider not registered          |
| `ProviderAlreadyExists`     | 5    | Provider ID duplicate            |
| `ProviderNotActive`         | 6    | Provider in inactive state       |
| `InvalidMapping`            | 7    | No suitable field mapping        |
| `ExchangeNotFound`          | 8    | Exchange ID not found            |
| `ExchangeAlreadyExists`     | 9    | Exchange already initiated       |
| `SyncFailed`                | 10   | Data sync verification failed    |
| `InvalidDataFormat`         | 11   | Data format validation failed    |
| `MappingAlreadyExists`      | 12   | Field mapping already configured |
| `VerificationNotFound`      | 13   | Sync verification not found      |
| `VerificationAlreadyExists` | 14   | Verification already submitted   |

---

## Events

| Event                      | Parameters                                      | Description           |
| -------------------------- | ----------------------------------------------- | --------------------- |
| `initialized`              | `(admin)`                                       | Contract initialized  |
| `provider_registered`      | `(provider_id, name)`                           | Provider registered   |
| `provider_activated`       | `(provider_id)`                                 | Provider activated    |
| `provider_deactivated`     | `(provider_id)`                                 | Provider deactivated  |
| `field_mapping_registered` | `(source, target)`                              | Field mapping created |
| `exchange_initiated`       | `(source_provider, target_provider, record_id)` | Data exchange started |
| `sync_completed`           | `(exchange_id, record_count)`                   | Sync succeeded        |
| `sync_failed`              | `(exchange_id, reason)`                         | Sync failed           |

---

## Typical Workflow

```
1. Register EMR providers (Epic, Cerner, etc.)
2. Activate providers
3. Configure field mappings between provider pairs
4. Initiate data exchange between selected providers
5. Providers perform API-level data transfer
6. Confirm sync completion with verification data
```

---

## Related Documentation

- [FHIR Integration](./fhir.md)
- [EMR Integration Guide](../docs/emr-integration.md)
- [Data Portability](../docs/data-portability.md)
- [Interoperability](../docs/integrators.md)
