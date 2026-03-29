#![cfg(test)]

use crate::{
    types::{Gender, ObservationStatus},
    FhirContract, FhirContractClient,
};
use proptest::prelude::*;
use soroban_sdk::{Address, Bytes, Env, String};

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);
}

#[test]
fn test_register_and_get_resource() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let id = String::from_str(&env, "patient-1");
    let payload = Bytes::from_slice(&env, b"{\"resourceType\":\"Patient\",\"id\":\"patient-1\"}");

    client.register_resource(&admin, &id, &payload);
    assert_eq!(client.get_resource(&id), payload);
}

#[test]
fn test_update_resource() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let id = String::from_str(&env, "patient-1");
    let payload1 = Bytes::from_slice(&env, b"v1");
    let payload2 = Bytes::from_slice(&env, b"v2");

    client.register_resource(&admin, &id, &payload1);
    client.update_resource(&admin, &id, &payload2);
    assert_eq!(client.get_resource(&id), payload2);
}

#[test]
fn test_delete_resource() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let id = String::from_str(&env, "patient-1");
    let payload = Bytes::from_slice(&env, b"v1");

    client.register_resource(&admin, &id, &payload);
    client.delete_resource(&admin, &id);

    let result = client.try_get_resource(&id);
    assert!(result.is_err());
}

#[test]
fn test_patient_validation() {
    let env = Env::default();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);

    let id = String::from_str(&env, "p1");
    let identifier = String::from_str(&env, "id1");
    let name = String::from_str(&env, "John Doe");
    let gender = Gender::Male;
    let birth_date = 123456789;

    let patient = client.create_patient(&id, &identifier, &name, &gender, &birth_date);
    assert!(client.validate_patient(&patient));
}

// ── Invalid input fuzzing (proptest) ─────────────────────────────────────────

proptest! {
    // Generate a broad space of malformed inputs for ids and payloads and
    // ensure the contract fails gracefully with contract errors (no host crash).
    #[test]
    fn proptest_register_resource_handles_malformed_args(
        id_bytes in prop::collection::vec(any::<u8>(), 0..64),
        payload_bytes in prop::collection::vec(any::<u8>(), 0..256),
        unauthorized in any::<bool>(),
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(FhirContract, ());
        let client = FhirContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        // Turn arbitrary bytes into a (possibly empty) UTF-8-ish id string.
        let id_std = core::str::from_utf8(&id_bytes).unwrap_or("");
        let id = String::from_str(&env, id_std);

        let payload = Bytes::from_slice(&env, &payload_bytes);

        let caller = if unauthorized {
            Address::generate(&env)
        } else {
            admin.clone()
        };

        let res = client.try_register_resource(&caller, &id, &payload);

        // Valid cases: non-empty id and payload and authorized caller.
        // Everything else must return a contract error (not panic/crash).
        if !unauthorized && !id.is_empty() && !payload.is_empty() {
            prop_assert!(res.is_ok());
            // Read-back should match exactly.
            prop_assert_eq!(client.get_resource(&id), payload);
        } else {
            prop_assert!(res.is_err());
        }
    }
}

#[test]
fn test_register_resource_empty_id_or_payload_fails_with_invalid_payload() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let empty = String::from_str(&env, "");
    let nonempty = String::from_str(&env, "x");
    let payload_empty = Bytes::new(&env);
    let payload_nonempty = Bytes::from_slice(&env, b"x");

    let r1 = client.try_register_resource(&admin, &empty, &payload_nonempty);
    assert!(r1.is_err());
    let r2 = client.try_register_resource(&admin, &nonempty, &payload_empty);
    assert!(r2.is_err());
}
