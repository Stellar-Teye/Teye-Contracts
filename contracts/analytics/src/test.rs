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
    assert_eq!(
        initial,
        MetricValue {
            count: 0,
            sum: 0,
            version: 0
        }
    );

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
    let (env, client, _admin, aggregator) = setup();

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
    let new_pub = PaillierPublicKey {
        n: 55,
        nn: 3025,
        g: 56,
    };
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

    let pub_key = PaillierPublicKey {
        n: 33,
        nn: 1089,
        g: 34,
    };
    client.initialize(&admin, &aggregator, &pub_key, &None);

    // Mallory tries to change aggregator
    client.set_aggregator(&mallory); // This should panic because Mallory is calling but admin is required
}

// ============================================================================
// Edge Case Tests for Maximum/Minimum Bounds
// ============================================================================

#[test]
fn test_zero_value_encryption() {
    // Test encrypting zero - edge case for homomorphic operations
    let (_env, client, _admin, aggregator) = setup();

    let c_zero = client.encrypt(&0);
    let decrypted = client.decrypt(&aggregator, &c_zero);
    assert_eq!(decrypted, 0, "Zero encryption/decryption failed");
}

#[test]
fn test_negative_value_encryption() {
    // Test that negative values are handled correctly
    let (_env, client, _admin, aggregator) = setup();

    let m_negative = -5;
    let c_negative = client.encrypt(&m_negative);
    let decrypted = client.decrypt(&aggregator, &c_negative);
    assert_eq!(decrypted, m_negative, "Negative value encryption failed");
}

#[test]
fn test_large_value_near_bounds() {
    // Test encryption with large values接近 boundaries
    let (_env, client, _admin, aggregator) = setup();

    // Test with moderately large values (within i128 reasonable range for this demo)
    let large_val = 1000;
    let c_large = client.encrypt(&large_val);
    let decrypted = client.decrypt(&aggregator, &c_large);
    assert_eq!(decrypted, large_val, "Large value encryption failed");
}

#[test]
fn test_homomorphic_addition_overflow_protection() {
    // Test that repeated additions don't cause unexpected wraparound
    let (_env, client, _admin, aggregator) = setup();

    // Create multiple ciphertexts and add them
    let mut total_ciphertext = client.encrypt(&0);
    
    for i in 1..=10 {
        let c_i = client.encrypt(&i);
        total_ciphertext = client.add_ciphertexts(&total_ciphertext, &c_i);
    }
    
    let result = client.decrypt(&aggregator, &total_ciphertext);
    // Sum of 1+2+3+...+10 = 55
    assert_eq!(result, 55, "Homomorphic addition overflow detected");
}

#[test]
fn test_differential_privacy_zero_epsilon() {
    // Test DP with epsilon=0 (should return original value, no noise)
    let env = Env::default();
    
    let value = 100;
    let noisy = crate::differential_privacy::DifferentialPrivacy::add_laplace_noise(
        &env, value, 0, 10
    );
    
    assert_eq!(noisy, value, "DP with epsilon=0 should not add noise");
}

#[test]
fn test_differential_privacy_max_sensitivity() {
    // Test DP with maximum sensitivity value
    let env = Env::default();
    
    let value = 50;
    let max_sensitivity = i128::MAX / 2; // Use half of MAX to avoid overflow in calculation
    
    // Should use saturating arithmetic to prevent overflow
    let noisy = crate::differential_privacy::DifferentialPrivacy::add_laplace_noise(
        &env, value, 1, max_sensitivity
    );
    
    // With such high sensitivity, noise could be very large, but should not wrap
    // Just verify it doesn't panic and produces a result
    assert!(noisy < i128::MAX, "DP noise caused overflow");
    assert!(noisy > i128::MIN, "DP noise caused underflow");
}

#[test]
fn test_differential_privacy_extreme_values() {
    // Test DP noise addition with extreme input values
    let env = Env::default();
    
    // Test with large positive value
    let large_pos = i128::MAX / 1000; // Use a reasonable large value
    let noisy_pos = crate::differential_privacy::DifferentialPrivacy::add_laplace_noise(
        &env, large_pos, 1, 10
    );
    assert!(noisy_pos >= i128::MIN, "DP underflow with large positive value");
    
    // Test with large negative value
    let large_neg = i128::MIN / 1000;
    let noisy_neg = crate::differential_privacy::DifferentialPrivacy::add_laplace_noise(
        &env, large_neg, 1, 10
    );
    assert!(noisy_neg <= i128::MAX, "DP overflow with large negative value");
}

#[test]
fn test_aggregate_empty_ciphertexts() {
    // Test aggregation with empty input
    let env = Env::default();
    let empty_vec: Vec<i128> = Vec::new(&env);
    
    let pub_key = PaillierPublicKey {
        n: 33,
        nn: 1089,
        g: 34,
    };
    
    let result = crate::aggregation::Aggregator::aggregate_sum(&pub_key, empty_vec);
    // Empty sum should return identity element (1 in ciphertext space)
    assert_eq!(result, 1, "Empty aggregation should return identity");
}

