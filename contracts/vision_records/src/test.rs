#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    unused_imports,
    unused_variables
)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::Env;

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert!(client.is_initialized());
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_whitelist_register_user_and_add_record_enforcement() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let data_hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");

    client.set_whitelist_enabled(&admin, &true);

    // Admin removed from whitelist should be blocked from protected calls.
    client.remove_from_whitelist(&admin, &admin);
    let reg_blocked = client.try_register_user(
        &admin,
        &user,
        &Role::Patient,
        &String::from_str(&env, "Alice"),
    );
    assert!(reg_blocked.is_err());
    assert!(matches!(
        reg_blocked.unwrap_err(),
        Ok(ContractError::Unauthorized)
    ));

    let add_blocked = client.try_add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );
    assert!(add_blocked.is_err());
    assert!(matches!(
        add_blocked.unwrap_err(),
        Ok(ContractError::Unauthorized)
    ));

    // Admin can restore access by adding self back.
    client.add_to_whitelist(&admin, &admin);
    let reg_allowed = client.try_register_user(
        &admin,
        &user,
        &Role::Patient,
        &String::from_str(&env, "Alice"),
    );
    assert!(reg_allowed.is_ok());

    let add_allowed = client.try_add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );
    assert!(add_allowed.is_ok());

    // Global disable should bypass whitelist entries.
    client.set_whitelist_enabled(&admin, &false);
    client.remove_from_whitelist(&admin, &admin);
    let add_when_disabled = client.try_add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );
    assert!(add_when_disabled.is_ok());
}

#[test]
fn test_whitelist_enforced_for_add_records_batch() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    client.initialize(&admin);
    client.register_user(
        &admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&env, "Provider"),
    );

    client.set_whitelist_enabled(&admin, &true);
    assert!(!client.is_whitelisted(&provider));

    let mut records = Vec::new(&env);
    records.push_back(BatchRecordInput {
        patient: patient.clone(),
        record_type: RecordType::Examination,
        data_hash: String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"),
    });

    let blocked = client.try_add_records(&provider, &records);
    assert!(blocked.is_err());
    assert!(matches!(
        blocked.unwrap_err(),
        Ok(ContractError::Unauthorized)
    ));

    client.add_to_whitelist(&admin, &provider);
    let allowed = client.try_add_records(&provider, &records);
    assert!(allowed.is_ok());
}

#[test]
fn test_whitelist_admin_only_management() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let target = Address::generate(&env);
    client.initialize(&admin);

    let add_res = client.try_add_to_whitelist(&non_admin, &target);
    assert!(add_res.is_err());
    assert!(matches!(
        add_res.unwrap_err(),
        Ok(ContractError::Unauthorized)
    ));

    let remove_res = client.try_remove_from_whitelist(&non_admin, &target);
    assert!(remove_res.is_err());
    assert!(matches!(
        remove_res.unwrap_err(),
        Ok(ContractError::Unauthorized)
    ));

    let toggle_res = client.try_set_whitelist_enabled(&non_admin, &true);
    assert!(toggle_res.is_err());
    assert!(matches!(
        toggle_res.unwrap_err(),
        Ok(ContractError::Unauthorized)
    ));
}

#[test]
fn test_rate_limit_add_record_and_grant_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Configure a small window for testing
    client.set_rate_limit_config(&admin, &2, &60, &0);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let data_hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");

    // First two record additions should succeed
    client.add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );
    client.add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    // Third should be rate limited
    let res = client.try_add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(matches!(err, Ok(ContractError::RateLimitExceeded)));

    // Advance time beyond the window and ensure the limit resets
    let current = env.ledger().timestamp();
    env.ledger().set_timestamp(current + 61);

    let res_after_reset = client.try_add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );
    assert!(res_after_reset.is_ok());

    // Grant access calls should also consume the same per-address budget
    let doctor = Address::generate(&env);
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);
    let rate_limited =
        client.try_grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);
    assert!(rate_limited.is_err());
}

#[test]
fn test_permission_without_consent_denied() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    // Grant access but NOT consent
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);

    // Access denied — no consent
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
}

#[test]
fn test_consent_and_permission_grants_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    // Grant both consent and access
    client.grant_consent(&patient, &doctor, &ConsentType::Treatment, &86400);
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);

    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);
}

#[test]
fn test_revoked_consent_blocks_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.grant_consent(&patient, &doctor, &ConsentType::Sharing, &86400);
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);

    // Revoke consent
    client.revoke_consent(&patient, &doctor);

    // Access now denied despite active access grant
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
}

#[test]
fn test_expired_consent_blocks_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    // Grant short-lived consent and long-lived access
    client.grant_consent(&patient, &doctor, &ConsentType::Research, &100);
    client.grant_access(&patient, &patient, &doctor, &AccessLevel::Read, &86400);

    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);

    // Advance time past consent expiry
    env.ledger().set_timestamp(200);

    // Consent expired — access denied
    assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
}

#[test]
fn test_get_record_consent_required() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let doctor = Address::generate(&env);
    let data_hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");

    let record_id = client.add_record(
        &admin,
        &patient,
        &provider,
        &RecordType::Examination,
        &data_hash,
    );

    // Patient can always view own record
    let record = client.get_record(&patient, &record_id);
    assert_eq!(record.patient, patient);

    // Doctor without consent → error (ConsentRequired = 26)
    let result = client.try_get_record(&doctor, &record_id);
    assert!(result.is_err());

    // Grant consent → doctor can view
    client.grant_consent(&patient, &doctor, &ConsentType::Treatment, &86400);
    let record = client.get_record(&doctor, &record_id);
    assert_eq!(record.patient, patient);
}
