extern crate std;

use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, BytesN, Env};

use crate::{VaultContract, VaultContractClient};

#[test]
fn test_initialize_and_deadman_guard() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultContract, ());
    let client = VaultContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let identity = Address::generate(&env);
    client.initialize(&admin, &identity);

    let owner = Address::generate(&env);
    let res = client.try_trigger_deadman_release(&owner);
    assert!(res.is_err());

    env.ledger().set_timestamp(1_000);
    let bytes = BytesN::from_array(&env, &[3u8; 32]);
    let _ = bytes;
}