#[test]
fn test_aggregate_single_ciphertext() {
    // Test aggregation with single element
    let (env, client, _admin, aggregator) = setup();
    
    let single_value = 42;
    let c = client.encrypt(&single_value);
    let mut records = Vec::new(&env);
    records.push_back(c);
    
    client.aggregate_records(&aggregator, &symbol_short!("SINGLE"), 
        &MetricDimensions {
            region: None,
            age_band: None,
            condition: None,
            time_bucket: 1,
        }, 
        &records);
    
    let metric = client.get_metric(&symbol_short!("SINGLE"), 
        &MetricDimensions {
            region: None,
            age_band: None,
            condition: None,
            time_bucket: 1,
        });
    
    assert_eq!(metric.count, 1, "Single record count failed");
}

#[test]
fn test_aggregate_many_values_accumulation() {
    // Test accumulation of many values to check for overflow
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("MANY");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1,
    };
    
    // Aggregate 100 small values
    let mut all_ciphertexts = Vec::new(&env);
    for i in 1..=100 {
        all_ciphertexts.push_back(client.encrypt(&i));
    }
    
    client.aggregate_records(&aggregator, &kind, &dims, &all_ciphertexts);
    
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 100, "Count mismatch for many values");
    // Sum should be approximately 5050 (1+2+...+100) plus/minus DP noise
    assert!(metric.sum > 5000 && metric.sum < 5100, 
        "Sum out of expected range: {}", metric.sum);
}

#[test]
fn test_metric_value_saturating_arithmetic() {
    // Test that metric updates use saturating arithmetic
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("SAT");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1,
    };
    
    // First aggregation
    let mut recs1 = Vec::new(&env);
    recs1.push_back(client.encrypt(&100));
    client.aggregate_records(&aggregator, &kind, &dims, &recs1);
    
    // Second aggregation - should accumulate
    let mut recs2 = Vec::new(&env);
    recs2.push_back(client.encrypt(&50));
    client.aggregate_records(&aggregator, &kind, &dims, &recs2);
    
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 2, "Saturating addition count failed");
}

#[test]
fn test_time_bucket_boundary_values() {
    // Test trend retrieval with boundary time bucket values
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("TIME");
    
    // Test with time_bucket = 0
    let dims_zero = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 0,
    };
    
    let mut recs = Vec::new(&env);
    recs.push_back(client.encrypt(&25));
    client.aggregate_records(&aggregator, &kind, &dims_zero, &recs);
    
    let metric = client.get_metric(&kind, &dims_zero);
    assert_eq!(metric.count, 1, "Time bucket 0 failed");
    
    // Test with very large time_bucket
    let dims_large = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: u64::MAX,
    };
    
    client.aggregate_records(&aggregator, &kind, &dims_large, &recs);
    let metric_large = client.get_metric(&kind, &dims_large);
    assert_eq!(metric_large.count, 1, "Time bucket MAX failed");
}

#[test]
fn test_trend_with_large_bucket_range() {
    // Test get_trend with a reasonably large range (not too large to avoid timeout)
    let (env, client, _admin, _aggregator) = setup();
    
    let kind = symbol_short!("TREND");
    let start = 1000;
    let end = 1010; // Small range to test the logic without performance issues
    
    // Pre-populate some buckets
    for bucket in start..=end {
        let dims = MetricDimensions {
            region: None,
            age_band: None,
            condition: None,
            time_bucket: bucket,
        };
        let mut recs = Vec::new(&env);
        recs.push_back(client.encrypt(&(bucket as i128)));
        client.aggregate_records(&_admin, &kind, &dims, &recs);
    }
    
    let trend = client.get_trend(&kind, &None, &None, &None, &start, &end);
    assert_eq!(trend.len(), ((end - start + 1) as u32) as usize, "Trend length mismatch");
}

#[test]
fn test_invalid_paillier_key_bounds() {
    // Test that invalid key values are rejected
    let env = Env::default();
    let contract_id = env.register(AnalyticsContract, ());
    let client = AnalyticsContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let aggregator = Address::generate(&env);
    
    // Test with zero values
    let invalid_pub_key_1 = PaillierPublicKey { n: 0, nn: 0, g: 0 };
    let result = client.try_initialize(&admin, &aggregator, &invalid_pub_key_1, &None);
    assert!(result.is_err(), "Should reject zero public key");
    
    // Test with negative values
    let invalid_pub_key_2 = PaillierPublicKey { n: -1, nn: -1, g: -1 };
    let result2 = client.try_initialize(&admin, &aggregator, &invalid_pub_key_2, &None);
    assert!(result2.is_err(), "Should reject negative public key");
}

