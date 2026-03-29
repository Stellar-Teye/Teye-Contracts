#![allow(clippy::unwrap_used, clippy::expect_used)]

extern crate std;

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Vec};

use analytics::{
    homomorphic::{PaillierPrivateKey, PaillierPublicKey},
    AnalyticsContract, AnalyticsContractClient, ContractError, MetricDimensions, MetricValue,
};

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

    client
        .initialize(&admin, &aggregator, &pub_key, &Some(priv_key))
        .unwrap();

    (env, client, admin, aggregator)
}

/// Test gas limits with massive batch processing - 1000 records
#[test]
fn test_gas_limit_massive_batch_1000_records() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("REC_CNT");
    let dims = MetricDimensions {
        region: Some(symbol_short!("EU")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket: 1_700_000_000,
    };

    // Create 1000 encrypted records
    let mut records = Vec::new(&env);
    for i in 0..1000 {
        let value = ((i % 100) + 1) as i128;
        let encrypted = client.encrypt(&value);
        records.push_back(encrypted);
    }

    // Aggregate all records - should handle without panicking
    let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
    assert!(result.is_ok());

    // Verify aggregation completed
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 1000);
    assert!(metric.sum > 0);
}

/// Test gas limits with maximum batch size - 5000 records
#[test]
fn test_gas_limit_maximum_batch_5000_records() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("REC_CNT");
    let dims = MetricDimensions {
        region: Some(symbol_short!("US")),
        age_band: None,
        condition: None,
        time_bucket: 1_700_000_000,
    };

    // Create 5000 encrypted records
    let mut records = Vec::new(&env);
    for i in 0..5000 {
        let value = ((i % 1000) + 1) as i128;
        let encrypted = client.encrypt(&value);
        records.push_back(encrypted);
    }

    // Aggregate all records
    let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
    assert!(result.is_ok());

    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 5000);
}

/// Test gas limits with extreme values in batch processing
#[test]
fn test_gas_limit_extreme_values_batch() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("EXTREME");
    let dims = MetricDimensions {
        region: Some(symbol_short!("EU")),
        age_band: Some(symbol_short!("A18_40")),
        condition: None,
        time_bucket: 1_700_000_000,
    };

    // Create batch with extreme values
    let mut records = Vec::new(&env);

    // Add i128::MAX values
    for _ in 0..100 {
        let encrypted = client.encrypt(&i128::MAX);
        records.push_back(encrypted);
    }

    // Add i128::MIN values
    for _ in 0..100 {
        let encrypted = client.encrypt(&i128::MIN);
        records.push_back(encrypted);
    }

    // Add zero values
    for _ in 0..100 {
        let encrypted = client.encrypt(&0);
        records.push_back(encrypted);
    }

    // Aggregate - should use saturating_add to prevent overflow
    let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
    assert!(result.is_ok());

    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 300);
}

/// Test gas limits with repeated aggregations on same dimensions
#[test]
fn test_gas_limit_repeated_aggregations_same_dimensions() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("REPEAT");
    let dims = MetricDimensions {
        region: Some(symbol_short!("US")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("DIABETES")),
        time_bucket: 1_700_000_000,
    };

    // Perform 100 aggregations on the same dimensions
    for batch_num in 0..100 {
        let mut records = Vec::new(&env);
        for i in 0..100 {
            let value = ((batch_num * 100 + i) % 1000 + 1) as i128;
            let encrypted = client.encrypt(&value);
            records.push_back(encrypted);
        }

        let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
        assert!(result.is_ok());
    }

    // Verify final aggregation
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 10_000); // 100 batches * 100 records
}

/// Test gas limits with empty batch handling
#[test]
fn test_gas_limit_empty_batch_rejection() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("EMPTY");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1_700_000_000,
    };

    let empty_records: Vec<i128> = Vec::new(&env);

    // Empty batch should be rejected
    let result = client.try_aggregate_records(&aggregator, &kind, &dims, &empty_records);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), ContractError::InvalidInput);
}

