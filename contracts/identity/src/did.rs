use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct VerificationMethod {
    pub id: String,
    pub type_: String,
    pub public_key: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DIDDocument {
    pub id: String,
    pub controller: Option<String>,
    pub verification_methods: HashMap<String, VerificationMethod>,
}

impl DIDDocument {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            controller: None,
            verification_methods: HashMap::new(),
        }
    }

    pub fn add_verification_method(&mut self, vm: VerificationMethod) {
        self.verification_methods.insert(vm.id.clone(), vm);
    }

    pub fn remove_verification_method(&mut self, id: &str) {
        self.verification_methods.remove(id);
    }
}

#[derive(Default)]
pub struct DIDRegistry {
    pub docs: HashMap<String, DIDDocument>,
}

impl DIDRegistry {
    pub fn register(&mut self, doc: DIDDocument) {
        self.docs.insert(doc.id.clone(), doc);
    }

    pub fn resolve(&self, id: &str) -> Option<&DIDDocument> {
        self.docs.get(id)
    }
}
