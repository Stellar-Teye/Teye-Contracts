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

    let kind = symbol_short!("REG_K_TST");
    let time_bucket = 1_700_000_000;

    // Test valid region keys
    let valid_regions = vec![
        symbol_short!("HOSP_A"),
        symbol_short!("CLIN_B"),
        symbol_short!("REG_X"),
        symbol_short!("CENT_Y"),
    ];

    for region in &valid_regions {
        let dims = MetricDimensions {
            region: Some(region.clone()),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("MYOPIA")),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&10i128));
        
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
    null_records.push_back(client.encrypt(&10i128));
    
    // Should also succeed with null region
    let null_result = client.try_aggregate_records(&aggregator, &kind, &null_region_dims, &null_records);
    assert!(null_result.is_ok(), "Should succeed with null region");

    // Verify isolation between different regions
    let region_a_data = client.get_metric(&kind, &MetricDimensions {
        region: Some(symbol_short!("HOSP_A")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    });

    let region_b_data = client.get_metric(&kind, &MetricDimensions {
        region: Some(symbol_short!("CLIN_B")),
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

    let kind = symbol_short!("AGE_K_TST");
    let time_bucket = 1_700_000_000;
    let region = symbol_short!("TEST_HOSP");

    // Test valid age band keys
    let valid_age_bands = vec![
        symbol_short!("A0_17"),    // Minors
        symbol_short!("A18_39"),   // Young adults
        symbol_short!("A40_64"),   // Middle-aged adults
        symbol_short!("A65P"),     // Seniors
    ];

    for age_band in &valid_age_bands {
        let dims = MetricDimensions {
            region: Some(region.clone()),
            age_band: Some(age_band.clone()),
            condition: Some(symbol_short!("DIABETES")),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&15i128));
        
        // Should succeed with valid age band
        let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
        assert!(result.is_ok(), "Should succeed with valid age band: {:?}", age_band);
    }

    // Test null age band
    let null_age_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: None,
        condition: Some(symbol_short!("DIABETES")),
        time_bucket,
    };

    let mut null_records = Vec::new(&env);
    null_records.push_back(client.encrypt(&20i128));
    
    // Should succeed with null age band
    let null_result = client.try_aggregate_records(&aggregator, &kind, &null_age_dims, &null_records);
    assert!(null_result.is_ok(), "Should succeed with null age band");

    // Verify isolation between different age bands
    for age_band in &valid_age_bands {
        let age_data = client.get_metric(&kind, &MetricDimensions {
            region: Some(region.clone()),
            age_band: Some(age_band.clone()),
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

    let kind = symbol_short!("CON_K_TST");
    let time_bucket = 1_700_000_000;
    let region = symbol_short!("MED_CENT");
    let age_band = symbol_short!("A40_64");

    // Test valid condition keys
    let valid_conditions = vec![
        symbol_short!("MYOPIA"),
        symbol_short!("GLAUCOMA"),
        symbol_short!("CATARACT"),
        symbol_short!("DIABETES"),
        symbol_short!("HYPER_T"),
        symbol_short!("HEART_DIS"),
        symbol_short!("MENT_HLTH"),
        symbol_short!("RESP_DIS"),
    ];

    for condition in &valid_conditions {
        let dims = MetricDimensions {
            region: Some(region.clone()),
            age_band: Some(age_band.clone()),
            condition: Some(condition.clone()),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&25i128));
        
        // Should succeed with valid condition
        let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
        assert!(result.is_ok(), "Should succeed with valid condition: {:?}", condition);
    }

    // Test null condition
    let null_condition_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(age_band.clone()),
        condition: None,
        time_bucket,
    };

    let mut null_records = Vec::new(&env);
    null_records.push_back(client.encrypt(&30i128));
    
    // Should succeed with null condition
    let null_result = client.try_aggregate_records(&aggregator, &kind, &null_condition_dims, &null_records);
    assert!(null_result.is_ok(), "Should succeed with null condition");

    // Verify isolation between different conditions
    for condition in &valid_conditions {
        let condition_data = client.get_metric(&kind, &MetricDimensions {
            region: Some(region.clone()),
            age_band: Some(age_band.clone()),
            condition: Some(condition.clone()),
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

    let kind = symbol_short!("TIME_B_T");
    let region = symbol_short!("T_TST_HSP");
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
            region: Some(region.clone()),
            age_band: Some(age_band.clone()),
            condition: Some(condition.clone()),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&5i128));
        
        // Should succeed with valid time bucket
        let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
        assert!(result.is_ok(), "Should succeed with valid time bucket: {}", time_bucket);
    }

    // Test edge cases
    let edge_time_buckets = vec![0, 1, u64::MAX - 1];

    for time_bucket in edge_time_buckets {
        let dims = MetricDimensions {
            region: Some(region.clone()),
            age_band: Some(age_band.clone()),
            condition: Some(condition.clone()),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&1i128));
        
        // Should handle edge cases
        let result = client.try_aggregate_records(&aggregator, &kind, &dims, &records);
        assert!(result.is_ok(), "Should handle edge time bucket: {}", time_bucket);
    }
}