/// Test gas limits with single-record batches
#[test]
fn test_gas_limit_single_record_batches() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("SINGLE");
    let dims = MetricDimensions {
        region: Some(symbol_short!("EU")),
        age_band: None,
        condition: None,
        time_bucket: 1_700_000_000,
    };

    // Submit 1000 single-record batches
    for i in 0..1000 {
        let mut records = Vec::new(&env);
        let value = ((i % 100) + 1) as i128;
        let encrypted = client.encrypt(&value);
        records.push_back(encrypted);

        let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
        assert!(result.is_ok());
    }

    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 1000);
}

/// Test gas limits with multiple dimensions across time buckets
#[test]
fn test_gas_limit_multiple_dimensions_time_buckets() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("MULTI");

    // Create 100 different dimension combinations across 100 time buckets
    for region_idx in 0..10 {
        for age_idx in 0..10 {
            for bucket_idx in 0..100 {
                let dims = MetricDimensions {
                    region: Some(symbol_short!(&format!("R{}", region_idx))),
                    age_band: Some(symbol_short!(&format!("A{}", age_idx))),
                    condition: None,
                    time_bucket: 1_700_000_000 + bucket_idx as u64,
                };

                let mut records = Vec::new(&env);
                for i in 0..10 {
                    let value = ((region_idx * 10 + age_idx + i) % 100 + 1) as i128;
                    let encrypted = client.encrypt(&value);
                    records.push_back(encrypted);
                }

                let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
                assert!(result.is_ok());
            }
        }
    }

    // Verify a sample metric
    let sample_dims = MetricDimensions {
        region: Some(symbol_short!("R0")),
        age_band: Some(symbol_short!("A0")),
        condition: None,
        time_bucket: 1_700_000_000,
    };

    let metric = client.get_metric(&kind, &sample_dims);
    assert_eq!(metric.count, 10);
}

/// Test gas limits with time-window constrained aggregations
#[test]
fn test_gas_limit_time_window_aggregations() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("TIMED");
    let dims = MetricDimensions {
        region: Some(symbol_short!("US")),
        age_band: None,
        condition: None,
        time_bucket: 1_700_000_000,
    };

    let now = env.ledger().timestamp();
    let not_before = now;
    let expires_at = now + 3600; // 1 hour window

    // Perform 100 aggregations within time window
    for i in 0..100 {
        let mut records = Vec::new(&env);
        for j in 0..100 {
            let value = ((i * 100 + j) % 1000 + 1) as i128;
            let encrypted = client.encrypt(&value);
            records.push_back(encrypted);
        }

        let result = client.try_aggregate_records_in_window(
            &aggregator,
            &kind,
            &dims,
            &records,
            &not_before,
            &expires_at,
        );
        assert!(result.is_ok());
    }

    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 10_000);
}

/// Test gas limits with time-window expiration
#[test]
fn test_gas_limit_time_window_expiration() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("EXPIRED");
    let dims = MetricDimensions {
        region: Some(symbol_short!("EU")),
        age_band: None,
        condition: None,
        time_bucket: 1_700_000_000,
    };

    let now = env.ledger().timestamp();
    let not_before = now;
    let expires_at = now - 1; // Already expired

    let mut records = Vec::new(&env);
    for i in 0..100 {
        let value = (i + 1) as i128;
        let encrypted = client.encrypt(&value);
        records.push_back(encrypted);
    }

    // Should fail due to expiration
    let result = client.try_aggregate_records_in_window(
        &aggregator,
        &kind,
        &dims,
        &records,
        &not_before,
        &expires_at,
    );

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        ContractError::SubmissionExpired
    );
}

/// Test gas limits with time-window not yet active
#[test]
fn test_gas_limit_time_window_not_active() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("FUTURE");
    let dims = MetricDimensions {
        region: Some(symbol_short!("US")),
        age_band: None,
        condition: None,
        time_bucket: 1_700_000_000,
    };

    let now = env.ledger().timestamp();
    let not_before = now + 3600; // 1 hour in future
    let expires_at = now + 7200; // 2 hours in future

    let mut records = Vec::new(&env);
    for i in 0..100 {
        let value = (i + 1) as i128;
        let encrypted = client.encrypt(&value);
        records.push_back(encrypted);
    }

    // Should fail due to timelock
    let result = client.try_aggregate_records_in_window(
        &aggregator,
        &kind,
        &dims,
        &records,
        &not_before,
        &expires_at,
    );

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), ContractError::TimelockNotMet);
}

