use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

// ---------------------
// Patient data structure
// ---------------------
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatientRecord {
    pub id: u64,
    pub name: String,
    pub date_of_birth: String,
    pub conditions: Vec<String>,
    pub medications: Vec<String>,
}

// ---------------------
// Export Formats
// ---------------------
pub enum ExportFormat {
    Csv,
    Json,
    Ccda, // Placeholder for CCD/CCDA XML
}

// ---------------------
// Export Functions
// ---------------------
pub fn export_patient_records(
    records: &[PatientRecord],
    format: ExportFormat,
    output_path: &str,
) -> io::Result<()> {
    match format {
        ExportFormat::Csv => export_csv(records, output_path),
        ExportFormat::Json => export_json(records, output_path),
        ExportFormat::Ccda => export_ccda(records, output_path),
    }
}

// ---------------------
// CSV Export
// ---------------------
fn export_csv(records: &[PatientRecord], path: &str) -> io::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    for record in records {
        wtr.serialize(record)?;
    }
    wtr.flush()?;
    Ok(())
}

// ---------------------
// JSON Export
// ---------------------
fn export_json(records: &[PatientRecord], path: &str) -> io::Result<()> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, records)?;
    Ok(())
}

// ---------------------
// CCDA Export (simple XML placeholder)
// ---------------------
fn export_ccda(records: &[PatientRecord], path: &str) -> io::Result<()> {
    let mut file = File::create(path)?;
    write!(file, "<CCDA>\n")?;
    for r in records {
        write!(
            file,
            "  <Patient id=\"{}\">\n    <Name>{}</Name>\n    <DOB>{}</DOB>\n  </Patient>\n",
            r.id, r.name, r.date_of_birth
        )?;
    }
    write!(file, "</CCDA>")?;
    Ok(())
}

// ---------------------
// Bulk Export
// ---------------------
pub fn bulk_export(
    records: &[PatientRecord],
    formats: &[ExportFormat],
    base_path: &str,
) -> io::Result<()> {
    for format in formats {
        let filename = match format {
            ExportFormat::Csv => format!("{}/patients.csv", base_path),
            ExportFormat::Json => format!("{}/patients.json", base_path),
            ExportFormat::Ccda => format!("{}/patients.xml", base_path),
        };
        export_patient_records(records, format.clone(), &filename)?;
    }
    Ok(())
}

// ---------------------
// Import Validation (basic)
// ---------------------
pub fn validate_import_file(path: &str, format: ExportFormat) -> bool {
    let file_exists = Path::new(path).exists();
    if !file_exists {
        return false;
    }

    match format {
        ExportFormat::Csv => path.ends_with(".csv"),
        ExportFormat::Json => path.ends_with(".json"),
        ExportFormat::Ccda => path.ends_with(".xml"),
    }
}
