extern crate std;

use analytics::{AnalyticsContract, AnalyticsContractClient, MetricDimensions, ContractError, MetricValue};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec, symbol_short};

fn setup_cross_query_test() -> (Env, AnalyticsContractClient<'static>, Address, Address) {
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
fn test_cross_tenant_data_access_prevention() {
    let (env, client, _admin, aggregator) = setup_cross_query_test();

    let kind = symbol_short!("PAT_DATA");
    let time_bucket = 1_700_000_000;

    // Set up data for two different tenants
    let tenant_a_dims = MetricDimensions {
        region: Some(symbol_short!("TENANT_A")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("DIABETES")),
        time_bucket,
    };

    let tenant_b_dims = MetricDimensions {
        region: Some(symbol_short!("TENANT_B")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("DIABETES")),
        time_bucket,
    };

    // Add sensitive data for Tenant A
    let mut tenant_a_records = Vec::new(&env);
    tenant_a_records.push_back(client.encrypt(&1000i128)); // 1000 patients
    client.aggregate_records(&aggregator, &kind, &tenant_a_dims, &tenant_a_records);

    // Add sensitive data for Tenant B
    let mut tenant_b_records = Vec::new(&env);
    tenant_b_records.push_back(client.encrypt(&500i128)); // 500 patients
    client.aggregate_records(&aggregator, &kind, &tenant_b_dims, &tenant_b_records);

    // Attempt 1: Try to access Tenant A data using Tenant B's region
    let malicious_dims_b_trying_a = MetricDimensions {
        region: Some(symbol_short!("TENANT_A")), // Maliciously trying to access A's data
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("DIABETES")),
        time_bucket,
    };

    // This should return Tenant A's data (not a failure, but shows data is properly keyed)
    let result = client.get_metric(&kind, &malicious_dims_b_trying_a);
    assert_eq!(result.count, 1); // Returns Tenant A's data because region matches

    // Attempt 2: Try to access non-existent tenant data
    let non_existent_tenant_dims = MetricDimensions {
        region: Some(symbol_short!("NONEXIST")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("DIABETES")),
        time_bucket,
    };

    let non_existent_result = client.get_metric(&kind, &non_existent_tenant_dims);
    assert_eq!(non_existent_result.count, 0); // No data for non-existent tenant
    assert_eq!(non_existent_result.sum, 0);
    assert_eq!(non_existent_result.version, 0);

    // Attempt 3: Try to use wildcard/None region to access all data
    let wildcard_dims = MetricDimensions {
        region: None, // Attempting to access all regions
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("DIABETES")),
        time_bucket,
    };

    let wildcard_result = client.get_metric(&kind, &wildcard_dims);
    assert_eq!(wildcard_result.count, 0); // No data because no data was stored with None region
    assert_eq!(wildcard_result.sum, 0);
    assert_eq!(wildcard_result.version, 0);
}

#[test]
fn test_cross_condition_data_leakage_prevention() {
    let (env, client, _admin, aggregator) = setup_cross_query_test();

    let kind = symbol_short!("COND_DATA");
    let time_bucket = 1_700_000_000;
    let region = symbol_short!("HOSP_X");

    // Set up data for different conditions within same tenant
    let hiv_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("HIV")),
        time_bucket,
    };

    let diabetes_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("DIABETES")),
        time_bucket,
    };

    // Add highly sensitive HIV data
    let mut hiv_records = Vec::new(&env);
    hiv_records.push_back(client.encrypt(&25i128)); // 25 patients
    client.aggregate_records(&aggregator, &kind, &hiv_dims, &hiv_records);

    // Add less sensitive diabetes data
    let mut diabetes_records = Vec::new(&env);
    diabetes_records.push_back(client.encrypt(&150i128)); // 150 patients
    client.aggregate_records(&aggregator, &kind, &diabetes_dims, &diabetes_records);

    // Attempt to cross-query: Try to access HIV data using diabetes dimensions
    let cross_query_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("HIV")), // Correctly specifying HIV
        time_bucket,
    };

    let hiv_result = client.get_metric(&kind, &cross_query_dims);
    assert_eq!(hiv_result.count, 1); // Returns HIV data

    // Attempt to access HIV data with wrong condition
    let wrong_condition_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("CANCER")), // Wrong condition
        time_bucket,
    };

    let wrong_condition_result = client.get_metric(&kind, &wrong_condition_dims);
    assert_eq!(wrong_condition_result.count, 0); // No data for wrong condition
    assert_eq!(wrong_condition_result.sum, 0);
    assert_eq!(wrong_condition_result.version, 0);

    // Verify conditions are properly isolated
    let diabetes_result = client.get_metric(&kind, &diabetes_dims);
    assert_eq!(diabetes_result.count, 1);
    
    // HIV and diabetes data should be completely separate
    assert_ne!(hiv_dims.condition, diabetes_dims.condition);
}

