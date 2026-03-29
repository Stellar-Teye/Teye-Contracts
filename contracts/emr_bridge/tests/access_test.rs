#![allow(clippy::unwrap_used, clippy::expect_used)]

use emr_bridge::{
    types::{DataFormat, EmrSystem, ExchangeDirection, SyncStatus},
    EmrBridgeContract, EmrBridgeContractClient, EmrBridgeError,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

fn s(env: &Env, value: &str) -> String {
    String::from_str(env, value)
}

fn setup() -> (Env, EmrBridgeContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EmrBridgeContract, ());
    let client = EmrBridgeContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin, attacker)
}

fn register_active_provider(
    env: &Env,
    client: &EmrBridgeContractClient<'_>,
    admin: &Address,
) -> String {
    let provider_id = s(env, "provider-1");
    client.register_provider(
        admin,
        &provider_id,
        &s(env, "Provider One"),
        &EmrSystem::EpicFhir,
        &s(env, "https://provider.example"),
        &DataFormat::FhirR4,
    );
    client.activate_provider(admin, &provider_id);
    provider_id
}

fn create_exchange(
    env: &Env,
    client: &EmrBridgeContractClient<'_>,
    admin: &Address,
    provider_id: &String,
) -> String {
    let exchange_id = s(env, "exchange-1");
    client.record_data_exchange(
        admin,
        &exchange_id,
        provider_id,
        &s(env, "patient-1"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &s(env, "Patient"),
        &s(env, "hash-1"),
    );
    exchange_id
}

#[test]
fn test_register_provider_random_address_is_unauthorized() {
    let (env, client, _admin, attacker) = setup();

    assert_eq!(
        client.try_register_provider(
            &attacker,
            &s(&env, "provider-1"),
            &s(&env, "Provider One"),
            &EmrSystem::EpicFhir,
            &s(&env, "https://provider.example"),
            &DataFormat::FhirR4,
        ),
        Err(Ok(EmrBridgeError::Unauthorized))
    );
}

#[test]
fn test_activate_provider_random_address_is_unauthorized() {
    let (env, client, admin, attacker) = setup();
    let provider_id = register_active_provider(&env, &client, &admin);

    assert_eq!(
        client.try_suspend_provider(&attacker, &provider_id),
        Err(Ok(EmrBridgeError::Unauthorized))
    );
}

#[test]
fn test_suspend_provider_random_address_is_unauthorized() {
    let (env, client, admin, attacker) = setup();
    let provider_id = s(&env, "provider-1");
    client.register_provider(
        &admin,
        &provider_id,
        &s(&env, "Provider One"),
        &EmrSystem::EpicFhir,
        &s(&env, "https://provider.example"),
        &DataFormat::FhirR4,
    );

    assert_eq!(
        client.try_activate_provider(&attacker, &provider_id),
        Err(Ok(EmrBridgeError::Unauthorized))
    );
}

#[test]
fn test_record_data_exchange_random_address_is_unauthorized() {
    let (env, client, admin, attacker) = setup();
    let provider_id = register_active_provider(&env, &client, &admin);

    assert_eq!(
        client.try_record_data_exchange(
            &attacker,
            &s(&env, "exchange-1"),
            &provider_id,
            &s(&env, "patient-1"),
            &ExchangeDirection::Import,
            &DataFormat::FhirR4,
            &s(&env, "Patient"),
            &s(&env, "hash-1"),
        ),
        Err(Ok(EmrBridgeError::Unauthorized))
    );
}

#[test]
fn test_update_exchange_status_random_address_is_unauthorized() {
    let (env, client, admin, attacker) = setup();
    let provider_id = register_active_provider(&env, &client, &admin);
    let exchange_id = create_exchange(&env, &client, &admin, &provider_id);

    assert_eq!(
        client.try_update_exchange_status(&attacker, &exchange_id, &SyncStatus::Completed),
        Err(Ok(EmrBridgeError::Unauthorized))
    );
}

#[test]
fn test_create_field_mapping_random_address_is_unauthorized() {
    let (env, client, admin, attacker) = setup();
    let provider_id = register_active_provider(&env, &client, &admin);

    assert_eq!(
        client.try_create_field_mapping(
            &attacker,
            &s(&env, "mapping-1"),
            &provider_id,
            &s(&env, "source"),
            &s(&env, "target"),
            &s(&env, "identity"),
        ),
        Err(Ok(EmrBridgeError::Unauthorized))
    );
}

#[test]
fn test_verify_sync_random_address_is_unauthorized() {
    let (env, client, admin, attacker) = setup();
    let provider_id = register_active_provider(&env, &client, &admin);
    let exchange_id = create_exchange(&env, &client, &admin, &provider_id);
    let discrepancies: Vec<String> = Vec::new(&env);

    assert_eq!(
        client.try_verify_sync(
            &attacker,
            &s(&env, "verification-1"),
            &exchange_id,
            &s(&env, "source-hash"),
            &s(&env, "target-hash"),
            &discrepancies,
        ),
        Err(Ok(EmrBridgeError::Unauthorized))
    );
}
