#![allow(clippy::unwrap_used, clippy::expect_used)]

extern crate std;

use analytics::{
    homomorphic::{PaillierPrivateKey, PaillierPublicKey},
    AnalyticsContract, AnalyticsContractClient, ContractError,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn sample_pub_key() -> PaillierPublicKey {
    PaillierPublicKey {
        n: 33,
        nn: 1089,
        g: 34,
    }
}

fn sample_priv_key() -> PaillierPrivateKey {
    PaillierPrivateKey { lambda: 20, mu: 5 }
}

#[test]
fn test_initialize_rejects_second_call_with_already_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AnalyticsContract, ());
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let aggregator = Address::generate(&env);

    client.initialize(&admin, &aggregator, &sample_pub_key(), &Some(sample_priv_key()));

    let attacker_admin = Address::generate(&env);
    let attacker_aggregator = Address::generate(&env);
    assert_eq!(
        client.try_initialize(
            &attacker_admin,
            &attacker_aggregator,
            &sample_pub_key(),
            &Some(sample_priv_key())
        ),
        Err(Ok(ContractError::AlreadyInitialized))
    );

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_aggregator(), aggregator);
}

#[test]
fn test_initialize_rejects_second_call_even_with_different_key_material() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AnalyticsContract, ());
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let aggregator = Address::generate(&env);
    client.initialize(&admin, &aggregator, &sample_pub_key(), &Some(sample_priv_key()));

    let replacement_pub_key = PaillierPublicKey {
        n: 35,
        nn: 1225,
        g: 36,
    };
    let replacement_priv_key = PaillierPrivateKey { lambda: 24, mu: 6 };

    assert_eq!(
        client.try_initialize(
            &Address::generate(&env),
            &Address::generate(&env),
            &replacement_pub_key,
            &Some(replacement_priv_key)
        ),
        Err(Ok(ContractError::AlreadyInitialized))
    );

    let ciphertext = client.encrypt(&5);
    assert_eq!(client.decrypt(&aggregator, &ciphertext), 5);
}