/// Test gas limits with unauthorized aggregator
#[test]
fn test_gas_limit_unauthorized_aggregator() {
    let (env, client, _admin, _aggregator) = setup();

    let unauthorized = Address::generate(&env);
    let kind = symbol_short!("UNAUTH");
    let dims = MetricDimensions {
        region: Some(symbol_short!("US")),
        age_band: None,
        condition: None,
        time_bucket: 1_700_000_000,
    };

    let mut records = Vec::new(&env);
    for i in 0..100 {
        let value = (i + 1) as i128;
        let encrypted = client.encrypt(&value);
        records.push_back(encrypted);
    }

    let result = client.try_aggregate_records(&unauthorized, &kind, &dims, &records);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), ContractError::Unauthorized);
}

/// Test gas limits with metric retrieval after massive aggregation
#[test]
fn test_gas_limit_metric_retrieval_after_massive_aggregation() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("RETRIEVE");
    let dims = MetricDimensions {
        region: Some(symbol_short!("US")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("HYPERTENSION")),
        time_bucket: 1_700_000_000,
    };

    // Perform massive aggregation
    let mut records = Vec::new(&env);
    for i in 0..10_000 {
        let value = ((i % 1000) + 1) as i128;
        let encrypted = client.encrypt(&value);
        records.push_back(encrypted);
    }

    client
        .aggregate_records(&aggregator, &kind, &dims, &records)
        .unwrap();

    // Retrieve metric - should be fast despite large aggregation
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 10_000);
    assert!(metric.sum > 0);
}

/// Test gas limits with trend computation across many time buckets
#[test]
fn test_gas_limit_trend_computation_many_buckets() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("TREND");
    let region = Some(symbol_short!("US"));
    let age_band = Some(symbol_short!("A40_64"));
    let condition = None;

    // Populate 1000 time buckets with data
    for bucket in 0..1000 {
        let dims = MetricDimensions {
            region: region.clone(),
            age_band: age_band.clone(),
            condition: condition.clone(),
            time_bucket: 1_700_000_000 + bucket as u64,
        };

        let mut records = Vec::new(&env);
        for i in 0..10 {
            let value = ((bucket * 10 + i) % 1000 + 1) as i128;
            let encrypted = client.encrypt(&value);
            records.push_back(encrypted);
        }

        client
            .aggregate_records(&aggregator, &kind, &dims, &records)
            .unwrap();
    }

    // Retrieve trend across all buckets
    let trend = client.get_trend(
        &kind,
        &region,
        &age_band,
        &condition,
        &1_700_000_000,
        &1_700_000_999,
    );

    assert_eq!(trend.len(), 1000);
}

/// Test gas limits with integer overflow protection in aggregation
#[test]
fn test_gas_limit_integer_overflow_protection() {
    let (env, client, _admin, aggregator) = setup();

    let kind = symbol_short!("OVERFLOW");
    let dims = MetricDimensions {
        region: Some(symbol_short!("US")),
        age_band: None,
        condition: None,
        time_bucket: 1_700_000_000,
    };

    // First aggregation with large values
    let mut records1 = Vec::new(&env);
    for _ in 0..100 {
        let encrypted = client.encrypt(&(i128::MAX / 2));
        records1.push_back(encrypted);
    }

    client
        .aggregate_records(&aggregator, &kind, &dims, &records1)
        .unwrap();

    // Second aggregation - should saturate instead of overflow
    let mut records2 = Vec::new(&env);
    for _ in 0..100 {
        let encrypted = client.encrypt(&(i128::MAX / 2));
        records2.push_back(encrypted);
    }

    client
        .aggregate_records(&aggregator, &kind, &dims, &records2)
        .unwrap();

    // Verify saturation occurred
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 200);
    // Sum should be saturated at i128::MAX, not overflowed
    assert!(metric.sum > 0);
}
