use soroban_sdk::String;

use crate::ContractError;

const MIN_NAME_LEN: u32 = 2;
const MAX_NAME_LEN: u32 = 64;

const MIN_HASH_LEN: u32 = 32;
const MAX_HASH_LEN: u32 = 64;

const MIN_DURATION_SECONDS: u64 = 3600; // 1 hour
const MAX_DURATION_SECONDS: u64 = 157_680_000; // 5 years

/// Validate a user's name.
/// Names must be between MIN_NAME_LEN and MAX_NAME_LEN bytes.
/// Names should only contain printable ASCII characters (specifically alphanumeric and spaces for simplicity, but we'll accept standard printable ASCII).
pub fn validate_name(name: &String) -> Result<(), ContractError> {
    let len = name.len();
    if !(MIN_NAME_LEN..=MAX_NAME_LEN).contains(&len) {
        return Err(ContractError::InvalidInput);
    }

    // Soroban String iterators yield u8 representing ASCII/UTF-8 bytes.
    // For simplicity, we ensure all bytes are valid, readable ASCII.
    let mut buf = [0u8; MAX_NAME_LEN as usize];
    name.copy_into_slice(&mut buf[..len as usize]);

    let mut is_valid = true;
    for &b in &buf[..len as usize] {
        // We only allow printable ASCII (space ' ' to tilde '~')
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
/// Hashes (IPFS CID, SHA256 hex, etc.) must be of a reasonable length.
/// We restrict to alphanumeric characters to prevent injection of uncontrolled data.
pub fn validate_data_hash(hash: &String) -> Result<(), ContractError> {
    let len = hash.len();
    if !(MIN_HASH_LEN..=MAX_HASH_LEN).contains(&len) {
        return Err(ContractError::InvalidInput);
    }

    // Characters should be alphanumeric (e.g. base58, hex, base64url).
    // Let's allow [A-Za-z0-9_-]
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
/// Prevent extremely short durations (e.g., 0) or extremely long ones (overflow risk).
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

        // Valid
        assert_eq!(validate_name(&String::from_str(&env, "John Doe")), Ok(()));
        assert_eq!(
            validate_name(&String::from_str(&env, "Alice 123 !@#")),
            Ok(())
        );

        // Too short
        assert_eq!(
            validate_name(&String::from_str(&env, "A")),
            Err(ContractError::InvalidInput)
        );

        // Too long
        let long_name = "A".repeat(65);
        assert_eq!(
            validate_name(&String::from_str(&env, &long_name)),
            Err(ContractError::InvalidInput)
        );

        // Invalid characters (non-printable)
        let invalid_chars = String::from_str(&env, "John\nDoe");
        assert_eq!(
            validate_name(&invalid_chars),
            Err(ContractError::InvalidInput)
        );
    }

    #[test]
    fn test_validate_data_hash() {
        let env = Env::default();

        // Valid SHA-256 Hex
        let sha256_hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(
            validate_data_hash(&String::from_str(&env, sha256_hex)),
            Ok(())
        );

        // Valid IPFS CID (Base58)
        let ipfs_cid = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG";
        assert_eq!(
            validate_data_hash(&String::from_str(&env, ipfs_cid)),
            Ok(())
        );

        // Valid with hyphen and underscore
        assert_eq!(
            validate_data_hash(&String::from_str(&env, "valid_hash-with-symbols-12345678")),
            Ok(())
        );

        // Too short
        assert_eq!(
            validate_data_hash(&String::from_str(&env, "short")),
            Err(ContractError::InvalidInput)
        );

        // Too long
        let long_hash = "a".repeat(65);
        assert_eq!(
            validate_data_hash(&String::from_str(&env, &long_hash)),
            Err(ContractError::InvalidInput)
        );

        // Invalid characters (e.g. space)
        let invalid_hash = "e3b0c44298fc1c149afbf4c8996fb924 27ae41e4649b934ca495991b7852b85";
        assert_eq!(
            validate_data_hash(&String::from_str(&env, invalid_hash)),
            Err(ContractError::InvalidInput)
        );
    }

    #[test]
    fn test_validate_duration() {
        // Valid
        assert_eq!(validate_duration(3600), Ok(()));
        assert_eq!(validate_duration(157_680_000), Ok(()));
        assert_eq!(validate_duration(100_000), Ok(()));

        // Too short
        assert_eq!(validate_duration(3599), Err(ContractError::InvalidInput));
        assert_eq!(validate_duration(0), Err(ContractError::InvalidInput));

        // Too long
        assert_eq!(
            validate_duration(157_680_001),
            Err(ContractError::InvalidInput)
        );
    }
}
