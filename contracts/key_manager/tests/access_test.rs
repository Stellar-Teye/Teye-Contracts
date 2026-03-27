#![cfg(test)]

use key_manager::{
    ContractError, KeyLevel, KeyManagerContract, KeyManagerContractClient, KeyPolicy, KeyType,
};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, BytesN, Env, Vec};

fn setup() -> (Env, KeyManagerContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KeyManagerContract, ());
    let client = KeyManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let identity_contract = Address::generate(&env);
    client.initialize(&admin, &identity_contract);

    (env, client, admin, identity_contract)
}

#[test]
fn test_unauthenticated_admin_calls() {
    let (env, client, _admin, identity_contract) = setup();
    let unauthenticated_address = Address::generate(&env);

    // 1. set_identity_contract should fail when called by a random address
    let res_set_identity =
        client.try_set_identity_contract(&unauthenticated_address, &identity_contract);
    assert_eq!(res_set_identity, Err(Ok(ContractError::Unauthorized)));

    // 2. create_master_key should fail when called by a random address
    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(&env),
    };
    let key_bytes = BytesN::from_array(&env, &[0u8; 32]);
    let res_create_master = client.try_create_master_key(
        &unauthenticated_address,
        &KeyType::Signing,
        &policy,
        &0,
        &key_bytes,
    );
    assert_eq!(res_create_master, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_unauthenticated_owner_level_calls() {
    let (env, client, admin, _identity_contract) = setup();

    // Create a master key with admin to have a key_id for testing other calls
    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(&env),
    };
    let key_bytes = BytesN::from_array(&env, &[1u8; 32]);
    let key_id = client.create_master_key(&admin, &KeyType::Signing, &policy, &0, &key_bytes);

    let unauthenticated_address = Address::generate(&env);

    // 3. derive_key should fail when called by a random address (not owner or admin)
    let res_derive = client.try_derive_key(
        &unauthenticated_address,
        &key_id,
        &KeyLevel::Contract,
        &0,
        &false,
        &KeyType::Signing,
        &policy,
        &0,
    );
    assert_eq!(res_derive, Err(Ok(ContractError::Unauthorized)));

    // 4. use_key should fail when called by a random address
    let res_use = client.try_use_key(&unauthenticated_address, &key_id, &symbol_short!("OP"));
    assert_eq!(res_use, Err(Ok(ContractError::Unauthorized)));

    // 5. rotate_key should fail when called by a random address
    let res_rotate = client.try_rotate_key(&unauthenticated_address, &key_id);
    assert_eq!(res_rotate, Err(Ok(ContractError::Unauthorized)));

    // 6. revoke_key should fail when called by a random address
    let res_revoke = client.try_revoke_key(&unauthenticated_address, &key_id);
    assert_eq!(res_revoke, Err(Ok(ContractError::Unauthorized)));
}