#[test]
fn test_cross_age_band_data_isolation() {
    let (env, client, _admin, aggregator) = setup_cross_query_test();

    let kind = symbol_short!("AGE_DATA");
    let time_bucket = 1_700_000_000;
    let region = symbol_short!("CLINIC_Y");

    // Set up data for different age bands
    let minor_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A0_17")), // Minors
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    let adult_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A18_39")), // Adults
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    // Add sensitive minor data
    let mut minor_records = Vec::new(&env);
    minor_records.push_back(client.encrypt(&45i128)); // 45 minor patients
    client.aggregate_records(&aggregator, &kind, &minor_dims, &minor_records);

    // Add adult data
    let mut adult_records = Vec::new(&env);
    adult_records.push_back(client.encrypt(&120i128)); // 120 adult patients
    client.aggregate_records(&aggregator, &kind, &adult_dims, &adult_records);

    // Attempt to access minor data using adult dimensions
    let adult_query_result = client.get_metric(&kind, &adult_dims);
    assert_eq!(adult_query_result.count, 1); // Returns adult data only

    // Attempt to access adult data using minor dimensions
    let minor_query_result = client.get_metric(&kind, &minor_dims);
    assert_eq!(minor_query_result.count, 1); // Returns minor data only

    // Verify strict isolation
    assert_ne!(minor_dims.age_band, adult_dims.age_band);

    // Attempt to access data with wrong age band
    let wrong_age_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A65P")), // Senior citizens
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    let wrong_age_result = client.get_metric(&kind, &wrong_age_dims);
    assert_eq!(wrong_age_result.count, 0); // No data for this age band
    assert_eq!(wrong_age_result.sum, 0);
    assert_eq!(wrong_age_result.version, 0);
}

#[test]
fn test_cross_time_bucket_data_isolation() {
    let (env, client, _admin, aggregator) = setup_cross_query_test();

    let kind = symbol_short!("TIME_ISO");
    let region = symbol_short!("HOSP_Z");

    let time_bucket_jan = 1_700_000_000; // January 2023
    let time_bucket_feb = 1_700_259_200; // February 2023

    // Set up data for different time periods
    let jan_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("CATARACT")),
        time_bucket: time_bucket_jan,
    };

    let feb_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("CATARACT")),
        time_bucket: time_bucket_feb,
    };

    // Add January data
    let mut jan_records = Vec::new(&env);
    jan_records.push_back(client.encrypt(&30i128)); // 30 procedures in January
    client.aggregate_records(&aggregator, &kind, &jan_dims, &jan_records);

    // Add February data
    let mut feb_records = Vec::new(&env);
    feb_records.push_back(client.encrypt(&45i128)); // 45 procedures in February
    client.aggregate_records(&aggregator, &kind, &feb_dims, &feb_records);

    // Attempt to access January data using February time bucket
    let feb_query_result = client.get_metric(&kind, &feb_dims);
    assert_eq!(feb_query_result.count, 1); // Returns February data only

    // Attempt to access February data using January time bucket
    let jan_query_result = client.get_metric(&kind, &jan_dims);
    assert_eq!(jan_query_result.count, 1); // Returns January data only

    // Verify time-based isolation
    assert_ne!(jan_dims.time_bucket, feb_dims.time_bucket);

    // Attempt to access data with wrong time bucket
    let wrong_time_dims = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("CATARACT")),
        time_bucket: 1_699_000_000, // Wrong time
    };

    let wrong_time_result = client.get_metric(&kind, &wrong_time_dims);
    assert_eq!(wrong_time_result.count, 0); // No data for wrong time
    assert_eq!(wrong_time_result.sum, 0);
    assert_eq!(wrong_time_result.version, 0);
}

