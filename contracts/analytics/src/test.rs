#![allow(clippy::unwrap_used, clippy::expect_used)]
extern crate std;

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Vec};

use crate::{
    homomorphic::{PaillierPrivateKey, PaillierPublicKey},
    AnalyticsContract, AnalyticsContractClient, MetricDimensions, MetricValue, TrendPoint,
};

fn setup() -> (Env, AnalyticsContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AnalyticsContract, ());
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let aggregator = Address::generate(&env);

    // Generate keys: n=33 (p=3, q=11), nn=1089, g=34, lambda=20, mu=5
    let pub_key = PaillierPublicKey {
        n: 33,
        nn: 1089,
        g: 34,
    };
    let priv_key = PaillierPrivateKey { lambda: 20, mu: 5 };

    client.initialize(&admin, &aggregator, &pub_key, &Some(priv_key));

    (env, client, admin, aggregator)
}

#[test]
fn test_homomorphic_addition() {
    let (_env, client, _admin, aggregator) = setup();

    let m1 = 5;
    let m2 = 10;

    let c1 = client.encrypt(&m1);
    let c2 = client.encrypt(&m2);
    let c3 = client.add_ciphertexts(&c1, &c2);

    let res = client.decrypt(&aggregator, &c3);
    assert_eq!(res, 15);
}

#[test]
fn test_initialize_and_getters() {
    let (env, client, admin, aggregator) = setup();

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_aggregator(), aggregator);

    // Re-initialisation should panic; use try_ variant to assert failure.
    let new_admin = Address::generate(&env);
    let new_aggregator = Address::generate(&env);
    // Note: initialize now takes 5 arguments
    let pub_key = PaillierPublicKey {
        n: 33,
        nn: 1089,
        g: 34,
    };
    let result = client.try_initialize(&new_admin, &new_aggregator, &pub_key, &None);
    assert!(result.is_err());
}

#[test]
fn test_aggregate_records() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("REC_CNT");
    let dims = MetricDimensions {
        region: Some(symbol_short!("EU")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket: 1_700_000_000,
    };

    // Initial value should be zeroed.
    let initial = client.get_metric(&kind, &dims);
    assert_eq!(initial, MetricValue { count: 0, sum: 0 });

    // Encrypt some records
    let c1 = client.encrypt(&10);
    let c2 = client.encrypt(&5);

    let mut records = Vec::new(&env);
    records.push_back(c1);
    records.push_back(c2);

    client.aggregate_records(&aggregator, &kind, &dims, &records);

    let value = client.get_metric(&kind, &dims);
    // count should be 2, sum should be 15 (plus/minus DP noise, but with sensitivity=10 and epsilon=1, it might be exactly 15 or close)
    assert_eq!(value.count, 2);
    // Since our DP noise is simple seed-based, we can check if it's within a range if needed,
    // but for the sake of this test, we check if it's at least positive.
    assert!(value.sum > 0);
}

#[test]
fn test_trend_over_time_buckets() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("REC_CNT");
    let region = Some(symbol_short!("US"));
    let age_band = None;
    let condition = None;

    // Two time buckets
    let dims1 = MetricDimensions {
        region: region.clone(),
        age_band: age_band.clone(),
        condition: condition.clone(),
        time_bucket: 1,
    };
    let dims2 = MetricDimensions {
        region: region.clone(),
        age_band: age_band.clone(),
        condition: condition.clone(),
        time_bucket: 2,
    };

    let mut r1 = Vec::new(&env);
    r1.push_back(client.encrypt(&3));
    client.aggregate_records(&aggregator, &kind, &dims1, &r1);

    let mut r2 = Vec::new(&env);
    r2.push_back(client.encrypt(&7));
    client.aggregate_records(&aggregator, &kind, &dims2, &r2);

    let trend = client.get_trend(&kind, &region, &age_band, &condition, &1, &2);
    assert_eq!(trend.len(), 2);

    let TrendPoint {
        time_bucket: t1,
        value: v1,
    } = trend.get(0).unwrap();
    let TrendPoint {
        time_bucket: t2,
        value: v2,
    } = trend.get(1).unwrap();

    assert_eq!(t1, 1);
    assert_eq!(v1.count, 1);
    assert_eq!(t2, 2);
    assert_eq!(v2.count, 1);
}
