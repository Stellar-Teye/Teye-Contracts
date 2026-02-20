use soroban_sdk::{contracttype, symbol_short, Address, Bytes, Env, String, Vec};

use crate::{AccessLevel, ContractError, Role, VisionRecordsContract};

/// Patient demographics information
#[contracttype]
#[derive(Clone, Debug)]
pub struct Demographics {
    pub full_name: String,
    pub date_of_birth: String,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

/// Emergency contact information
#[contracttype]
#[derive(Clone, Debug)]
pub struct EmergencyContact {
    pub name: String,
    pub relationship: String,
    pub phone: String,
}

/// Complete patient profile structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct PatientProfile {
    pub owner: Address,
    pub demographics: Demographics,
    pub medical_history: Vec<u64>,
    pub emergency_contacts: Vec<EmergencyContact>,
    /// Encrypted insurance information blob - must be encrypted client-side before storage
    /// The contract does not perform any encryption/decryption operations
    pub insurance_encrypted: Bytes,
}

/// Helper function to generate patient profile storage key
fn patient_profile_key(patient_address: Address) -> (soroban_sdk::Symbol, Address) {
    (symbol_short!("PAT_PROF"), patient_address)
}

/// Validates string field lengths to prevent excessive storage usage
fn validate_string_length(value: &String, max_length: u32) -> Result<(), ContractError> {
    if value.len() > max_length {
        return Err(ContractError::InvalidInput);
    }
    Ok(())
}

/// Validates demographics fields for reasonable length constraints
fn validate_demographics(demographics: &Demographics) -> Result<(), ContractError> {
    validate_string_length(&demographics.full_name, 256)?;
    validate_string_length(&demographics.date_of_birth, 32)?;
    
    if let Some(ref gender) = demographics.gender {
        validate_string_length(gender, 32)?;
    }
    if let Some(ref address) = demographics.address {
        validate_string_length(address, 512)?;
    }
    if let Some(ref phone) = demographics.phone {
        validate_string_length(phone, 32)?;
    }
    if let Some(ref email) = demographics.email {
        validate_string_length(email, 256)?;
    }
    
    Ok(())
}

/// Validates emergency contact fields
fn validate_emergency_contact(contact: &EmergencyContact) -> Result<(), ContractError> {
    validate_string_length(&contact.name, 256)?;
    validate_string_length(&contact.relationship, 64)?;
    validate_string_length(&contact.phone, 32)?;
    Ok(())
}

/// Checks if caller has authorization to modify patient data
fn check_patient_authorization(
    env: &Env,
    caller: &Address,
    patient: &Address,
) -> Result<(), ContractError> {
    // Patient can always modify their own data
    if caller == patient {
        return Ok(());
    }

    // Check if caller is admin
    if let Ok(admin) = VisionRecordsContract::get_admin(env.clone()) {
        if caller == &admin {
            return Ok(());
        }
    }

    // Check access level granted by patient
    let access_level = VisionRecordsContract::check_access(env.clone(), patient.clone(), caller.clone());
    match access_level {
        AccessLevel::Write | AccessLevel::Full => Ok(()),
        _ => Err(ContractError::Unauthorized),
    }
}

impl VisionRecordsContract {
    /// Creates a new patient profile
    /// 
    /// # Arguments
    /// * `env` - The contract environment
    /// * `owner` - The patient's address (must be registered with Role::Patient)
    /// * `demographics` - Patient demographic information
    /// * `insurance_encrypted` - Encrypted insurance data (must be encrypted client-side)
    /// 
    /// # Returns
    /// * `Ok(())` if profile created successfully
    /// * `Err(ContractError::UserNotFound)` if owner is not a registered patient
    /// * `Err(ContractError::InvalidInput)` if profile already exists or validation fails
    pub fn create_patient_profile(
        env: Env,
        owner: Address,
        demographics: Demographics,
        insurance_encrypted: Bytes,
    ) -> Result<(), ContractError> {
        owner.require_auth();

        // Verify owner is a registered patient
        let user = Self::get_user(env.clone(), owner.clone())?;
        if user.role != Role::Patient {
            return Err(ContractError::Unauthorized);
        }

        // Check if profile already exists
        let key = patient_profile_key(owner.clone());
        if env.storage().persistent().has(&key) {
            return Err(ContractError::InvalidInput);
        }

        // Validate demographics
        validate_demographics(&demographics)?;

        // Create profile
        let profile = PatientProfile {
            owner: owner.clone(),
            demographics,
            medical_history: Vec::new(&env),
            emergency_contacts: Vec::new(&env),
            insurance_encrypted,
        };

        // Store profile
        env.storage().persistent().set(&key, &profile);

        Ok(())
    }

