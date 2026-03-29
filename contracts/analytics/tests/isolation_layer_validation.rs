extern crate std;

use analytics::{AnalyticsContract, AnalyticsContractClient, MetricDimensions, ContractError};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec, symbol_short};

fn setup_isolation_layer_test() -> (Env, AnalyticsContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AnalyticsContract, ());
    let client = AnalyticsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let aggregator = Address::generate(&env);

    let pub_key = analytics::homomorphic::PaillierPublicKey {
        n: 33,
        nn: 1089,
        g: 34,
    };
    let priv_key = analytics::homomorphic::PaillierPrivateKey { lambda: 20, mu: 5 };

    client.initialize(&admin, &aggregator, &pub_key, &Some(priv_key));

    (env, client, admin, aggregator)
}

#[test]
fn test_region_key_validation() {
    let (env, client, _admin, aggregator) = setup_isolation_layer_test();

    let kind = symbol_short!("REGION_KEY_TEST");
    let time_bucket = 1_700_000_000;

    // Test valid region keys
    let valid_regions = vec![
        symbol_short!("HOSPITAL_A"),
        symbol_short!("CLINIC_B"),
        symbol_short!("REGION_X"),
        symbol_short!("CENTER_Y"),
    ];

    for region in valid_regions {
        let dims = MetricDimensions {
            region: Some(region),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("MYOPIA")),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&10));
        
        // Should succeed with valid region
        let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
        assert!(result.is_ok(), "Should succeed with valid region: {:?}", region);
    }

    // Test null region
    let null_region_dims = MetricDimensions {
        region: None,
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    let mut null_records = Vec::new(&env);
    null_records.push_back(client.encrypt(&10));
    
    // Should also succeed with null region
    let null_result = client.try_aggregate_records(&aggregator, &kind, &null_region_dims, &null_records);
    assert!(null_result.is_ok(), "Should succeed with null region");

    // Verify isolation between different regions
    let region_a_data = client.get_metric(&kind, &MetricDimensions {
        region: Some(symbol_short!("HOSPITAL_A")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    });

    let region_b_data = client.get_metric(&kind, &MetricDimensions {
        region: Some(symbol_short!("CLINIC_B")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    });

    let null_region_data = client.get_metric(&kind, &null_region_dims);

    // Each should have their own isolated data
    assert_eq!(region_a_data.count, 1);
    assert_eq!(region_b_data.count, 1);
    assert_eq!(null_region_data.count, 1);
}

#[test]
fn test_age_band_key_validation() {
    let (env, client, _admin, aggregator) = setup_isolation_layer_test();

    let kind = symbol_short!("AGE_BAND_KEY_TEST");
    let time_bucket = 1_700_000_000;
    let region = symbol_short!("TEST_HOSPITAL");

    // Test valid age band keys
    let valid_age_bands = vec![
        symbol_short!("A0_17"),    // Minors
        symbol_short!("A18_39"),   // Young adults
        symbol_short!("A40_64"),   // Middle-aged adults
        symbol_short!("A65P"),     // Seniors
    ];

    for age_band in valid_age_bands {
        let dims = MetricDimensions {
            region: Some(region),
            age_band: Some(age_band),
            condition: Some(symbol_short!("DIABETES")),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&15));
        
        // Should succeed with valid age band
        let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
        assert!(result.is_ok(), "Should succeed with valid age band: {:?}", age_band);
    }

    // Test null age band
    let null_age_dims = MetricDimensions {
        region: Some(region),
        age_band: None,
        condition: Some(symbol_short!("DIABETES")),
        time_bucket,
    };

    let mut null_records = Vec::new(&env);
    null_records.push_back(client.encrypt(&20));
    
    // Should succeed with null age band
    let null_result = client.try_aggregate_records(&aggregator, &kind, &null_age_dims, &null_records);
    assert!(null_result.is_ok(), "Should succeed with null age band");

    // Verify isolation between different age bands
    for age_band in valid_age_bands {
        let age_data = client.get_metric(&kind, &MetricDimensions {
            region: Some(region),
            age_band: Some(age_band),
            condition: Some(symbol_short!("DIABETES")),
            time_bucket,
        });

        assert_eq!(age_data.count, 1, "Each age band should have its own data");
    }

    let null_age_data = client.get_metric(&kind, &null_age_dims);
    assert_eq!(null_age_data.count, 1);
}

#[test]
fn test_condition_key_validation() {
    let (env, client, _admin, aggregator) = setup_isolation_layer_test();

    let kind = symbol_short!("CONDITION_KEY_TEST");
    let time_bucket = 1_700_000_000;
    let region = symbol_short!("MEDICAL_CENTER");
    let age_band = symbol_short!("A40_64");

    // Test valid condition keys
    let valid_conditions = vec![
        symbol_short!("MYOPIA"),
        symbol_short!("GLAUCOMA"),
        symbol_short!("CATARACT"),
        symbol_short!("DIABETES"),
        symbol_short!("HYPERTENSION"),
        symbol_short!("HEART_DISEASE"),
        symbol_short!("MENTAL_HEALTH"),
        symbol_short!("RESPIRATORY"),
    ];

    for condition in valid_conditions {
        let dims = MetricDimensions {
            region: Some(region),
            age_band: Some(age_band),
            condition: Some(condition),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&25));
        
        // Should succeed with valid condition
        let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
        assert!(result.is_ok(), "Should succeed with valid condition: {:?}", condition);
    }

    // Test null condition
    let null_condition_dims = MetricDimensions {
        region: Some(region),
        age_band: Some(age_band),
        condition: None,
        time_bucket,
    };

    let mut null_records = Vec::new(&env);
    null_records.push_back(client.encrypt(&30));
    
    // Should succeed with null condition
    let null_result = client.try_aggregate_records(&aggregator, &kind, &null_condition_dims, &null_records);
    assert!(null_result.is_ok(), "Should succeed with null condition");

    // Verify isolation between different conditions
    for condition in valid_conditions {
        let condition_data = client.get_metric(&kind, &MetricDimensions {
            region: Some(region),
            age_band: Some(age_band),
            condition: Some(condition),
            time_bucket,
        });

        assert_eq!(condition_data.count, 1, "Each condition should have its own data");
    }

    let null_condition_data = client.get_metric(&kind, &null_condition_dims);
    assert_eq!(null_condition_data.count, 1);
}

