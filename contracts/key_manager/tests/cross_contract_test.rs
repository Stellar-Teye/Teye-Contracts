#![allow(clippy::unwrap_used, clippy::expect_used)]

use key_manager::{
    ContractError, KeyManagerContract, KeyManagerContractClient, KeyPolicy, KeyType,
};
use soroban_sdk::{
    contract, contractimpl, symbol_short, testutils::Address as _, testutils::Ledger, Address,
    BytesN, Env, Vec,
};

#[contract]
struct MockIdentityContract;

const GUARDIANS_KEY: &str = "guardians";
const THRESHOLD_KEY: &str = "threshold";
const FAIL_GUARDIANS_KEY: &str = "fail_guardians";
const FAIL_THRESHOLD_KEY: &str = "fail_threshold";

#[contractimpl]
impl MockIdentityContract {
    pub fn configure(
        env: Env,
        guardians: Vec<Address>,
        threshold: u32,
        fail_guardians: bool,
        fail_threshold: bool,
    ) {
        env.storage().instance().set(&GUARDIANS_KEY, &guardians);
        env.storage().instance().set(&THRESHOLD_KEY, &threshold);
        env.storage().instance().set(&FAIL_GUARDIANS_KEY, &fail_guardians);
        env.storage().instance().set(&FAIL_THRESHOLD_KEY, &fail_threshold);
    }

    pub fn get_guardians(env: Env, _owner: Address) -> Vec<Address> {
        if env
            .storage()
            .instance()
            .get(&FAIL_GUARDIANS_KEY)
            .unwrap_or(false)
        {
            panic!("mock guardian lookup failure");
        }

        env.storage()
            .instance()
            .get(&GUARDIANS_KEY)
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_recovery_threshold(env: Env, _owner: Address) -> u32 {
        if env
            .storage()
            .instance()
            .get(&FAIL_THRESHOLD_KEY)
            .unwrap_or(false)
        {
            panic!("mock threshold lookup failure");
        }

        env.storage().instance().get(&THRESHOLD_KEY).unwrap_or(0)
    }
}

fn setup() -> (
    Env,
    KeyManagerContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let guardian1 = Address::generate(&env);
    let guardian2 = Address::generate(&env);
    let outsider = Address::generate(&env);

    let mut guardians = Vec::new(&env);
    guardians.push_back(guardian1.clone());
    guardians.push_back(guardian2.clone());

    let identity_id = env.register(MockIdentityContract, ());
    let identity = MockIdentityContractClient::new(&env, &identity_id);
    identity.configure(&guardians, &2, &false, &false);

    let admin = Address::generate(&env);
    let key_manager_id = env.register(KeyManagerContract, ());
    let key_manager = KeyManagerContractClient::new(&env, &key_manager_id);
    key_manager.initialize(&admin, &identity_id);

    (
        env, key_manager, admin, identity_id, guardian1, guardian2, outsider,
    )
}

fn create_master_key(
    env: &Env,
    client: &KeyManagerContractClient<'static>,
    admin: &Address,
    seed: u8,
) -> BytesN<32> {
    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(env),
    };

    client.create_master_key(
        admin,
        &KeyType::Encryption,
        &policy,
        &0u64,
        &BytesN::from_array(env, &[seed; 32]),
    )
}

#[test]
fn recovery_flow_uses_mock_identity_contract_data() {
    let (env, client, admin, _identity_id, guardian1, guardian2, outsider) = setup();
    let key_id = create_master_key(&env, &client, &admin, 7);

    assert_eq!(
        client.try_initiate_recovery(
            &outsider,
            &key_id,
            &BytesN::from_array(&env, &[8u8; 32]),
        ),
        Err(Ok(ContractError::NotAGuardian))
    );

    let replacement = BytesN::from_array(&env, &[9u8; 32]);
    client.initiate_recovery(&guardian1, &key_id, &replacement);
    client.approve_recovery(&guardian2, &key_id);

    let now = env.ledger().timestamp();
    env.ledger().set_timestamp(now + 86_401);

    let version = client.execute_recovery(&admin, &key_id);
    let recovered = client.get_key_version(&key_id, &version).unwrap();

    assert_eq!(version, 2);
    assert_eq!(recovered.key_bytes, replacement);
}

#[test]
fn initiate_recovery_surfaces_external_guardian_lookup_failure_without_side_effects() {
    let (env, client, admin, identity_id, guardian1, _guardian2, _outsider) = setup();
    let key_id = create_master_key(&env, &client, &admin, 11);
    let replacement = BytesN::from_array(&env, &[12u8; 32]);

    client.set_identity_contract(&admin, &identity_id);
    let identity = MockIdentityContractClient::new(&env, &identity_id);
    identity.configure(&Vec::new(&env), &0, &true, &false);

    let failed = client.try_initiate_recovery(&guardian1, &key_id, &replacement);
    assert!(matches!(failed, Err(Err(_))));

    let mut guardians = Vec::new(&env);
    guardians.push_back(guardian1.clone());
    identity.configure(&guardians, &1, &false, &false);

    client.initiate_recovery(&guardian1, &key_id, &replacement);

    let duplicate = client.try_initiate_recovery(&guardian1, &key_id, &replacement);
    assert_eq!(duplicate, Err(Ok(ContractError::RecoveryAlreadyActive)));
}

#[test]
fn execute_recovery_surfaces_external_threshold_failure_and_recovers_cleanly() {
    let (env, client, admin, identity_id, guardian1, guardian2, _outsider) = setup();
    let key_id = create_master_key(&env, &client, &admin, 21);
    let replacement = BytesN::from_array(&env, &[22u8; 32]);

    client.initiate_recovery(&guardian1, &key_id, &replacement);
    client.approve_recovery(&guardian2, &key_id);

    let now = env.ledger().timestamp();
    env.ledger().set_timestamp(now + 86_401);

    let identity = MockIdentityContractClient::new(&env, &identity_id);
    let mut guardians = Vec::new(&env);
    guardians.push_back(guardian1);
    guardians.push_back(guardian2);
    identity.configure(&guardians, &2, &false, &true);

    let failed = client.try_execute_recovery(&admin, &key_id);
    assert!(matches!(failed, Err(Err(_))));

    identity.configure(&guardians, &2, &false, &false);

    let version = client.execute_recovery(&admin, &key_id);
    let recovered = client.get_key_version(&key_id, &version).unwrap();

    assert_eq!(version, 2);
    assert_eq!(recovered.key_bytes, replacement);
}

#[test]
fn use_key_invariants_remain_intact_after_cross_contract_recovery_failures() {
    let (env, client, admin, identity_id, guardian1, guardian2, _outsider) = setup();
    let key_id = create_master_key(&env, &client, &admin, 31);
    let replacement = BytesN::from_array(&env, &[32u8; 32]);

    client.initiate_recovery(&guardian1, &key_id, &replacement);
    client.approve_recovery(&guardian2, &key_id);

    let now = env.ledger().timestamp();
    env.ledger().set_timestamp(now + 86_401);

    let identity = MockIdentityContractClient::new(&env, &identity_id);
    let mut guardians = Vec::new(&env);
    guardians.push_back(guardian1);
    guardians.push_back(guardian2);
    identity.configure(&guardians, &2, &false, &true);

    assert!(matches!(client.try_execute_recovery(&admin, &key_id), Err(Err(_))));

    let current = client.use_key(&admin, &key_id, &symbol_short!("ENC"));
    assert_eq!(current, BytesN::from_array(&env, &[31u8; 32]));
}
