#![allow(clippy::unwrap_used, clippy::expect_used)]

extern crate std;

use analytics::{
    homomorphic::{PaillierPrivateKey, PaillierPublicKey},
    AnalyticsContract, AnalyticsContractClient, ContractError, MetricDimensions, MetricValue,
};
use soroban_sdk::{symbol_short, testutils::Address as _, testutils::Ledger as _, Address, Env, Vec};

fn setup() -> (Env, AnalyticsContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AnalyticsContract, ());
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let aggregator = Address::generate(&env);

    let pub_key = PaillierPublicKey {
        n: 33,
        nn: 1089,
        g: 34,
    };
    let priv_key = PaillierPrivateKey { lambda: 20, mu: 5 };

    client.initialize(&admin, &aggregator, &pub_key, &Some(priv_key));

    (env, client, admin, aggregator)
}

fn dims() -> MetricDimensions {
    MetricDimensions {
        region: Some(symbol_short!("NG")),
        age_band: None,
        condition: None,
        time_bucket: 123,
    }
}

#[test]
fn test_initialize_rejects_zero_public_key_material() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AnalyticsContract, ());
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let aggregator = Address::generate(&env);
    let zero_pub_key = PaillierPublicKey { n: 0, nn: 0, g: 0 };

    assert_eq!(
        client.try_initialize(&admin, &aggregator, &zero_pub_key, &None),
        Err(Ok(ContractError::InvalidInput))
    );
}

#[test]
fn test_initialize_rejects_zero_private_key_material() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AnalyticsContract, ());
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let aggregator = Address::generate(&env);
    let pub_key = PaillierPublicKey {
        n: 33,
        nn: 1089,
        g: 34,
    };
    let zero_priv_key = PaillierPrivateKey { lambda: 0, mu: 0 };

    assert_eq!(
        client.try_initialize(&admin, &aggregator, &pub_key, &Some(zero_priv_key)),
        Err(Ok(ContractError::InvalidInput))
    );
}

#[test]
fn test_aggregate_records_rejects_empty_ciphertext_vector() {
    let (env, client, _admin, aggregator) = setup();
    let kind = symbol_short!("REC_CNT");
    let dims = dims();
    let empty_records = Vec::new(&env);

    assert_eq!(
        client.try_aggregate_records(&aggregator, &kind, &dims, &empty_records),
        Err(Ok(ContractError::InvalidInput))
    );
    assert_eq!(client.get_metric(&kind, &dims), MetricValue { count: 0, sum: 0, version: 0 });
}

#[test]
fn test_aggregate_records_in_window_rejects_empty_ciphertext_vector() {
    let (env, client, _admin, aggregator) = setup();
    env.ledger().set_timestamp(10);

    let kind = symbol_short!("REC_CNT");
    let dims = dims();
    let empty_records = Vec::new(&env);

    assert_eq!(
        client.try_aggregate_records_in_window(&aggregator, &kind, &dims, &empty_records, &5, &20),
        Err(Ok(ContractError::InvalidInput))
    );
    assert_eq!(client.get_metric(&kind, &dims), MetricValue { count: 0, sum: 0, version: 0 });
}

#[test]
fn test_aggregate_records_in_window_with_zero_bounds_expires_immediately() {
    let (env, client, _admin, aggregator) = setup();
    env.ledger().set_timestamp(1);

    let kind = symbol_short!("REC_CNT");
    let dims = dims();
    let mut records = Vec::new(&env);
    records.push_back(client.encrypt(&0));

    assert_eq!(
        client.try_aggregate_records_in_window(&aggregator, &kind, &dims, &records, &0, &0),
        Err(Ok(ContractError::SubmissionExpired))
    );
    assert_eq!(client.get_metric(&kind, &dims), MetricValue { count: 0, sum: 0 });
}
