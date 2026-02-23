
---

### 3️⃣ Tests (`tests/portability_tests.rs`)

```rust
use contracts::vision_records::export::*;
use std::fs;

#[test]
fn test_csv_export() {
    let records = sample_records();
    export_patient_records(&records, ExportFormat::Csv, "test.csv").unwrap();
    assert!(fs::metadata("test.csv").is_ok());
    fs::remove_file("test.csv").unwrap();
}

#[test]
fn test_json_export() {
    let records = sample_records();
    export_patient_records(&records, ExportFormat::Json, "test.json").unwrap();
    assert!(fs::metadata("test.json").is_ok());
    fs::remove_file("test.json").unwrap();
}

#[test]
fn test_ccda_export() {
    let records = sample_records();
    export_patient_records(&records, ExportFormat::Ccda, "test.xml").unwrap();
    assert!(fs::metadata("test.xml").is_ok());
    fs::remove_file("test.xml").unwrap();
}

#[test]
fn test_bulk_export() {
    let records = sample_records();
    bulk_export(&records, &[ExportFormat::Csv, ExportFormat::Json], ".").unwrap();
    assert!(fs::metadata("patients.csv").is_ok());
    assert!(fs::metadata("patients.json").is_ok());
    fs::remove_file("patients.csv").unwrap();
    fs::remove_file("patients.json").unwrap();
}

#[test]
fn test_import_validation() {
    assert!(validate_import_file("patients.csv", ExportFormat::Csv));
    assert!(!validate_import_file("patients.json", ExportFormat::Csv));
}

fn sample_records() -> Vec<PatientRecord> {
    vec![
        PatientRecord {
            id: 1,
            name: "Alice".to_string(),
            date_of_birth: "1990-01-01".to_string(),
            conditions: vec!["Asthma".to_string()],
            medications: vec!["Inhaler".to_string()],
        },
        PatientRecord {
            id: 2,
            name: "Bob".to_string(),
            date_of_birth: "1985-05-15".to_string(),
            conditions: vec!["Diabetes".to_string()],
            medications: vec!["Insulin".to_string()],
        },
    ]
}
