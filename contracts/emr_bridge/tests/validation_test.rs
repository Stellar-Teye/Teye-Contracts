#![cfg(test)]

use emr_bridge::types::{DataFormat, EmrSystem};
use emr_bridge::{EmrBridgeContract, EmrBridgeContractClient, EmrBridgeError};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String, Vec,
};

#[test]
fn test_create_field_mapping_empty_strings() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EmrBridgeContract, ());
    let client = EmrBridgeContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let provider_id = String::from_str(&env, "prov_1_zero");

    let result = client.try_create_field_mapping(
        &admin,
        &String::from_str(&env, "map_1"),
        &provider_id,
        &String::from_str(&env, "src"),
        &String::from_str(&env, "tgt"),
        &String::from_str(&env, "rule"),
    );
    assert_eq!(result.unwrap_err(), Ok(EmrBridgeError::ProviderNotFound));

    client.register_provider(
        &admin,
        &provider_id,
        &String::from_str(&env, "Test Prov"),
        &EmrSystem::Fhir,
        &String::from_str(&env, "http://zero"),
        &DataFormat::Json,
    );

    let empty_str = String::from_str(&env, "");

    let result_empty_src = client.try_create_field_mapping(
        &admin,
        &String::from_str(&env, "map_1"),
        &provider_id,
        &empty_str,
        &String::from_str(&env, "tgt"),
        &String::from_str(&env, "rule"),
    );
    assert_eq!(
        result_empty_src.unwrap_err(),
        Ok(EmrBridgeError::InvalidMapping)
    );

    let result_empty_tgt = client.try_create_field_mapping(
        &admin,
        &String::from_str(&env, "map_2"),
        &provider_id,
        &String::from_str(&env, "src"),
        &empty_str,
        &String::from_str(&env, "rule"),
    );
    assert_eq!(
        result_empty_tgt.unwrap_err(),
        Ok(EmrBridgeError::InvalidMapping)
    );
}

#[test]
fn test_verify_sync_empty_discrepancies() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EmrBridgeContract, ());
    let client = EmrBridgeContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let provider_id = String::from_str(&env, "prov_1");
    client.register_provider(
        &admin,
        &provider_id,
        &String::from_str(&env, "Prov"),
        &EmrSystem::Fhir,
        &String::from_str(&env, "http://test"),
        &DataFormat::Json,
    );
    client.activate_provider(&admin, &provider_id);

    let exchange_id = String::from_str(&env, "ex_1");
    client.record_data_exchange(
        &admin,
        &exchange_id,
        &provider_id,
        &String::from_str(&env, "pat_1"),
        &emr_bridge::types::ExchangeDirection::Export,
        &DataFormat::Json,
        &String::from_str(&env, "Observation"),
        &String::from_str(&env, "hash_1"),
    );

    let discrepancies: Vec<String> = Vec::new(&env);
    let verification_id = String::from_str(&env, "v_1");

    let result = client.verify_sync(
        &admin,
        &verification_id,
        &exchange_id,
        &String::from_str(&env, "hash_1"),
        &String::from_str(&env, "hash_1"),
        &discrepancies,
    );
    assert_eq!(result.is_consistent, true);
}