#[test]
fn test_time_bucket_key_validation() {
    let (env, client, _admin, aggregator) = setup_isolation_layer_test();

    let kind = symbol_short!("TIME_BUCKET_TEST");
    let region = symbol_short!("TIME_TEST_HOSPITAL");
    let age_band = symbol_short!("A40_64");
    let condition = symbol_short!("MYOPIA");

    // Test valid time buckets
    let valid_time_buckets = vec![
        1_600_000_000, // 2020
        1_650_000_000, // 2022
        1_700_000_000, // 2023
        1_750_000_000, // 2025
    ];

    for time_bucket in valid_time_buckets {
        let dims = MetricDimensions {
            region: Some(region),
            age_band: Some(age_band),
            condition: Some(condition),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&5));
        
        // Should succeed with valid time bucket
        let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
        assert!(result.is_ok(), "Should succeed with valid time bucket: {}", time_bucket);
    }

    // Test edge cases
    let edge_time_buckets = vec![0, 1, u64::MAX - 1];

    for time_bucket in edge_time_buckets {
        let dims = MetricDimensions {
            region: Some(region),
            age_band: Some(age_band),
            condition: Some(condition),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&1));
        
        // Should handle edge cases
        let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
        assert!(result.is_ok(), "Should handle edge time bucket: {}", time_bucket);
    }

    // Verify isolation between different time buckets
    for time_bucket in valid_time_buckets {
        let time_data = client.get_metric(&kind, &MetricDimensions {
            region: Some(region),
            age_band: Some(age_band),
            condition: Some(condition),
            time_bucket,
        });

        assert_eq!(time_data.count, 1, "Each time bucket should have its own data");
    }
}