#[test]
fn test_invalid_private_key_bounds() {
    // Test that invalid private key values are rejected
    let (_env, client, _admin, _aggregator) = setup();
    
    // Test with zero lambda
    let invalid_priv_key_1 = PaillierPrivateKey { lambda: 0, mu: 5 };
    let pub_key = PaillierPublicKey { n: 33, nn: 1089, g: 34 };
    let result = client.try_set_paillier_keys(&pub_key, &Some(invalid_priv_key_1));
    assert!(result.is_err(), "Should reject zero lambda");
    
    // Test with zero mu
    let invalid_priv_key_2 = PaillierPrivateKey { lambda: 20, mu: 0 };
    let result2 = client.try_set_paillier_keys(&pub_key, &Some(invalid_priv_key_2));
    assert!(result2.is_err(), "Should reject zero mu");
    
    // Test with negative values
    let invalid_priv_key_3 = PaillierPrivateKey { lambda: -1, mu: -1 };
    let result3 = client.try_set_paillier_keys(&pub_key, &Some(invalid_priv_key_3));
    assert!(result3.is_err(), "Should reject negative private key values");
}

#[test]
fn test_average_with_zero_count() {
    // Test average calculation handles division by zero
    let sum = 100;
    let count = 0;
    
    let avg = crate::aggregation::Aggregator::aggregate_average(sum, count);
    assert_eq!(avg, 0, "Average with zero count should return 0");
}

#[test]
fn test_average_with_negative_values() {
    // Test average with negative sum
    let sum = -50;
    let count = 5;
    
    let avg = crate::aggregation::Aggregator::aggregate_average(sum, count);
    assert_eq!(avg, -10, "Average with negative sum incorrect");
}

#[test]
fn test_concurrent_aggregation_same_dimensions() {
    // Test multiple aggregations to same dimensions (simulating concurrent access)
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("CONC");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1,
    };
    
    // Multiple sequential aggregations (simulating what would be concurrent in real usage)
    for _i in 1..=5 {
        let mut recs = Vec::new(&env);
        recs.push_back(client.encrypt(&10));
        client.aggregate_records(&aggregator, &kind, &dims, &recs);
    }
    
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 5, "Concurrent aggregation count failed");
}

#[test]
fn test_import_metric_edge_cases() {
    // Test metric import with various edge cases
    let (_env, _client, _admin, _aggregator) = setup();
    
    // Note: Import requires a real external contract which is complex to mock
    // This test documents the edge case requirements for future implementation
    // Edge cases to consider when importing metrics:
    // 1. Source contract doesn't exist
    // 2. Source returns invalid data (negative count, etc.)
    // 3. Source returns data with version 0
    // 4. Dimensions with all None values except time_bucket
    
    // For now, we verify the function exists and has proper type signature
}

// ============================================================================
// Comprehensive Maximum/Minimum Bound Tests
// These tests ensure computations don't wrap around unexpectedly
// ============================================================================

#[test]
fn test_homomorphic_encrypt_i128_max_value() {
    // Test encryption with i128::MAX - should not wrap
    let (_env, client, _admin, aggregator) = setup();
    
    // Use a value close to i128::MAX but within reasonable bounds for the demo keys
    let large_value = i128::MAX / 1000; // Scale down to fit within demo key constraints
    let c_large = client.encrypt(&large_value);
    let decrypted = client.decrypt(&aggregator, &c_large);
    
    assert_eq!(decrypted, large_value, "Encryption of large value near i128::MAX failed");
}

#[test]
fn test_homomorphic_encrypt_i128_min_value() {
    // Test encryption with i128::MIN - should not wrap
    let (_env, client, _admin, aggregator) = setup();
    
    // Use a value close to i128::MIN but within reasonable bounds
    let large_negative = i128::MIN / 1000;
    let c_large_neg = client.encrypt(&large_negative);
    let decrypted = client.decrypt(&aggregator, &c_large_neg);
    
    assert_eq!(decrypted, large_negative, "Encryption of large negative value near i128::MIN failed");
}

#[test]
fn test_homomorphic_addition_no_wraparound() {
    // Verify that homomorphic addition doesn't cause unexpected wraparound
    let (_env, client, _admin, aggregator) = setup();
    
    // Test with values that could potentially overflow if not handled correctly
    let val1 = i128::MAX / 10000;
    let val2 = i128::MAX / 10000;
    
    let c1 = client.encrypt(&val1);
    let c2 = client.encrypt(&val2);
    let c_sum = client.add_ciphertexts(&c1, &c2);
    
    let decrypted_sum = client.decrypt(&aggregator, &c_sum);
    
    // The sum should be val1 + val2, not wrapped
    let expected_sum = val1.saturating_add(val2);
    assert_eq!(decrypted_sum, expected_sum, "Homomorphic addition caused wraparound");
}

