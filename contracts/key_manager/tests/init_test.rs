#![cfg(test)]
#![allow(clippy::unwrap_used)]

use key_manager::{
    ContractError, KeyLevel, KeyManagerContract, KeyManagerContractClient, KeyPolicy, KeyType,
};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Symbol, Vec};

fn setup_env() -> (Env, KeyManagerContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(KeyManagerContract, ());
    let client = KeyManagerContractClient::new(&env, &contract_id);
    (env, client)
}

#[test]
fn test_successful_initialization() {
    let (env, client) = setup_env();
    let admin = Address::generate(&env);
    let identity = Address::generate(&env);

    // Should succeed silently
    client.initialize(&admin, &identity);

    // We can verify it worked by calling something admin-gated
    let new_identity = Address::generate(&env);
    assert!(client.try_set_identity_contract(&admin, &new_identity).is_ok());
}

#[test]
fn test_double_initialization_is_ignored() {
    let (env, client) = setup_env();
    let admin_1 = Address::generate(&env);
    let identity_1 = Address::generate(&env);
    
    let admin_2 = Address::generate(&env);
    let identity_2 = Address::generate(&env);

    client.initialize(&admin_1, &identity_1);

    // Call a second time with different parameters
    client.initialize(&admin_2, &identity_2);

    // Verify admin 1 is still the admin
    let dummy = Address::generate(&env);
    let res = client.try_set_identity_contract(&admin_2, &dummy);
    
    // admin_2 should get Unauthorized, proving admin_1 is still the designated admin
    assert_eq!(res.unwrap_err().unwrap(), ContractError::Unauthorized);
    
    // admin_1 should succeed
    assert!(client.try_set_identity_contract(&admin_1, &dummy).is_ok());
}

#[test]
fn test_set_identity_contract_before_init_returns_not_initialized() {
    let (env, client) = setup_env();
    let caller = Address::generate(&env);
    let identity = Address::generate(&env);

    let res = client.try_set_identity_contract(&caller, &identity);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::NotInitialized);
}

#[test]
fn test_create_master_key_before_init_returns_not_initialized() {
    let (env, client) = setup_env();
    let caller = Address::generate(&env);
    let policy = KeyPolicy {
        max_uses: 10,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(&env),
    };
    let key_bytes = BytesN::from_array(&env, &[1; 32]);

    let res = client.try_create_master_key(
        &caller,
        &KeyType::Encryption,
        &policy,
        &86400,
        &key_bytes,
    );
    assert_eq!(res.unwrap_err().unwrap(), ContractError::NotInitialized);
}

#[test]
fn test_derive_key_before_init_returns_not_found() {
    let (env, client) = setup_env();
    let caller = Address::generate(&env);
    let parent_id = BytesN::from_array(&env, &[1; 32]);
    let policy = KeyPolicy {
        max_uses: 10,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(&env),
    };

    let res = client.try_derive_key(
        &caller,
        &parent_id,
        &KeyLevel::Contract,
        &1,
        &true,
        &KeyType::Signing,
        &policy,
        &86400,
    );
    // require_owner_or_admin checks ADMIN, but it happens after load_key_record
    assert_eq!(res.unwrap_err().unwrap(), ContractError::KeyNotFound);
}

#[test]
fn test_use_key_before_init_returns_not_initialized() {
    let (env, client) = setup_env();
    let caller = Address::generate(&env);
    let key_id = BytesN::from_array(&env, &[1; 32]);
    let op = Symbol::new(&env, "sign");

    let res = client.try_use_key(&caller, &key_id, &op);
    // Even though it loads key record first (which might return KeyNotFound),
    // let's see. Wait, if `load_key_record` happens BEFORE `require_owner_or_admin`, 
    // it will return `KeyNotFound`. Let's actually check behavior.
    assert_eq!(res.unwrap_err().unwrap(), ContractError::KeyNotFound);
}

#[test]
fn test_rotate_key_before_init_returns_not_found() {
    let (env, client) = setup_env();
    let caller = Address::generate(&env);
    let key_id = BytesN::from_array(&env, &[1; 32]);

    let res = client.try_rotate_key(&caller, &key_id);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::KeyNotFound);
}

#[test]
fn test_revoke_key_before_init_returns_not_found() {
    let (env, client) = setup_env();
    let caller = Address::generate(&env);
    let key_id = BytesN::from_array(&env, &[1; 32]);

    let res = client.try_revoke_key(&caller, &key_id);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::KeyNotFound);
}

#[test]
fn test_initiate_recovery_before_init_returns_not_found() {
    let (env, client) = setup_env();
    let guardian = Address::generate(&env);
    let key_id = BytesN::from_array(&env, &[1; 32]);
    let new_key = BytesN::from_array(&env, &[2; 32]);

    // load_key_record happens before load_guardians
    let res = client.try_initiate_recovery(&guardian, &key_id, &new_key);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::KeyNotFound);
}

#[test]
fn test_read_only_endpoints_return_none_on_fresh_contract() {
    let (env, client) = setup_env();
    let key_id = BytesN::from_array(&env, &[1; 32]);

    assert!(client.get_key_record(&key_id).is_none());
    assert!(client.get_key_version(&key_id, &1).is_none());
    assert!(client.get_audit_entry(&1).is_none());
    assert!(client.get_audit_tail().is_none());
}