#[test]
fn test_combination_key_isolation() {
    let (env, client, _admin, aggregator) = setup_isolation_layer_test();

    let kind = symbol_short!("COMB_TEST");
    let time_bucket = 1_700_000_000;

    // Create different combinations of dimensions
    let regions = vec![symbol_short!("HOSP_A"), symbol_short!("HOSP_B")];
    let age_bands = vec![symbol_short!("A18_39"), symbol_short!("A40_64")];
    let conditions = vec![symbol_short!("MYOPIA"), symbol_short!("GLAUCOMA")];

    let mut combination_count = 0;

    for region in &regions {
        for age_band in &age_bands {
            for condition in &conditions {
                let dims = MetricDimensions {
                    region: Some(region.clone()),
                    age_band: Some(age_band.clone()),
                    condition: Some(condition.clone()),
                    time_bucket,
                };

                let mut records = Vec::new(&env);
                records.push_back(client.encrypt(&10i128));
                
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
                    region: Some(region.clone()),
                    age_band: Some(age_band.clone()),
                    condition: Some(condition.clone()),
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

    let kind = symbol_short!("PRT_NUL_T");
    let time_bucket = 1_700_000_000;

    // Test combinations with some null dimensions
    let region_x = symbol_short!("HOSP_X");
    let age_a40 = symbol_short!("A40_64");
    let cond_myo = symbol_short!("MYOPIA");

    let test_cases = vec![
        // Only region is null
        MetricDimensions {
            region: None,
            age_band: Some(age_a40.clone()),
            condition: Some(cond_myo.clone()),
            time_bucket,
        },
        // Only age_band is null
        MetricDimensions {
            region: Some(region_x.clone()),
            age_band: None,
            condition: Some(cond_myo.clone()),
            time_bucket,
        },
        // Only condition is null
        MetricDimensions {
            region: Some(region_x.clone()),
            age_band: Some(age_a40.clone()),
            condition: None,
            time_bucket,
        },
        // Region and age_band are null
        MetricDimensions {
            region: None,
            age_band: None,
            condition: Some(cond_myo.clone()),
            time_bucket,
        },
        // Region and condition are null
        MetricDimensions {
            region: None,
            age_band: Some(age_a40.clone()),
            condition: None,
            time_bucket,
        },
        // Age_band and condition are null
        MetricDimensions {
            region: Some(region_x.clone()),
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
}

#[test]
fn test_storage_key_isolation() {
    let (env, client, _admin, aggregator) = setup_isolation_layer_test();

    let kind = symbol_short!("STOR_K_T");
    let time_bucket = 1_700_000_000;

    // Create two very similar but different dimensions
    let dims1 = MetricDimensions {
        region: Some(symbol_short!("HOSP_A")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    let dims2 = MetricDimensions {
        region: Some(symbol_short!("HOSP_A")), // Same region
        age_band: Some(symbol_short!("A40_64")),   // Same age band
        condition: Some(symbol_short!("GLAUCOMA")), // Different condition
        time_bucket,
    };

    // Add different data to each
    let mut records1 = Vec::new(&env);
    records1.push_back(client.encrypt(&100i128)); // 100 for myopia
    
    let mut records2 = Vec::new(&env);
    records2.push_back(client.encrypt(&50i128));  // 50 for glaucoma

    client.aggregate_records(&aggregator, &kind, &dims1, &records1);
    client.aggregate_records(&aggregator, &kind, &dims2, &records2);

    // Verify storage isolation by checking metrics
    let data1 = client.get_metric(&kind, &dims1);
    let data2 = client.get_metric(&kind, &dims2);

    assert_eq!(data1.count, 1);
    assert_eq!(data2.count, 1);

    // Test with same condition but different time bucket
    let dims3 = MetricDimensions {
        region: Some(symbol_short!("HOSP_A")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")), // Same as dims1
        time_bucket: time_bucket + 1,             // Different time bucket
    };

    let mut records3 = Vec::new(&env);
    records3.push_back(client.encrypt(&75i128));
    client.aggregate_records(&aggregator, &kind, &dims3, &records3);

    let data3 = client.get_metric(&kind, &dims3);
    assert_eq!(data3.count, 1);
}

#[test]
fn test_authorization_layer_isolation() {
    let (env, client, _admin, aggregator) = setup_isolation_layer_test();

    let kind = symbol_short!("AUT_ISO_T");
    let dims = MetricDimensions {
        region: Some(symbol_short!("AUTH_HOSP")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("CATARACT")),
        time_bucket: 1_700_000_000,
    };

    let mut records = Vec::new(&env);
    records.push_back(client.encrypt(&20i128));

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
    let ciphertext = client.encrypt(&42i128);
    let admin_decrypt_result = client.try_decrypt(&_admin, &ciphertext);
    assert!(admin_decrypt_result.is_err());
    assert_eq!(admin_decrypt_result.unwrap_err(), Ok(ContractError::Unauthorized));

    // Test that aggregator can decrypt data
    let aggregator_decrypt_result = client.try_decrypt(&aggregator, &ciphertext);
    assert!(aggregator_decrypt_result.is_ok());
}