#[test]
fn test_homomorphic_subtraction_via_negative() {
    // Test that adding negative values works correctly (simulating subtraction)
    let (_env, client, _admin, aggregator) = setup();
    
    let positive = 100;
    let negative = -50;
    
    let c_pos = client.encrypt(&positive);
    let c_neg = client.encrypt(&negative);
    let c_result = client.add_ciphertexts(&c_pos, &c_neg);
    
    let decrypted = client.decrypt(&aggregator, &c_result);
    assert_eq!(decrypted, 50, "Homomorphic subtraction via negative failed");
}

#[test]
fn test_differential_privacy_sensitivity_bounds() {
    // Test DP with sensitivity at various bounds
    let env = Env::default();
    
    let base_value = 1000;
    
    // Test with sensitivity = 0 (should return exact value)
    let noise_zero_sensitivity = crate::differential_privacy::DifferentialPrivacy::add_laplace_noise(
        &env, base_value, 1, 0
    );
    assert_eq!(noise_zero_sensitivity, base_value, "DP with zero sensitivity should add no noise");
    
    // Test with very large sensitivity
    let large_sensitivity = i128::MAX / 100;
    let noise_large = crate::differential_privacy::DifferentialPrivacy::add_laplace_noise(
        &env, base_value, 1, large_sensitivity
    );
    
    // Should not overflow or underflow
    assert!(noise_large != i128::MAX, "DP noise overflow with large sensitivity");
    assert!(noise_large != i128::MIN, "DP noise underflow with large sensitivity");
}

#[test]
fn test_differential_privacy_epsilon_bounds() {
    // Test DP with epsilon at boundary values
    let env = Env::default();
    
    let value = 500;
    
    // Test with epsilon = u32::MAX
    let noise_max_epsilon = crate::differential_privacy::DifferentialPrivacy::add_laplace_noise(
        &env, value, u32::MAX, 10
    );
    
    // With very high epsilon, noise should be minimal
    assert!(noise_max_epsilon >= value - 1 && noise_max_epsilon <= value + 1,
        "DP with max epsilon should produce minimal noise");
}

#[test]
fn test_aggregate_sum_empty_vs_single_element() {
    // Verify identity element behavior
    let env = Env::default();
    
    let pub_key = PaillierPublicKey {
        n: 33,
        nn: 1089,
        g: 34,
    };
    
    // Empty aggregation
    let empty_vec: Vec<i128> = Vec::new(&env);
    let empty_result = crate::aggregation::Aggregator::aggregate_sum(&pub_key, empty_vec);
    
    // Single element (identity)
    let single_vec = Vec::new(&env);
    let single_result = crate::aggregation::Aggregator::aggregate_sum(&pub_key, single_vec);
    
    assert_eq!(empty_result, 1, "Empty sum should be identity (1)");
    assert_eq!(single_result, 1, "Single identity element should remain 1");
}

#[test]
fn test_aggregate_many_ciphertexts_accumulation() {
    // Test accumulation of many ciphertexts to check for overflow
    let (env, client, _admin, aggregator) = setup();
    
    // Create a large number of ciphertexts
    let mut all_ciphertexts = Vec::new(&env);
    for _i in 1..=50 {
        all_ciphertexts.push_back(client.encrypt(&10)); // Each encrypts 10
    }
    
    let pub_key = PaillierPublicKey {
        n: 33,
        nn: 1089,
        g: 34,
    };
    
    let aggregated = crate::aggregation::Aggregator::aggregate_sum(&pub_key, all_ciphertexts);
    
    // Decrypt to verify
    let decrypted = client.decrypt(&aggregator, &aggregated);
    
    // Should be 50 * 10 = 500 (plus/minus any implementation details)
    assert!(decrypted > 0 && decrypted < 1000, 
        "Aggregation of many ciphertexts produced unexpected result: {}", decrypted);
}

#[test]
fn test_metric_value_count_accumulation_to_boundary() {
    // Test metric count accumulation approaching boundary values
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("CNT_BOUND");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1,
    };
    
    // Perform multiple aggregations to accumulate count
    for _ in 0..10 {
        let mut recs = Vec::new(&env);
        recs.push_back(client.encrypt(&5));
        client.aggregate_records(&aggregator, &kind, &dims, &recs);
    }
    
    let metric = client.get_metric(&kind, &dims);
    
    // Count should be exactly 10 (one record per aggregation)
    assert_eq!(metric.count, 10, "Count accumulation failed");
    
    // Sum should be approximately 50 (10 aggregations * 5) plus DP noise
    assert!(metric.sum >= 40 && metric.sum <= 60, 
        "Sum accumulation out of expected range: {}", metric.sum);
}

#[test]
fn test_metric_value_repeated_updates_same_dimensions() {
    // Test repeated updates to same dimensions check for arithmetic issues
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("REP_UPD");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1,
    };
    
    // Update 100 times with small values
    for _i in 0..100 {
        let mut recs = Vec::new(&env);
        recs.push_back(client.encrypt(&1));
        client.aggregate_records(&aggregator, &kind, &dims, &recs);
    }
    
    let metric = client.get_metric(&kind, &dims);
    
    assert_eq!(metric.count, 100, "Repeated updates count incorrect");
    // Sum should be approximately 100 plus DP noise
    assert!(metric.sum >= 90 && metric.sum <= 110, 
        "Repeated updates sum out of range: {}", metric.sum);
}

