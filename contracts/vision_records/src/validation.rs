use soroban_sdk::{String, Bytes, Env};
use crate::types::ContractError; // Ensure this matches your error enum location

const MIN_NAME_LEN: u32 = 2;
const MAX_NAME_LEN: u32 = 64;

const MIN_HASH_LEN: u32 = 32;
const MAX_HASH_LEN: u32 = 64;

const MIN_DURATION_SECONDS: u64 = 3600; // 1 hour
const MAX_DURATION_SECONDS: u64 = 157_680_000; // 5 years

/// NEW: Fixes Error #11 - Missing validation function for lib.rs logic
/// Validates that the raw prescription data is present and within reasonable size limits.
pub fn validate_prescription_data(_env: &Env, data: &Bytes) -> bool {
    // Basic safety check: ensure data isn't empty and isn't a massive memory-bomb
    data.len() > 0 && data.len() < 100_000 
}

/// Validate a user's name.
/// Names must be between MIN_NAME_LEN and MAX_NAME_LEN bytes.
pub fn validate_name(name: &String) -> Result<(), ContractError> {
    let len = name.len();
    if !(MIN_NAME_LEN..=MAX_NAME_LEN).contains(&len) {
        return Err(ContractError::InvalidInput);
    }

    let mut buf = [0u8; MAX_NAME_LEN as usize];
    name.copy_into_slice(&mut buf[..len as usize]);

    let mut is_valid = true;
    for &b in &buf[..len as usize] {
        // Only allow printable ASCII (space ' ' to tilde '~')
        if !(32..=126).contains(&b) {
            is_valid = false;
            break;
        }
    }

    if !is_valid {
        return Err(ContractError::InvalidInput);
    }

    Ok(())
}

/// Validate a record's data hash.
pub fn validate_data_hash(hash: &String) -> Result<(), ContractError> {
    let len = hash.len();
    if !(MIN_HASH_LEN..=MAX_HASH_LEN).contains(&len) {
        return Err(ContractError::InvalidInput);
    }

    let mut buf = [0u8; MAX_HASH_LEN as usize];
    hash.copy_into_slice(&mut buf[..len as usize]);

    let mut is_valid = true;
    for &b in &buf[..len as usize] {
        let valid_char = b.is_ascii_alphanumeric() || b == b'-' || b == b'_';
        if !valid_char {
            is_valid = false;
            break;
        }
    }

    if !is_valid {
        return Err(ContractError::InvalidInput);
    }

    Ok(())
}

/// Validate a grant access duration.
pub fn validate_duration(duration_seconds: u64) -> Result<(), ContractError> {
    if !(MIN_DURATION_SECONDS..=MAX_DURATION_SECONDS).contains(&duration_seconds) {
        return Err(ContractError::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_validate_name() {
        let env = Env::default();
        assert_eq!(validate_name(&String::from_str(&env, "John Doe")), Ok(()));
        assert_eq!(validate_name(&String::from_str(&env, "A")), Err(ContractError::InvalidInput));
    }

    #[test]
    fn test_validate_prescription_data() {
        let env = Env::default();
        let data = Bytes::from_slice(&env, &[1, 2, 3]);
        assert!(validate_prescription_data(&env, &data));
        
        let empty_data = Bytes::new(&env);
        assert!(!validate_prescription_data(&env, &empty_data));
    }
}