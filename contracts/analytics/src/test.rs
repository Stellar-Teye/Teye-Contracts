#![allow(clippy::unwrap_used, clippy::expect_used)]
extern crate std;

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

use crate::{
    AnalyticsContract, AnalyticsContractClient, MetricDimensions, MetricValue, TrendPoint,
};

fn setup() -> (Env, AnalyticsContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AnalyticsContract, ());
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let aggregator = Address::generate(&env);

    client.initialize(&admin, &aggregator);

    (env, client, admin, aggregator)
}

#[test]
fn test_initialize_and_getters() {
    let (env, client, admin, aggregator) = setup();

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_aggregator(), aggregator);

    // Re-initialisation should panic; use try_ variant to assert failure.
    let new_admin = Address::generate(&env);
    let new_aggregator = Address::generate(&env);
    let result = client.try_initialize(&new_admin, &new_aggregator);
    assert!(result.is_err());
}

#[test]
fn test_record_and_get_metric() {
    let (_env, client, _admin, aggregator) = setup();

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

    // Record two contributions.
    client.record_metric(&aggregator, &kind, &dims, &10, &100);
    client.record_metric(&aggregator, &kind, &dims, &5, &50);

    let value = client.get_metric(&kind, &dims);
    assert_eq!(value.count, 15);
    assert_eq!(value.sum, 150);
}

#[test]
fn test_trend_over_time_buckets() {
    let (_env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("REC_CNT");
    let region = Some(symbol_short!("US"));
    let age_band = None;
    let condition = None;

    // Two time buckets with different values.
    let dims_day1 = MetricDimensions {
        region: region.clone(),
        age_band: age_band.clone(),
        condition: condition.clone(),
        time_bucket: 1,
    };
    let dims_day2 = MetricDimensions {
        region: region.clone(),
        age_band: age_band.clone(),
        condition: condition.clone(),
        time_bucket: 2,
    };

    client.record_metric(&aggregator, &kind, &dims_day1, &3, &0);
    client.record_metric(&aggregator, &kind, &dims_day2, &7, &0);

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
    assert_eq!(v1.count, 3);
    assert_eq!(t2, 2);
    assert_eq!(v2.count, 7);
}

#[test]
fn test_population_metrics_for_anonymous_bucket() {
    let (_env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("VIS_CNT");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 42,
    };

    client.record_metric(&aggregator, &kind, &dims, &100, &500);

    let total = client.get_population_metrics(&kind, &42);
    assert_eq!(total.count, 100);
    assert_eq!(total.sum, 500);
}
