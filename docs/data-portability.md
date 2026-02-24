# Data Portability & Export

## Supported Formats

- **CSV**: Comma-separated values for interoperability.
- **JSON**: Human-readable structured format.
- **CCD / CCDA**: Standard clinical document formats (XML-based).

## Bulk Export

Multiple patient records can be exported at once to any supported format using the `bulk_export` function.

## Import Validation

- Checks file existence.
- Validates file extension against format.
- Can be extended to validate schema.

## Usage Example

```rust
use contracts::vision_records::export::{PatientRecord, ExportFormat, bulk_export};

let patients = vec![
    PatientRecord { id: 1, name: "Alice".to_string(), date_of_birth: "1990-01-01".to_string(), conditions: vec!["Asthma".to_string()], medications: vec!["Inhaler".to_string()] }
];

bulk_export(&patients, &[ExportFormat::Csv, ExportFormat::Json, ExportFormat::Ccda], "./exports").unwrap();
```
