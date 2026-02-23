use std::collections::HashMap;

/// Errors that can occur during DID operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DIDError {
    /// DID string does not start with "did:".
    MissingPrefix,
    /// DID is missing the method component (second segment).
    MissingMethod,
    /// DID is missing the method-specific identifier (third segment).
    MissingIdentifier,
    /// A DID component contains invalid characters (must be alphanumeric, '.', '-', '_').
    InvalidCharacters,
    /// DID already exists in the registry.
    AlreadyRegistered,
    /// DID not found in the registry.
    NotFound,
}

/// Validate that `did` conforms to the simplified W3C DID syntax:
///   `did:<method>:<method_specific_id>`
/// Both `<method>` and `<method_specific_id>` must be non-empty and
/// contain only alphanumeric characters plus `.`, `-`, `_`, and `:`.
pub fn validate_did_format(did: &str) -> Result<(), DIDError> {
    // Must start with the "did:" prefix.
    let rest = did.strip_prefix("did:").ok_or(DIDError::MissingPrefix)?;

    // Split on the first ':' to get method and method_specific_id.
    let colon_pos = rest.find(':').ok_or(DIDError::MissingMethod)?;

    let method = &rest[..colon_pos];
    let id = &rest[colon_pos + 1..];

    if method.is_empty() {
        return Err(DIDError::MissingMethod);
    }
    if id.is_empty() {
        return Err(DIDError::MissingIdentifier);
    }

    // Validate characters in method: [a-z0-9]
    let valid_method = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    if !method.chars().all(valid_method) {
        return Err(DIDError::InvalidCharacters);
    }

    // Validate characters in id: [a-zA-Z0-9._:%-]
    let valid_id =
        |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '%');
    if !id.chars().all(valid_id) {
        return Err(DIDError::InvalidCharacters);
    }

    Ok(())
}

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
    /// Create a new DID document. Returns `Err` if the DID string is malformed.
    pub fn new(id: &str) -> Result<Self, DIDError> {
        validate_did_format(id)?;
        Ok(Self {
            id: id.to_string(),
            controller: None,
            verification_methods: HashMap::new(),
        })
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
    /// Register a DID document. The document's `id` is validated on construction, so by the time it reaches the registry the format is guaranteed correct.  Returns `Err(AlreadyRegistered)` if a document with the same id already exists.
    pub fn register(&mut self, doc: DIDDocument) -> Result<(), DIDError> {
        if self.docs.contains_key(&doc.id) {
            return Err(DIDError::AlreadyRegistered);
        }
        self.docs.insert(doc.id.clone(), doc);
        Ok(()) // signals no error occured
    }

    pub fn resolve(&self, id: &str) -> Result<&DIDDocument, DIDError> {
        self.docs.get(id).ok_or(DIDError::NotFound)
    }
}