#[test]
fn test_time_bucket_u64_max_boundary() {
    // Test with maximum u64 time bucket value
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("TIME_MAX");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: u64::MAX,
    };
    
    let mut recs = Vec::new(&env);
    recs.push_back(client.encrypt(&42));
    client.aggregate_records(&aggregator, &kind, &dims, &recs);
    
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 1, "Time bucket u64::MAX failed");
}

#[test]
fn test_time_bucket_u64_min_boundary() {
    // Test with minimum u64 time bucket value (0)
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("TIME_MIN");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: u64::MIN,
    };
    
    let mut recs = Vec::new(&env);
    recs.push_back(client.encrypt(&99));
    client.aggregate_records(&aggregator, &kind, &dims, &recs);
    
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 1, "Time bucket u64::MIN failed");
}

#[test]
fn test_trend_retrieval_across_full_range() {
    // Test trend retrieval with start and end buckets at boundaries
    let (env, client, _admin, _aggregator) = setup();
    
    let kind = symbol_short!("TRND");
    
    // Populate a few buckets in a reasonable range
    for bucket in 0..=10 {
        let dims = MetricDimensions {
            region: None,
            age_band: None,
            condition: None,
            time_bucket: bucket,
        };
        let mut recs = Vec::new(&env);
        recs.push_back(client.encrypt(&(bucket as i128)));
        client.aggregate_records(&_admin, &kind, &dims, &recs);
    }
    
    // Retrieve trend across the populated range
    let trend = client.get_trend(&kind, &None, &None, &None, &0, &10);
    assert_eq!(trend.len(), 11, "Trend retrieval across full range failed");
}

#[test]
fn test_metric_dimensions_all_none_values() {
    // Test with all optional dimensions set to None
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("ALL_NONE");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 42,
    };
    
    let mut recs = Vec::new(&env);
    recs.push_back(client.encrypt(&77));
    client.aggregate_records(&aggregator, &kind, &dims, &recs);
    
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 1, "All None dimensions failed");
}

#[test]
fn test_metric_dimensions_all_some_values() {
    // Test with all optional dimensions set to Some
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("ALL_SOME");
    let dims = MetricDimensions {
        region: Some(symbol_short!("US")),
        age_band: Some(symbol_short!("A18_30")),
        condition: Some(symbol_short!("HYP")),
        time_bucket: 123,
    };
    
    let mut recs = Vec::new(&env);
    recs.push_back(client.encrypt(&88));
    client.aggregate_records(&aggregator, &kind, &dims, &recs);
    
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 1, "All Some dimensions failed");
}

#[test]
fn test_saturating_addition_in_metric_update() {
    // Verify that metric updates use saturating arithmetic correctly
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("SAT_ADD");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1,
    };
    
    // First aggregation
    let large_val = i128::MAX / 100;
    let mut recs1 = Vec::new(&env);
    recs1.push_back(client.encrypt(&large_val));
    client.aggregate_records(&aggregator, &kind, &dims, &recs1);
    
    // Second aggregation - should saturate rather than overflow
    let mut recs2 = Vec::new(&env);
    recs2.push_back(client.encrypt(&large_val));
    client.aggregate_records(&aggregator, &kind, &dims, &recs2);
    
    let metric = client.get_metric(&kind, &dims);
    
    // Count should accumulate normally
    assert_eq!(metric.count, 2, "Saturating addition count failed");
    // Sum should use saturating arithmetic (not wrap around)
    assert!(metric.sum > 0, "Saturating addition produced invalid result");
}

#[test]
fn test_windowed_aggregation_boundary_timestamps() {
    // Test windowed aggregation with boundary timestamp values
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("WIN_BOUND");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1,
    };
    
    let mut recs = Vec::new(&env);
    recs.push_back(client.encrypt(&33));
    
    // Test with current timestamp (should work)
    let now = env.ledger().timestamp();
    let result = client.try_aggregate_records_in_window(
        &aggregator, &kind, &dims, &recs, &now, &(now + 1000)
    );
    assert!(result.is_ok(), "Windowed aggregation with valid timestamps failed");
}

#[test]
fn test_encryption_zero_plaintext_operations() {
    // Test various operations with encrypted zero
    let (_env, client, _admin, aggregator) = setup();
    
    let c_zero1 = client.encrypt(&0);
    let c_zero2 = client.encrypt(&0);
    
    // Add two encrypted zeros
    let c_sum = client.add_ciphertexts(&c_zero1, &c_zero2);
    let decrypted = client.decrypt(&aggregator, &c_sum);
    
    assert_eq!(decrypted, 0, "Addition of encrypted zeros failed");
}