    /// Retrieves a patient profile
    /// 
    /// # Arguments
    /// * `env` - The contract environment
    /// * `patient` - The patient's address
    /// 
    /// # Returns
    /// * `Ok(PatientProfile)` if profile exists
    /// * `Err(ContractError::UserNotFound)` if profile not found
    pub fn get_patient_profile(env: Env, patient: Address) -> Result<PatientProfile, ContractError> {
        let key = patient_profile_key(patient);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::UserNotFound)
    }

    /// Updates patient demographics
    /// 
    /// # Arguments
    /// * `env` - The contract environment
    /// * `caller` - The address making the request
    /// * `patient` - The patient's address
    /// * `demographics` - New demographic information
    /// 
    /// # Returns
    /// * `Ok(())` if update successful
    /// * `Err(ContractError::Unauthorized)` if caller lacks permission
    /// * `Err(ContractError::UserNotFound)` if profile not found
    /// * `Err(ContractError::InvalidInput)` if validation fails
    pub fn update_demographics(
        env: Env,
        caller: Address,
        patient: Address,
        demographics: Demographics,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Check authorization
        check_patient_authorization(&env, &caller, &patient)?;

        // Validate demographics
        validate_demographics(&demographics)?;

        // Get existing profile
        let key = patient_profile_key(patient);
        let mut profile: PatientProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::UserNotFound)?;

        // Update demographics
        profile.demographics = demographics;

        // Save updated profile
        env.storage().persistent().set(&key, &profile);

        Ok(())
    }

    /// Adds a medical record reference to patient's history
    /// 
    /// # Arguments
    /// * `env` - The contract environment
    /// * `caller` - The address making the request
    /// * `patient` - The patient's address
    /// * `record_id` - The medical record ID to add
    /// 
    /// # Returns
    /// * `Ok(())` if record reference added successfully
    /// * `Err(ContractError::Unauthorized)` if caller lacks permission
    /// * `Err(ContractError::UserNotFound)` if profile not found
    pub fn add_medical_record_reference(
        env: Env,
        caller: Address,
        patient: Address,
        record_id: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Check authorization - patient or provider with access
        check_patient_authorization(&env, &caller, &patient)?;

        // Get existing profile
        let key = patient_profile_key(patient);
        let mut profile: PatientProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::UserNotFound)?;

        // Add record ID if not already present
        if !profile.medical_history.contains(&record_id) {
            profile.medical_history.push_back(record_id);
        }

        // Save updated profile
        env.storage().persistent().set(&key, &profile);

        Ok(())
    }

    /// Adds an emergency contact to patient profile
    /// 
    /// # Arguments
    /// * `env` - The contract environment
    /// * `caller` - The address making the request
    /// * `patient` - The patient's address
    /// * `contact` - The emergency contact to add
    /// 
    /// # Returns
    /// * `Ok(())` if contact added successfully
    /// * `Err(ContractError::Unauthorized)` if caller lacks permission
    /// * `Err(ContractError::UserNotFound)` if profile not found
    /// * `Err(ContractError::InvalidInput)` if validation fails
    pub fn add_emergency_contact(
        env: Env,
        caller: Address,
        patient: Address,
        contact: EmergencyContact,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Check authorization
        check_patient_authorization(&env, &caller, &patient)?;

        // Validate contact
        validate_emergency_contact(&contact)?;

        // Get existing profile
        let key = patient_profile_key(patient);
        let mut profile: PatientProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::UserNotFound)?;

        // Add emergency contact
        profile.emergency_contacts.push_back(contact);

        // Save updated profile
        env.storage().persistent().set(&key, &profile);

        Ok(())
    }

    /// Updates encrypted insurance information
    /// 
    /// # Arguments
    /// * `env` - The contract environment
    /// * `caller` - The address making the request (must be patient or admin)
    /// * `patient` - The patient's address
    /// * `insurance_encrypted` - New encrypted insurance data
    /// 
    /// # Returns
    /// * `Ok(())` if insurance data updated successfully
    /// * `Err(ContractError::Unauthorized)` if caller lacks permission
    /// * `Err(ContractError::UserNotFound)` if profile not found
    /// 
    /// # Note
    /// Insurance data must be encrypted client-side before calling this function.
    /// The contract does not perform any encryption or decryption operations.
    pub fn update_insurance_encrypted(
        env: Env,
        caller: Address,
        patient: Address,
        insurance_encrypted: Bytes,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Only patient or admin can update insurance info
        if caller != patient {
            let admin = Self::get_admin(env.clone())?;
            if caller != admin {
                return Err(ContractError::Unauthorized);
            }
        }

        // Get existing profile
        let key = patient_profile_key(patient);
        let mut profile: PatientProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::UserNotFound)?;

        // Update insurance data
        profile.insurance_encrypted = insurance_encrypted;

        // Save updated profile
        env.storage().persistent().set(&key, &profile);

        Ok(())
    }

    /// Removes an emergency contact by index
    /// 
    /// # Arguments
    /// * `env` - The contract environment
    /// * `caller` - The address making the request
    /// * `patient` - The patient's address
    /// * `index` - The index of the contact to remove
    /// 
    /// # Returns
    /// * `Ok(())` if contact removed successfully
    /// * `Err(ContractError::Unauthorized)` if caller lacks permission
    /// * `Err(ContractError::UserNotFound)` if profile not found
    /// * `Err(ContractError::InvalidInput)` if index is out of range
    pub fn remove_emergency_contact(
        env: Env,
        caller: Address,
        patient: Address,
        index: u32,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        // Check authorization
        check_patient_authorization(&env, &caller, &patient)?;

        // Get existing profile
        let key = patient_profile_key(patient);
        let mut profile: PatientProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::UserNotFound)?;

        // Check index bounds
        if index >= profile.emergency_contacts.len() {
            return Err(ContractError::InvalidInput);
        }

        // Remove contact at index
        profile.emergency_contacts.remove(index);

        // Save updated profile
        env.storage().persistent().set(&key, &profile);

        Ok(())
    }
}