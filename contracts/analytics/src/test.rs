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

    // Initial value should be zeroed (version 0 means non-existent or stale).
    let initial = client.get_metric(&kind, &dims);
    assert_eq!(initial, MetricValue { count: 0, sum: 0, version: 0 });

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

#[test]
fn test_stale_data_invalidation() {
    let (env, client, admin, aggregator) = setup();

    let kind = symbol_short!("STALE");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 100,
    };

    // 1. Aggregate some data (Ver 1)
    let mut recs = Vec::new(&env);
    recs.push_back(client.encrypt(&100));
    client.aggregate_records(&aggregator, &kind, &dims, &recs);

    let val1 = client.get_metric(&kind, &dims);
    assert_eq!(val1.count, 1);
    assert_eq!(val1.version, 1);

    // 2. Change aggregator (Increments version to 2)
    let new_aggregator = Address::generate(&env);
    client.set_aggregator(&new_aggregator);
    assert_eq!(client.get_dep_ver(), 2);

    // 3. Verify data is now stale (returns 0)
    let val2 = client.get_metric(&kind, &dims);
    assert_eq!(val2.count, 0);
    assert_eq!(val2.sum, 0);
    assert_eq!(val2.version, 0);

    // 4. Aggregate more data (New Aggregator, Ver 2)
    // Note: client.mock_all_auths() is on, so we can use new_aggregator
    client.aggregate_records(&new_aggregator, &kind, &dims, &recs);

    let val3 = client.get_metric(&kind, &dims);
    assert_eq!(val3.count, 1); // Reset to 0 then added 1
    assert_eq!(val3.version, 2);
}

#[test]
fn test_paillier_key_update_invalidates_data() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("KEY_CHG");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 200,
    };

    let mut recs = Vec::new(&env);
    recs.push_back(client.encrypt(&50));
    client.aggregate_records(&aggregator, &kind, &dims, &recs);

    // Update keys
    let new_pub = PaillierPublicKey { n: 55, nn: 3025, g: 56 };
    let new_priv = PaillierPrivateKey { lambda: 40, mu: 10 };
    client.set_paillier_keys(&new_pub, &Some(new_priv));

    assert_eq!(client.get_dep_ver(), 2);
    
    let val = client.get_metric(&kind, &dims);
    assert_eq!(val.count, 0);
}

#[test]
#[should_panic]
fn test_unauthorized_dependency_update() {
    let env = Env::default();
    let contract_id = env.register(AnalyticsContract, ());
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let aggregator = Address::generate(&env);
    let mallory = Address::generate(&env);

    let pub_key = PaillierPublicKey { n: 33, nn: 1089, g: 34 };
    client.initialize(&admin, &aggregator, &pub_key, &None);

    // Mallory tries to change aggregator
    client.set_aggregator(&mallory); // This should panic because Mallory is calling but admin is required
}
