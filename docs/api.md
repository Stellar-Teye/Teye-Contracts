# 📚 API Reference

## Vision Records Contract

### Initialization

#### `initialize(admin: Address)`
Initialize the contract with an admin address.

**Parameters:**
- `admin`: The address that will have admin privileges

**Returns:** `Result<(), ContractError>`

**Example:**
```rust
client.initialize(&admin_address);
```

---

### User Management

#### `register_user(user: Address, role: Role, name: String)`
Register a new user in the system.

**Parameters:**
- `user`: User's Stellar address
- `role`: One of `Patient`, `Optometrist`, `Ophthalmologist`, `Admin`
- `name`: Display name

**Returns:** `Result<(), ContractError>`

---

#### `get_user(user: Address)`
Retrieve user information.

**Parameters:**
- `user`: User's Stellar address

**Returns:** `Result<User, ContractError>`

---

### Record Management

#### `add_record(patient: Address, provider: Address, record_type: RecordType, data_hash: String)`
Add a new vision record.

**Parameters:**
- `patient`: Patient's address
- `provider`: Provider's address (must authenticate)
- `record_type`: Type of record (`Examination`, `Prescription`, `Diagnosis`, etc.)
- `data_hash`: IPFS/off-chain hash of encrypted data

**Returns:** `Result<u64, ContractError>` - Record ID

---

#### `get_record(record_id: u64)`
Retrieve a record by ID.

**Parameters:**
- `record_id`: The record ID

**Returns:** `Result<VisionRecord, ContractError>`

---

#### `get_patient_records(patient: Address)`
Get all record IDs for a patient.

**Parameters:**
- `patient`: Patient's address

**Returns:** `Vec<u64>`

---

### Access Control

#### `grant_access(patient: Address, grantee: Address, level: AccessLevel, duration_seconds: u64)`
Grant access to a user.

**Parameters:**
- `patient`: Patient granting access (must authenticate)
- `grantee`: User receiving access
- `level`: Access level (`None`, `Read`, `Write`, `Full`)
- `duration_seconds`: How long access is valid

**Returns:** `Result<(), ContractError>`

---

#### `check_access(patient: Address, grantee: Address)`
Check access level for a user.

**Parameters:**
- `patient`: Patient's address
- `grantee`: User to check

**Returns:** `AccessLevel`

---

#### `revoke_access(patient: Address, grantee: Address)`
Revoke access from a user.

**Parameters:**
- `patient`: Patient revoking access (must authenticate)
- `grantee`: User losing access

**Returns:** `Result<(), ContractError>`

---

### Utility Functions

#### `get_admin()`
Get the admin address.

**Returns:** `Result<Address, ContractError>`

---

#### `is_initialized()`
Check if contract is initialized.

**Returns:** `bool`

---

#### `get_record_count()`
Get total number of records.

**Returns:** `u64`

---

#### `version()`
Get contract version.

**Returns:** `u32`

---

## Data Types

### Role
```rust
enum Role {
    Patient,
    Optometrist,
    Ophthalmologist,
    Admin,
}
```

### AccessLevel
```rust
enum AccessLevel {
    None,
    Read,
    Write,
    Full,
}
```

### RecordType
```rust
enum RecordType {
    Examination,
    Prescription,
    Diagnosis,
    Treatment,
    Surgery,
    LabResult,
}
```

### ContractError
```rust
enum ContractError {
    NotInitialized,
    AlreadyInitialized,
    Unauthorized,
    UserNotFound,
    RecordNotFound,
    InvalidInput,
    AccessDenied,
    Paused,
}
```

---

## AI Integration Contract

### `initialize(admin: Address, anomaly_threshold_bps: u32)`
Initialize AI integration with an admin and anomaly threshold.

### `register_provider(caller: Address, provider_id: u32, operator: Address, name: String, model: String, endpoint_hash: String)`
Register an approved AI provider.

### `submit_analysis_request(caller: Address, provider_id: u32, patient: Address, record_id: u64, input_hash: String, task_type: String)`
Submit a new AI analysis request and return a request ID.

### `store_analysis_result(caller: Address, request_id: u64, output_hash: String, confidence_bps: u32, anomaly_score_bps: u32)`
Store provider output and return resulting request status (`Completed` or `Flagged`).

### `verify_analysis_result(caller: Address, request_id: u64, accepted: bool, verification_hash: String)`
Verify or reject an AI analysis result.

For complete type definitions and flow details, see `docs/ai_integration.md`.