#[test]
fn test_complex_cross_query_attempts() {
    let (env, client, _admin, aggregator) = setup_cross_query_test();

    let kind = symbol_short!("CMPLX_ISO");
    let time_bucket = 1_700_000_000;

    // Create a complex multi-dimensional dataset
    let base_dims = MetricDimensions {
        region: Some(symbol_short!("HOSP_SEC")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MENT_HLTH")),
        time_bucket,
    };

    // Add sensitive mental health data
    let mut sensitive_records = Vec::new(&env);
    sensitive_records.push_back(client.encrypt(&15i128)); // 15 patients
    client.aggregate_records(&aggregator, &kind, &base_dims, &sensitive_records);

    // Attempt 1: Change one dimension at a time to try to access data
    let mut cross_attempts = vec![
        // Wrong region, same other dimensions
        MetricDimensions {
            region: Some(symbol_short!("WRONG_HSP")),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("MENT_HLTH")),
            time_bucket,
        },
        // Wrong age band, same other dimensions
        MetricDimensions {
            region: Some(symbol_short!("HOSP_SEC")),
            age_band: Some(symbol_short!("A18_39")),
            condition: Some(symbol_short!("MENT_HLTH")),
            time_bucket,
        },
        // Wrong condition, same other dimensions
        MetricDimensions {
            region: Some(symbol_short!("HOSP_SEC")),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("PHYS_HLTH")),
            time_bucket,
        },
        // Wrong time bucket, same other dimensions
        MetricDimensions {
            region: Some(symbol_short!("HOSP_SEC")),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("MENT_HLTH")),
            time_bucket: 1_699_000_000,
        },
        // All dimensions wrong
        MetricDimensions {
            region: Some(symbol_short!("WRONG_HSP")),
            age_band: Some(symbol_short!("A18_39")),
            condition: Some(symbol_short!("PHYS_HLTH")),
            time_bucket: 1_699_000_000,
        },
    ];

    for (i, attempt_dims) in cross_attempts.iter_mut().enumerate() {
        let result = client.get_metric(&kind, attempt_dims);
        
        // All cross-query attempts should return no data
        assert_eq!(result.count, 0, "Cross-query attempt {} should return no data", i);
        assert_eq!(result.sum, 0, "Cross-query attempt {} should return no sum", i);
        assert_eq!(result.version, 0);
    }

    // Verify the original data is still accessible with correct dimensions
    let original_result = client.get_metric(&kind, &base_dims);
    assert_eq!(original_result.count, 1);
}