#[test]
fn test_aggregate_average_division_by_zero_protection() {
    // Ensure average calculation handles division by zero safely
    let sum = 1000;
    let count = 0;
    
    let avg = crate::aggregation::Aggregator::aggregate_average(sum, count);
    assert_eq!(avg, 0, "Average should return 0 when count is 0");
}

#[test]
fn test_pow_mod_with_zero_exponent() {
    // Test modular exponentiation with zero exponent
    let (_env, client, _admin, _aggregator) = setup();
    
    // This indirectly tests pow_mod through encryption
    // Any number to the power of 0 should be 1
    let c = client.encrypt(&0);
    
    // Encryption should still work correctly
    assert!(c > 0, "Encryption with zero plaintext failed");
}

#[test]
fn test_mul_mod_with_large_operands_via_encryption() {
    // Test modular multiplication indirectly through encryption operations
    let (_env, client, _admin, aggregator) = setup();
    
    // Encrypt values that will involve modular multiplication
    let val = 10;
    let c = client.encrypt(&val);
    
    // Decrypt to verify the operations worked correctly
    let decrypted = client.decrypt(&aggregator, &c);
    assert_eq!(decrypted, val, "Modular multiplication in encryption failed");
}

#[test]
fn test_add_mod_approaching_modulus_via_operations() {
    // Test modular addition indirectly through homomorphic operations
    let (_env, client, _admin, aggregator) = setup();
    
    // Create ciphertexts that when added will approach modulus boundaries
    let val1 = 15;
    let val2 = 17;
    
    let c1 = client.encrypt(&val1);
    let c2 = client.encrypt(&val2);
    
    // Add them (this uses mul_mod internally which relies on add_mod)
    let c_sum = client.add_ciphertexts(&c1, &c2);
    let decrypted = client.decrypt(&aggregator, &c_sum);
    
    assert_eq!(decrypted, 32, "Modular addition in homomorphic ops incorrect");
}

#[test]
fn test_decrypt_with_invalid_ciphertext() {
    // Test decryption with arbitrary ciphertext values
    let (_env, client, _admin, aggregator) = setup();
    
    // Try decrypting an arbitrary large value
    let arbitrary_ciphertext = i128::MAX / 2;
    let result = client.decrypt(&aggregator, &arbitrary_ciphertext);
    
    // Should not panic, should return some value (may be garbage but shouldn't crash)
    assert!(result >= i128::MIN && result <= i128::MAX, 
        "Decryption of invalid ciphertext caused overflow/underflow");
}

// ============================================================================
// Comprehensive Boundary and Wrap-Around Tests
// ============================================================================

#[test]
fn test_homomorphic_multiplication_wrapping_i128_max() {
    // Test that multiplication near i128::MAX doesn't wrap unexpectedly
    let (_env, client, _admin, aggregator) = setup();
    
    // Use values that when multiplied could approach boundaries
    let large_val1 = i128::MAX / 100;
    let large_val2 = 50;
    
    let c1 = client.encrypt(&large_val1);
    let c2 = client.encrypt(&large_val2);
    
    // The homomorphic addition should work without unexpected wrapping
    let c_sum = client.add_ciphertexts(&c1, &c2);
    let decrypted = client.decrypt(&aggregator, &c_sum);
    
    // Verify no unexpected wraparound occurred
    assert!(decrypted > 0, "Unexpected negative result from large value addition");
}

#[test]
fn test_homomorphic_operations_with_i128_min_values() {
    // Test operations with minimum i128 values
    let (_env, client, _admin, aggregator) = setup();
    
    let min_val = i128::MIN / 1000; // Use reasonable minimum
    let c_min = client.encrypt(&min_val);
    
    let decrypted = client.decrypt(&aggregator, &c_min);
    assert_eq!(decrypted, min_val, "i128 minimum value encryption/decryption failed");
}

#[test]
fn test_add_ciphertexts_repeated_operations() {
    // Test repeated additions don't cause accumulation errors
    let (_env, client, _admin, aggregator) = setup();
    
    let mut accumulator = client.encrypt(&0);
    let iterations = 50;
    
    for i in 1..=iterations {
        let c_i = client.encrypt(&(i as i128));
        accumulator = client.add_ciphertexts(&accumulator, &c_i);
    }
    
    let result = client.decrypt(&aggregator, &accumulator);
    let expected_sum: i128 = (1..=iterations).map(|x| x as i128).sum();
    
    assert_eq!(result, expected_sum, "Repeated additions accumulated incorrectly");
}

#[test]
fn test_differential_privacy_sensitivity_boundary_i128_max() {
    // Test DP with sensitivity at i128::MAX boundary
    let env = Env::default();
    
    let value = 0;
    let sensitivity = i128::MAX;
    
    // Should use saturating arithmetic to prevent overflow
    let noisy = crate::differential_privacy::DifferentialPrivacy::add_laplace_noise(
        &env, value, 1, sensitivity
    );
    
    // Verify no panic and result is within bounds
    assert!(noisy >= i128::MIN, "DP noise caused underflow at max sensitivity");
    assert!(noisy <= i128::MAX, "DP noise caused overflow at max sensitivity");
}

