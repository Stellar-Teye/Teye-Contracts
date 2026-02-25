# FHIR Contract API Reference

## Contract Purpose

Implements FHIR (Fast Healthcare Interoperability Resources) standards for healthcare data representation. Provides validation, serialization, and interoperability with other healthcare systems using FHIR-compliant data structures.

## Overview

FHIR is the modern standard for healthcare data exchange. This contract facilitates:

- Patient resource creation and validation
- Observation (lab, vital signs) resource handling
- FHIR profile compliance checking
- Interoperability with other FHIR-compliant systems

---

## Public Functions

### Patient Resource Management

#### `create_patient(env: Env, id: String, identifier: String, name: String, gender: Gender, birth_date: u64) -> Patient`

Create a FHIR Patient resource.

**Parameters:**

- `id` - Patient Identifier (UUID or MRN)
- `identifier` - External identifier (e.g., EHR ID, insurance ID)
- `name` - Patient full name
- `gender` - Biological sex (Male, Female, Other, Unknown)
- `birth_date` - Birth date timestamp

**Returns:** `Patient` resource struct

**Example:**

```rust
let patient = client.create_patient(
    &env,
    &uuid,
    &"MRN-12345",
    &"John Doe",
    &Gender::Male,
    &631152000  // Jan 1, 1990
);
```

---

#### `validate_patient(env: Env, patient: Patient) -> bool`

Validate a Patient resource against FHIR profile constraints.

**Parameters:**

- `patient` - Patient resource to validate

**Returns:** `bool` — true if valid

**Validation Rules:**

- ID must be non-empty
- Name must be non-empty
- Birth date must be reasonable (not in future)

**Example:**

```rust
if client.validate_patient(&env, &patient)? {
    // Patient is valid FHIR resource
}
```

---

### Observation Resource Management

#### `create_observation(env: Env, id: String, status: ObservationStatus, code_system: String, code_value: String, subject_id: String, value: String, effective_datetime: u64) -> Observation`

Create a FHIR Observation resource (lab result, vital sign, etc.).

**Parameters:**

- `id` - Observation identifier
- `status` - Status enum (Registered, Preliminary, Final, Amended, Corrected, Cancelled, Unknown)
- `code_system` - Coding system (e.g., "LOINC", "SNOMED-CT")
- `code_value` - Coded value (e.g., "2345-7" for glucose)
- `subject_id` - Reference to Patient ID
- `value` - Observation value (can be numeric or text)
- `effective_datetime` - When observation was made

**Returns:** `Observation` resource

**Common LOINC Codes:**
| Observation | LOINC Code |
|-------------|-----------|
| Blood Glucose | 2345-7 |
| Systolic BP | 8480-6 |
| Heart Rate | 3141-9 |
| Temperature | 8310-5 |

**Example:**

```rust
let observation = client.create_observation(
    &env,
    &"BS-12345",
    &ObservationStatus::Final,
    &"LOINC",
    &"2345-7",  // Blood Glucose
    &patient_id,
    &"120",      // mg/dL
    &timestamp
);
```

---

#### `validate_observation(env: Env, observation: Observation) -> bool`

Validate an Observation resource against FHIR constraints.

**Parameters:**

- `observation` - Observation to validate

**Returns:** `bool` — true if valid

**Validation Rules:**

- ID must be non-empty
- Code system must be non-empty
- Subject ID must be non-empty
- Effective datetime should be reasonable

**Example:**

```rust
if client.validate_observation(&env, &observation)? {
    // Observation is FHIR-compliant
}
```

---

## Data Types

### Patient

```rust
pub struct Patient {
    pub id: String,
    pub identifier: String,      // External ID (MRN, etc.)
    pub name: String,
    pub active: bool,            // Is this patient active?
    pub gender: Gender,
    pub birth_date: u64,
}
```

### Gender

```rust
pub enum Gender {
    Male,
    Female,
    Other,
    Unknown,
}
```

### Observation

```rust
pub struct Observation {
    pub id: String,
    pub status: ObservationStatus,
    pub code_system: String,     // e.g., "LOINC"
    pub code_value: String,      // e.g., "2345-7"
    pub subject_id: String,      // Reference to Patient.id
    pub value: String,           // Observation result
    pub effective_datetime: u64,
}
```

### ObservationStatus

```rust
pub enum ObservationStatus {
    Registered,
    Preliminary,
    Final,
    Amended,
    Corrected,
    Cancelled,
    Unknown,
}
```

---

## FHIR Compliance Profile

This contract implements **FHIR R4 (Release 4)** baseline:

- [Patient Resource](https://www.hl7.org/fhir/patient.html)
- [Observation Resource](https://www.hl7.org/fhir/observation.html)

**Supported Coding Systems:**
| System | Display | URL |
|--------|---------|-----|
| LOINC | Logical Observation Identifiers Names Codes | `http://loinc.org` |
| SNOMED-CT | Systematized Nomenclature of Medicine | `http://snomed.info/sct` |
| ICD-10-CM | International Classification of Diseases | `http://hl7.org/fhir/sid/icd-10-cm` |

---

## Common Use Cases

### 1. Recording a Lab Result

```rust
// Create patient if not exists
let patient = client.create_patient(&env, &uuid, &mrn, &name, &gender, &birth_date);
assert!(client.validate_patient(&env, &patient)?);

// Record blood glucose observation
let obs = client.create_observation(
    &env,
    &"lab_001",
    &ObservationStatus::Final,
    &"LOINC",
    &"2345-7",
    &patient.id,
    &"120 mg/dL",
    &timestamp
);
assert!(client.validate_observation(&env, &obs)?);
```

### 2. Storing Vital Signs

```rust
// Systolic BP (LOINC 8480-6)
let bp_sys = client.create_observation(
    &env,
    &"vs_001",
    &ObservationStatus::Final,
    &"LOINC",
    &"8480-6",
    &patient_id,
    &"130 mmHg",
    &timestamp
);

// Heart Rate (LOINC 3141-9)
let hr = client.create_observation(
    &env,
    &"vs_002",
    &ObservationStatus::Final,
    &"LOINC",
    &"3141-9",
    &patient_id,
    &"78 bpm",
    &timestamp
);
```

---

## Integration with EMR Bridge

The FHIR contract integrates with the EMR Bridge for system interoperability:

1. **EMR Systems** (Epic, Cerner) store data in proprietary formats
2. **FHIR Contract** provides standardized representations
3. **EMR Bridge** maps between proprietary ↔ FHIR formats
4. **Other Systems** can work with standard FHIR data

---

## Related Documentation

- [FHIR Standard](../docs/fhir.md)
- [EMR Integration](./emr_bridge.md)
- [Data Portability](../docs/data-portability.md)
- [FHIR Official Specification](https://www.hl7.org/fhir/R4/)

---

## Notes

- No explicit contract errors (validation returns bool)
- FHIR resources are immutable once created
- DateTime is stored as Unix timestamp (seconds since epoch)
- Gender follows ISO/IEC 5218 standard