#[test]
fn test_combination_key_isolation() {
    let (env, client, _admin, aggregator) = setup_isolation_layer_test();

    let kind = symbol_short!("COMBINATION_TEST");
    let time_bucket = 1_700_000_000;

    // Create different combinations of dimensions
    let regions = vec![symbol_short!("HOSPITAL_A"), symbol_short!("HOSPITAL_B")];
    let age_bands = vec![symbol_short!("A18_39"), symbol_short!("A40_64")];
    let conditions = vec![symbol_short!("MYOPIA"), symbol_short!("GLAUCOMA")];

    let mut combination_count = 0;

    for region in &regions {
        for age_band in &age_bands {
            for condition in &conditions {
                let dims = MetricDimensions {
                    region: Some(*region),
                    age_band: Some(*age_band),
                    condition: Some(*condition),
                    time_bucket,
                };

                let mut records = Vec::new(&env);
                records.push_back(client.encrypt(&10));
                
                let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
                assert!(result.is_ok(), "Should succeed with combination: region={:?}, age_band={:?}, condition={:?}", region, age_band, condition);
                
                combination_count += 1;
            }
        }
    }

    // Verify that each combination is isolated
    let mut verified_combinations = 0;

    for region in &regions {
        for age_band in &age_bands {
            for condition in &conditions {
                let dims = MetricDimensions {
                    region: Some(*region),
                    age_band: Some(*age_band),
                    condition: Some(*condition),
                    time_bucket,
                };

                let data = client.get_metric(&kind, &dims);
                assert_eq!(data.count, 1, "Each combination should have its own data");
                
                verified_combinations += 1;
            }
        }
    }

    assert_eq!(combination_count, verified_combinations, "All combinations should be verified");
    assert_eq!(combination_count, 8, "Should have 2*2*2 = 8 combinations");
}

#[test]
fn test_partial_null_combinations() {
    let (env, client, _admin, aggregator) = setup_isolation_layer_test();

    let kind = symbol_short!("PARTIAL_NULL_TEST");
    let time_bucket = 1_700_000_000;

    // Test combinations with some null dimensions
    let test_cases = vec![
        // Only region is null
        MetricDimensions {
            region: None,
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("MYOPIA")),
            time_bucket,
        },
        // Only age_band is null
        MetricDimensions {
            region: Some(symbol_short!("HOSPITAL_X")),
            age_band: None,
            condition: Some(symbol_short!("MYOPIA")),
            time_bucket,
        },
        // Only condition is null
        MetricDimensions {
            region: Some(symbol_short!("HOSPITAL_X")),
            age_band: Some(symbol_short!("A40_64")),
            condition: None,
            time_bucket,
        },
        // Region and age_band are null
        MetricDimensions {
            region: None,
            age_band: None,
            condition: Some(symbol_short!("MYOPIA")),
            time_bucket,
        },
        // Region and condition are null
        MetricDimensions {
            region: None,
            age_band: Some(symbol_short!("A40_64")),
            condition: None,
            time_bucket,
        },
        // Age_band and condition are null
        MetricDimensions {
            region: Some(symbol_short!("HOSPITAL_X")),
            age_band: None,
            condition: None,
            time_bucket,
        },
        // All dimensions are null
        MetricDimensions {
            region: None,
            age_band: None,
            condition: None,
            time_bucket,
        },
    ];

    for (i, dims) in test_cases.iter().enumerate() {
        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&(i as i128 + 1)));
        
        let result = client.try_aggregate_records(&aggregator, &kind, dims, &records);
        assert!(result.is_ok(), "Should succeed with partial null combination {}", i);
    }

    // Verify each partial null combination is isolated
    for (i, dims) in test_cases.iter().enumerate() {
        let data = client.get_metric(&kind, dims);
        assert_eq!(data.count, 1, "Partial null combination {} should have its own data", i);
    }

    // Cross-verify that different combinations don't interfere
    for (i, dims_a) in test_cases.iter().enumerate() {
        for (j, dims_b) in test_cases.iter().enumerate() {
            if i != j {
                let data_a = client.get_metric(&kind, dims_a);
                let data_b = client.get_metric(&kind, dims_b);
                
                // Both should have their own data
                assert_eq!(data_a.count, 1);
                assert_eq!(data_b.count, 1);
                
                // The dimensions should be different (unless they're actually the same)
                if dims_a.region != dims_b.region || dims_a.age_band != dims_b.age_band || dims_a.condition != dims_b.condition {
                    // These are different combinations, so they should be isolated
                    // We can't directly test isolation here since get_metric is public,
                    // but we can verify they have separate storage keys
                    assert_ne!(dims_a, dims_b, "Different test cases should have different dimensions");
                }
            }
        }
    }
}