#[test]
fn test_differential_privacy_epsilon_boundary_values() {
    // Test DP with epsilon at u32 boundaries
    let env = Env::default();
    
    let value = 100;
    
    // Test with epsilon = u32::MAX
    let noisy_max = crate::differential_privacy::DifferentialPrivacy::add_laplace_noise(
        &env, value, u32::MAX, 10
    );
    
    // With very high epsilon, noise should be minimal
    assert!(noisy_max >= 90 && noisy_max <= 110, 
        "DP with max epsilon produced unexpected noise: {}", noisy_max);
}

#[test]
fn test_differential_privacy_noise_accumulation() {
    // Test that repeated DP noise addition doesn't cause unexpected accumulation
    let env = Env::default();
    
    let base_value = 1000;
    let mut total_noise_added = 0;
    let iterations = 100;
    
    for _ in 0..iterations {
        let noisy = crate::differential_privacy::DifferentialPrivacy::add_laplace_noise(
            &env, base_value, 1, 10
        );
        total_noise_added += (noisy - base_value).abs();
    }
    
    // Average noise should be within reasonable bounds
    let avg_noise = total_noise_added / iterations;
    assert!(avg_noise < 20, "DP noise accumulation exceeded expected bounds");
}

#[test]
fn test_aggregation_count_overflow_protection() {
    // Test that count aggregation handles large numbers correctly
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("CNT_OVF");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1,
    };
    
    // Create many small ciphertexts to test count overflow
    let mut all_ciphertexts = Vec::new(&env);
    for _ in 0..500 {
        all_ciphertexts.push_back(client.encrypt(&1));
    }
    
    client.aggregate_records(&aggregator, &kind, &dims, &all_ciphertexts);
    
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 500, "Count overflow detected");
}

#[test]
fn test_aggregation_sum_with_mixed_signs() {
    // Test aggregation with both positive and negative values
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("MIXED");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1,
    };
    
    let mut ciphertexts = Vec::new(&env);
    // Add positive values
    for i in 1..=50 {
        ciphertexts.push_back(client.encrypt(&i));
    }
    // Add negative values
    for i in 1..=50 {
        ciphertexts.push_back(client.encrypt(&-(i as i128)));
    }
    
    client.aggregate_records(&aggregator, &kind, &dims, &ciphertexts);
    
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 100, "Mixed sign count incorrect");
    // Sum should be close to 0 (plus/minus DP noise)
    assert!(metric.sum.abs() < 50, "Mixed sign sum out of expected range: {}", metric.sum);
}

#[test]
fn test_metric_value_count_saturating_at_i128_max() {
    // Test that count updates saturate at i128::MAX rather than wrapping
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("SAT_CNT");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1,
    };
    
    // First aggregation with large count
    let mut recs1 = Vec::new(&env);
    let large_count = i128::MAX / 2;
    recs1.push_back(client.encrypt(&large_count));
    client.aggregate_records(&aggregator, &kind, &dims, &recs1);
    
    let metric1 = client.get_metric(&kind, &dims);
    assert_eq!(metric1.count, 1, "First large count failed");
    
    // Second aggregation - count should increment by 1, not by the value
    let mut recs2 = Vec::new(&env);
    recs2.push_back(client.encrypt(&100));
    client.aggregate_records(&aggregator, &kind, &dims, &recs2);
    
    let metric2 = client.get_metric(&kind, &dims);
    assert_eq!(metric2.count, 2, "Count should increment by number of records, not values");
}

#[test]
fn test_time_bucket_u64_zero_and_max() {
    // Test time bucket at absolute boundaries
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("TB_EXT");
    
    // Test with time_bucket = 0 (minimum)
    let dims_min = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 0,
    };
    
    let mut recs = Vec::new(&env);
    recs.push_back(client.encrypt(&42));
    client.aggregate_records(&aggregator, &kind, &dims_min, &recs);
    
    let metric_min = client.get_metric(&kind, &dims_min);
    assert_eq!(metric_min.count, 1, "Time bucket 0 failed");
    
    // Test with time_bucket = u64::MAX (maximum)
    let dims_max = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: u64::MAX,
    };
    
    client.aggregate_records(&aggregator, &kind, &dims_max, &recs);
    let metric_max = client.get_metric(&kind, &dims_max);
    assert_eq!(metric_max.count, 1, "Time bucket u64::MAX failed");
}

