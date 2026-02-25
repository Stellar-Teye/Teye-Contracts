#![allow(clippy::unwrap_used, clippy::expect_used)]

use identity::IdentityContract;
use identity::IdentityContractClient;
use key_manager::{
    ContractError, KeyLevel, KeyManagerContract, KeyManagerContractClient, KeyPolicy, KeyType,
};
use soroban_sdk::{symbol_short, testutils::Address as _, testutils::Ledger, Address, BytesN, Env, Vec};

fn setup() -> (Env, KeyManagerContractClient<'static>, IdentityContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let identity_id = env.register(IdentityContract, ());
    let identity = IdentityContractClient::new(&env, &identity_id);

    let admin = Address::generate(&env);
    identity.initialize(&admin);

    let key_manager_id = env.register(KeyManagerContract, ());
    let key_manager = KeyManagerContractClient::new(&env, &key_manager_id);
    key_manager.initialize(&admin, &identity_id);

    (env, key_manager, identity, admin)
}

#[test]
fn test_rotation_preserves_versions() {
    let (env, client, _identity, admin) = setup();

    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(&env),
    };

    let key_bytes = BytesN::from_array(&env, &[7u8; 32]);
    let key_id = client.create_master_key(&admin, &KeyType::Encryption, &policy, &0u64, &key_bytes);

    let derived_v1 = client.derive_record_key(&key_id, &1u64);
    let version_v1 = derived_v1.version;

    let v2 = client.rotate_key(&admin, &key_id);
    assert_eq!(v2, version_v1 + 1);

    let derived_v1_again = client.derive_record_key_with_version(&key_id, &1u64, &version_v1);
    let derived_v2 = client.derive_record_key_with_version(&key_id, &1u64, &v2);

    assert_ne!(derived_v1_again.key, derived_v2.key);
}

#[test]
fn test_recovery_flow() {
    let (env, client, identity, admin) = setup();

    let guardian1 = Address::generate(&env);
    let guardian2 = Address::generate(&env);
    let guardian3 = Address::generate(&env);

    identity.add_guardian(&admin, &guardian1);
    identity.add_guardian(&admin, &guardian2);
    identity.add_guardian(&admin, &guardian3);
    identity.set_recovery_threshold(&admin, &2);

    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(&env),
    };

    let key_bytes = BytesN::from_array(&env, &[9u8; 32]);
    let key_id = client.create_master_key(&admin, &KeyType::Encryption, &policy, &0u64, &key_bytes);

    let new_key = BytesN::from_array(&env, &[10u8; 32]);
    client.initiate_recovery(&guardian1, &key_id, &new_key);
    client.approve_recovery(&guardian2, &key_id);

    let now = env.ledger().timestamp();
    env.ledger().set_timestamp(now + 86_401);

    let new_version = client.execute_recovery(&admin, &key_id);
    let key_version = client.get_key_version(&key_id, &new_version).unwrap();
    assert_eq!(key_version.key_bytes, new_key);
}

#[test]
fn test_policy_enforcement() {
    let (env, client, _identity, admin) = setup();

    let mut ops = Vec::new(&env);
    ops.push_back(symbol_short!("ENC"));

    let policy = KeyPolicy {
        max_uses: 1,
        not_before: 0,
        not_after: 0,
        allowed_ops: ops,
    };

    let key_bytes = BytesN::from_array(&env, &[3u8; 32]);
    let key_id = client.create_master_key(&admin, &KeyType::Encryption, &policy, &0u64, &key_bytes);

    let ok = client.use_key(&admin, &key_id, &symbol_short!("ENC"));
    assert!(ok.to_array()[0] == 3);

    let err = client.try_use_key(&admin, &key_id, &symbol_short!("ENC"));
    assert!(matches!(err, Err(Ok(ContractError::PolicyViolation))));

    let wrong_op = client.try_use_key(&admin, &key_id, &symbol_short!("SIGN"));
    assert!(matches!(wrong_op, Err(Ok(ContractError::PolicyViolation))));
}

#[test]
fn test_hierarchy_validation() {
    let (env, client, _identity, admin) = setup();

    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(&env),
    };

    let key_bytes = BytesN::from_array(&env, &[5u8; 32]);
    let master_id = client.create_master_key(&admin, &KeyType::Encryption, &policy, &0u64, &key_bytes);

    let err = client.try_derive_key(
        &admin,
        &master_id,
        &KeyLevel::Session,
        &1u32,
        &false,
        &KeyType::Encryption,
        &policy,
        &0u64,
    );
    assert!(matches!(err, Err(Ok(ContractError::InvalidHierarchy))));
}