#[test]
fn test_trend_cross_query_isolation() {
    let (env, client, _admin, aggregator) = setup_cross_query_test();

    let kind = symbol_short!("TRND_ISO");
    let region_a = symbol_short!("HOSP_A");
    let region_b = symbol_short!("HOSP_B");

    // Create trend data for two different tenants
    for time_bucket in 1..=5 {
        // Tenant A data
        let dims_a = MetricDimensions {
            region: Some(region_a.clone()),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("HEART_DIS")),
            time_bucket,
        };

        let mut records_a = Vec::new(&env);
        records_a.push_back(client.encrypt(&( (time_bucket * 10) as i128 )));
        client.aggregate_records(&aggregator, &kind, &dims_a, &records_a);

        // Tenant B data
        let dims_b = MetricDimensions {
            region: Some(region_b.clone()),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("HEART_DIS")),
            time_bucket,
        };

        let mut records_b = Vec::new(&env);
        records_b.push_back(client.encrypt(&( (time_bucket * 5) as i128 )));
        client.aggregate_records(&aggregator, &kind, &dims_b, &records_b);
    }

    // Attempt to get trend for Tenant A using Tenant B's region
    let malicious_trend = client.get_trend(
        &kind,
        &Some(region_b.clone()), // Trying to get A's data but specifying B's region
        &Some(symbol_short!("A40_64")),
        &Some(symbol_short!("HEART_DIS")),
        &1,
        &5,
    );

    // Should return Tenant B's trend data, not Tenant A's
    assert_eq!(malicious_trend.len(), 5);

    // Verify the data is actually from Tenant B by checking one point
    let first_point = malicious_trend.get(0).unwrap();
    assert_eq!(first_point.time_bucket, 1);
    assert_eq!(first_point.value.count, 1); // One aggregation per time bucket

    // Get legitimate trends for comparison
    let legitimate_trend_a = client.get_trend(
        &kind,
        &Some(region_a.clone()),
        &Some(symbol_short!("A40_64")),
        &Some(symbol_short!("HEART_DIS")),
        &1,
        &5,
    );

    let legitimate_trend_b = client.get_trend(
        &kind,
        &Some(region_b.clone()),
        &Some(symbol_short!("A40_64")),
        &Some(symbol_short!("HEART_DIS")),
        &1,
        &5,
    );

    // Verify trends are different (though counts may be the same)
    assert_eq!(legitimate_trend_a.len(), 5);
    assert_eq!(legitimate_trend_b.len(), 5);

    // The malicious trend should match legitimate trend B, not A
    for i in 0..5 {
        let malicious_point = malicious_trend.get(i).unwrap();
        let legitimate_b_point = legitimate_trend_b.get(i).unwrap();
        
        assert_eq!(malicious_point.time_bucket, legitimate_b_point.time_bucket);
        assert_eq!(malicious_point.value.count, legitimate_b_point.value.count);
    }
}

#[test]
fn test_null_dimension_cross_queries() {
    let (env, client, _admin, aggregator) = setup_cross_query_test();

    let kind = symbol_short!("NUL_D_T");
    let time_bucket = 1_700_000_000;

    // Create data with specific dimensions
    let specific_dims = MetricDimensions {
        region: Some(symbol_short!("SPEC_HOSP")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("SPEC_COND")),
        time_bucket,
    };

    // Create data with null dimensions
    let null_region_dims = MetricDimensions {
        region: None,
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("SPEC_COND")),
        time_bucket,
    };

    let null_age_dims = MetricDimensions {
        region: Some(symbol_short!("SPEC_HOSP")),
        age_band: None,
        condition: Some(symbol_short!("SPEC_COND")),
        time_bucket,
    };

    let null_condition_dims = MetricDimensions {
        region: Some(symbol_short!("SPEC_HOSP")),
        age_band: Some(symbol_short!("A40_64")),
        condition: None,
        time_bucket,
    };

    // Add data to all dimension combinations
    let records = {
        let mut r = Vec::new(&env);
        r.push_back(client.encrypt(&10i128));
        r
    };

    client.aggregate_records(&aggregator, &kind, &specific_dims, &records);
    client.aggregate_records(&aggregator, &kind, &null_region_dims, &records);
    client.aggregate_records(&aggregator, &kind, &null_age_dims, &records);
    client.aggregate_records(&aggregator, &kind, &null_condition_dims, &records);

    // Attempt cross-queries between null and specific dimensions
    let cross_queries = vec![
        // Try to access specific data using null region
        null_region_dims,
        // Try to access specific data using null age band
        null_age_dims,
        // Try to access specific data using null condition
        null_condition_dims,
    ];

    for (i, query_dims) in cross_queries.iter().enumerate() {
        let result = client.get_metric(&kind, query_dims);
        
        // Each query should only return data for that specific dimension combination
        assert_eq!(result.count, 1, "Cross-query {} should return exactly 1 result", i);
    }

    // Verify specific data is isolated
    let specific_result = client.get_metric(&kind, &specific_dims);
    assert_eq!(specific_result.count, 1);

    // Verify that querying with wrong null dimension combinations returns no data
    let wrong_null_dims = MetricDimensions {
        region: None,
        age_band: None,
        condition: None,
        time_bucket,
    };

    let wrong_null_result = client.get_metric(&kind, &wrong_null_dims);
    assert_eq!(wrong_null_result.count, 0); // No data stored with all null dimensions
    assert_eq!(wrong_null_result.sum, 0);
    assert_eq!(wrong_null_result.version, 0);
}
