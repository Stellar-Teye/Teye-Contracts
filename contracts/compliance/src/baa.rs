#[derive(Debug, Clone)]
pub struct BAATemplate {
    pub provider: String,
    pub covered_data: String,
    pub terms: String,
}

impl BAATemplate {
    pub fn default_template() -> Self {
        BAATemplate {
            provider: "Provider".into(),
            covered_data: "PHI".into(),
            terms: "Standard BAA terms placeholder".into(),
        }
    }
}