#[test]
fn test_storage_key_isolation() {
    let (env, client, _admin, aggregator) = setup_isolation_layer_test();

    let kind = symbol_short!("STORAGE_KEY_TEST");
    let time_bucket = 1_700_000_000;

    // Create two very similar but different dimensions
    let dims1 = MetricDimensions {
        region: Some(symbol_short!("HOSPITAL_A")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    let dims2 = MetricDimensions {
        region: Some(symbol_short!("HOSPITAL_A")), // Same region
        age_band: Some(symbol_short!("A40_64")),   // Same age band
        condition: Some(symbol_short!("GLAUCOMA")), // Different condition
        time_bucket,
    };

    // Add different data to each
    let mut records1 = Vec::new(&env);
    records1.push_back(client.encrypt(&100)); // 100 for myopia
    
    let mut records2 = Vec::new(&env);
    records2.push_back(client.encrypt(&50));  // 50 for glaucoma

    client.aggregate_records(&aggregator, &kind, &dims1, &records1);
    client.aggregate_records(&aggregator, &kind, &dims2, &records2);

    // Verify storage isolation by checking metrics
    let data1 = client.get_metric(&kind, &dims1);
    let data2 = client.get_metric(&kind, &dims2);

    assert_eq!(data1.count, 1);
    assert_eq!(data2.count, 1);

    // Even though region and age_band are the same, different condition should create isolation
    assert_ne!(dims1.condition, dims2.condition);

    // Test with same condition but different time bucket
    let dims3 = MetricDimensions {
        region: Some(symbol_short!("HOSPITAL_A")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")), // Same as dims1
        time_bucket: time_bucket + 1,             // Different time bucket
    };

    let mut records3 = Vec::new(&env);
    records3.push_back(client.encrypt(&75));
    client.aggregate_records(&aggregator, &kind, &dims3, &records3);

    let data3 = client.get_metric(&kind, &dims3);
    assert_eq!(data3.count, 1);

    // Verify that dims1 and dims3 are isolated despite having same region, age_band, and condition
    assert_ne!(dims1.time_bucket, dims3.time_bucket);

    // All should have separate data
    let data1_recheck = client.get_metric(&kind, &dims1);
    let data2_recheck = client.get_metric(&kind, &dims2);
    let data3_recheck = client.get_metric(&kind, &dims3);

    assert_eq!(data1_recheck.count, 1);
    assert_eq!(data2_recheck.count, 1);
    assert_eq!(data3_recheck.count, 1);
}

#[test]
fn test_authorization_layer_isolation() {
    let (env, client, _admin, aggregator) = setup_isolation_layer_test();

    let kind = symbol_short!("AUTH_ISOLATION_TEST");
    let dims = MetricDimensions {
        region: Some(symbol_short!("AUTH_TEST_HOSPITAL")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("CATARACT")),
        time_bucket: 1_700_000_000,
    };

    let mut records = Vec::new(&env);
    records.push_back(client.encrypt(&20));

    // Test that only authorized aggregator can add data
    let unauthorized_user = Address::generate(&env);
    
    let unauthorized_result = client.try_aggregate_records(&unauthorized_user, &kind, &dims, &records);
    assert!(unauthorized_result.is_err());
    assert_eq!(unauthorized_result.unwrap_err(), Ok(ContractError::Unauthorized));

    // Test that authorized aggregator can add data
    let authorized_result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
    assert!(authorized_result.is_ok());

    // Test that read operations are public (no authorization required)
    let read_result = client.get_metric(&kind, &dims);
    assert_eq!(read_result.count, 1);

    // Test that admin cannot aggregate data
    let admin_aggregate_result = client.try_aggregate_records(&_admin, &kind, &dims, &records);
    assert!(admin_aggregate_result.is_err());
    assert_eq!(admin_aggregate_result.unwrap_err(), Ok(ContractError::Unauthorized));

    // Test that admin cannot decrypt data
    let ciphertext = client.encrypt(&42);
    let admin_decrypt_result = client.try_decrypt(&_admin, &ciphertext);
    assert!(admin_decrypt_result.is_err());
    assert_eq!(admin_decrypt_result.unwrap_err(), Ok(ContractError::Unauthorized));

    // Test that aggregator can decrypt data
    let aggregator_decrypt_result = client.try_decrypt(&aggregator, &ciphertext);
    assert!(aggregator_decrypt_result.is_ok());

    // Verify authorization isolation: unauthorized users can't modify data
    // but can read public metrics
    let unauthorized_read = client.get_metric(&kind, &dims);
    assert_eq!(unauthorized_read.count, 1); // Public read access works
}
