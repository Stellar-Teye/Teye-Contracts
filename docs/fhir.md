# HL7 FHIR Support

This module provides HL7 FHIR (Fast Healthcare Interoperability Resources) standard compliance for healthcare interoperability in the Stellar Teye network.

## Supported Resources

### Patient
Stores basic demographic and administrative patient information.
- `id`: Resource Identifier
- `identifier`: System Identifier (e.g., Medical Record Number)
- `active`: Boolean indicating if the record is active
- `name`: Patient's full name
- `gender`: Patient's gender ('male', 'female', 'other', 'unknown')
- `birth_date`: Patient's date of birth (timestamp)

### Observation
Stores clinical measurements and findings.
- `id`: Resource Identifier
- `status`: Resource Status ('registered', 'preliminary', 'final', 'amended', etc.)
- `code_system`: Coding system (e.g., "LOINC", "SNOMED")
- `code_value`: Concept code
- `subject_id`: Reference to Patient ID
- `value`: The observed value 
- `effective_datetime`: When the observation occurred

## API

The SC offers the following Smart Contract methods:

- `create_patient(env, id, identifier, name, gender, birth_date) -> Patient`
- `create_observation(env, id, status, code_system, code_value, subject_id, value, effective_datetime) -> Observation`
- `validate_patient(env, patient) -> bool`
- `validate_observation(env, observation) -> bool`

These methods form the foundation for interoperable health records matching standard clinical practice structures.
