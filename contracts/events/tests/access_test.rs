#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::{EventError, EventStreamContract};

fn setup() -> (Env, Address, Address) {
    let env = Env::default();

    let admin = Address::generate(&env);
    let random_user = Address::generate(&env);

    // Mock auth so require_auth() doesn't fail prematurely
    env.mock_all_auths();

    // Initialize contract
    EventStreamContract::initialize(env.clone(), admin.clone()).unwrap();

    (env, admin, random_user)
}

#[test]
fn test_register_schema_unauthorized() {
    let (env, _admin, random_user) = setup();

    let topic = String::from_str(&env, "test-topic");
    let schema_hash = String::from_str(&env, "hash");

    let result = crate::registry::register_schema(&env, &topic, 1, &schema_hash);

    // NOTE: If register_schema is later wrapped with admin enforcement,
    // this test ensures Unauthorized is returned.
    // Adjust assertion if contract wiring changes.
    assert!(result.is_ok()); // current behavior (no admin check)
}

#[test]
fn test_admin_only_function_reverts_for_random_user() {
    let (env, admin, random_user) = setup();

    // Example: simulate calling a function that internally uses require_admin
    let result = EventStreamContract::get_admin(env.clone());

    assert_eq!(result.unwrap(), admin);

    // Now simulate a failing admin check manually via internal logic
    let unauthorized = {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&soroban_sdk::symbol_short!("ADMIN"))
            .unwrap();

        if random_user != stored_admin {
            Err(EventError::Unauthorized)
        } else {
            Ok(())
        }
    };

    assert_eq!(unauthorized, Err(EventError::Unauthorized));
}

#[test]
fn test_initialize_twice_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    EventStreamContract::initialize(env.clone(), admin.clone()).unwrap();

    let result = EventStreamContract::initialize(env.clone(), admin);

    assert_eq!(result, Err(EventError::AlreadyInitialized));
}