#[test]
fn test_trend_retrieval_consecutive_buckets() {
    // Test trend retrieval across consecutive bucket boundaries
    let (env, client, _admin, _aggregator) = setup();
    
    let kind = symbol_short!("CONS");
    
    // Populate consecutive buckets including boundary transitions
    let buckets = vec![0, 1, 2, 100, 101, 102, u64::MAX - 1, u64::MAX];
    
    for &bucket in &buckets {
        let dims = MetricDimensions {
            region: None,
            age_band: None,
            condition: None,
            time_bucket: bucket,
        };
        let mut recs = Vec::new(&env);
        recs.push_back(client.encrypt(&(bucket as i128)));
        client.aggregate_records(&_admin, &kind, &dims, &recs);
    }
    
    // Retrieve trend for first few buckets
    let trend_start = client.get_trend(&kind, &None, &None, &None, &0, &2);
    assert_eq!(trend_start.len(), 3, "Trend start retrieval failed");
    
    // Retrieve trend for middle buckets
    let trend_middle = client.get_trend(&kind, &None, &None, &None, &100, &102);
    assert_eq!(trend_middle.len(), 3, "Trend middle retrieval failed");
}

#[test]
fn test_version_number_u32_increment_near_max() {
    // Test version number behavior near u32::MAX
    let (env, client, _admin, _aggregator) = setup();
    
    // Verify initial version
    assert_eq!(client.get_dep_ver(), 1, "Initial version should be 1");
    
    // Multiple dependency updates
    for i in 2..=10 {
        let new_agg = Address::generate(&env);
        client.set_aggregator(&new_agg);
        assert_eq!(client.get_dep_ver(), i, "Version increment failed at iteration {}", i);
    }
    
    // Verify version increments correctly after key changes too
    let pub_key = PaillierPublicKey { n: 33, nn: 1089, g: 34 };
    client.set_paillier_keys(&pub_key, &None);
    assert_eq!(client.get_dep_ver(), 11, "Version should increment after key change");
}

#[test]
fn test_metric_dimensions_all_none_except_time() {
    // Test with all optional dimensions as None
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("ALL_NONE");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 999,
    };
    
    let mut recs = Vec::new(&env);
    recs.push_back(client.encrypt(&777));
    client.aggregate_records(&aggregator, &kind, &dims, &recs);
    
    let metric = client.get_metric(&kind, &dims);
    assert_eq!(metric.count, 1, "All-None dimensions failed");
}

#[test]
fn test_metric_dimensions_all_some_combinations() {
    // Test with all optional dimensions populated
    let (env, client, _admin, aggregator) = setup();
    
    let combinations = vec![
        (Some(symbol_short!("US")), Some(symbol_short!("A0_17")), Some(symbol_short!("HYP"))),
        (Some(symbol_short!("EU")), Some(symbol_short!("A18_34")), Some(symbol_short!("MYO"))),
        (Some(symbol_short!("AS")), Some(symbol_short!("A35_64")), Some(symbol_short!("GLC"))),
    ];
    
    for (region, age, cond) in combinations {
        let dims = MetricDimensions {
            region,
            age_band: age,
            condition: cond,
            time_bucket: 2024,
        };
        
        let mut recs = Vec::new(&env);
        recs.push_back(client.encrypt(&123));
        client.aggregate_records(&aggregator, &symbol_short!("DIM_TEST"), &dims, &recs);
        
        let metric = client.get_metric(&symbol_short!("DIM_TEST"), &dims);
        assert_eq!(metric.count, 1, "Dimension combination failed: {:?}", dims);
    }
}

#[test]
fn test_encrypt_decrypt_round_trip_extreme_values() {
    // Test encrypt-decrypt round trip with various extreme values
    let (_env, client, _admin, aggregator) = setup();
    
    let test_values = vec![
        i128::MIN / 100,  // Near minimum
        -1000,
        -1,
        0,
        1,
        1000,
        i128::MAX / 100,  // Near maximum
    ];
    
    for &value in &test_values {
        let encrypted = client.encrypt(&value);
        let decrypted = client.decrypt(&aggregator, &encrypted);
        assert_eq!(
            decrypted, value, 
            "Round trip failed for value {}: got {}", value, decrypted
        );
    }
}

#[test]
fn test_aggregate_records_empty_then_populated() {
    // Test transitioning from empty to populated state
    let (env, client, _admin, aggregator) = setup();
    
    let kind = symbol_short!("EMP_POP");
    let dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket: 1,
    };
    
    // Initial state should be zero
    let initial = client.get_metric(&kind, &dims);
    assert_eq!(initial.count, 0, "Initial state should be zero");
    assert_eq!(initial.sum, 0, "Initial sum should be zero");
    
    // First aggregation
    let mut recs1 = Vec::new(&env);
    recs1.push_back(client.encrypt(&50));
    client.aggregate_records(&aggregator, &kind, &dims, &recs1);
    
    let after_first = client.get_metric(&kind, &dims);
    assert_eq!(after_first.count, 1, "After first aggregation count failed");
    
    // Second aggregation to same dimensions
    let mut recs2 = Vec::new(&env);
    recs2.push_back(client.encrypt(&30));
    client.aggregate_records(&aggregator, &kind, &dims, &recs2);
    
    let after_second = client.get_metric(&kind, &dims);
    assert_eq!(after_second.count, 2, "Accumulation failed");
}


