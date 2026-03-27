#![allow(clippy::unwrap_used, clippy::expect_used)]

extern crate std;

use analytics::{
    homomorphic::{PaillierPrivateKey, PaillierPublicKey},
    AnalyticsContract, AnalyticsContractClient, ContractError, MetricDimensions, MetricValue,
};
use soroban_sdk::{symbol_short, testutils::Address as _, testutils::Ledger as _, Address, Env, Vec};

fn setup() -> (Env, AnalyticsContractClient<'static>, Address) {
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

    (env, client, aggregator)
}

fn encrypted_records(env: &Env, client: &AnalyticsContractClient) -> Vec<i128> {
    let mut records = Vec::new(env);
    records.push_back(client.encrypt(&4));
    records.push_back(client.encrypt(&6));
    records
}

fn dims() -> MetricDimensions {
    MetricDimensions {
        region: Some(symbol_short!("NG")),
        age_band: Some(symbol_short!("A18_39")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket: 1_710_000_000,
    }
}

#[test]
fn test_aggregate_records_in_window_rejects_before_timelock() {
    let (env, client, aggregator) = setup();
    env.ledger().set_timestamp(1_000);

    let kind = symbol_short!("REC_CNT");
    let dims = dims();
    let records = encrypted_records(&env, &client);

    assert_eq!(
        client.try_aggregate_records_in_window(&aggregator, &kind, &dims, &records, &1_050, &1_100),
        Err(Ok(ContractError::TimelockNotMet))
    );
    assert_eq!(client.get_metric(&kind, &dims), MetricValue { count: 0, sum: 0 });
}

#[test]
fn test_aggregate_records_in_window_allows_execution_at_window_start() {
    let (env, client, aggregator) = setup();
    env.ledger().set_timestamp(1_050);

    let kind = symbol_short!("REC_CNT");
    let dims = dims();
    let records = encrypted_records(&env, &client);

    client.aggregate_records_in_window(&aggregator, &kind, &dims, &records, &1_050, &1_100);

    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 2);
    assert!(metric.sum > 0);
}

#[test]
fn test_aggregate_records_in_window_rejects_after_expiry() {
    let (env, client, aggregator) = setup();
    env.ledger().set_timestamp(1_101);

    let kind = symbol_short!("REC_CNT");
    let dims = dims();
    let records = encrypted_records(&env, &client);

    assert_eq!(
        client.try_aggregate_records_in_window(&aggregator, &kind, &dims, &records, &1_050, &1_100),
        Err(Ok(ContractError::SubmissionExpired))
    );
    assert_eq!(client.get_metric(&kind, &dims), MetricValue { count: 0, sum: 0 });
}

#[test]
fn test_aggregate_records_in_window_respects_advanced_ledger_time() {
    let (env, client, aggregator) = setup();

    let kind = symbol_short!("REC_CNT");
    let dims = dims();
    let records = encrypted_records(&env, &client);

    env.ledger().set_timestamp(1_049);
    assert_eq!(
        client.try_aggregate_records_in_window(&aggregator, &kind, &dims, &records, &1_050, &1_100),
        Err(Ok(ContractError::TimelockNotMet))
    );

    env.ledger().set_timestamp(1_075);
    client.aggregate_records_in_window(&aggregator, &kind, &dims, &records, &1_050, &1_100);
    assert_eq!(client.get_metric(&kind, &dims).count, 2);

    env.ledger().set_timestamp(1_101);
    assert_eq!(
        client.try_aggregate_records_in_window(&aggregator, &kind, &dims, &records, &1_050, &1_100),
        Err(Ok(ContractError::SubmissionExpired))
    );
    assert_eq!(client.get_metric(&kind, &dims).count, 2);
}
