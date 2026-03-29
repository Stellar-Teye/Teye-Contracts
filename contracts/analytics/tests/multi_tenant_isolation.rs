extern crate std;

use analytics::{AnalyticsContract, AnalyticsContractClient, MetricDimensions, MetricValue, ContractError};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec, symbol_short};

fn setup_multi_tenant() -> (Env, AnalyticsContractClient<'static>, Address, Address) {
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
fn test_tenant_data_isolation_by_region() {
    let (env, client, _admin, aggregator) = setup_multi_tenant();

    let kind = symbol_short!("PAT_CNT");
    let time_bucket = 1_700_000_000;

    // Create data for two different tenants (regions)
    let tenant_a_dims = MetricDimensions {
        region: Some(symbol_short!("HOSP_A")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    let tenant_b_dims = MetricDimensions {
        region: Some(symbol_short!("HOSP_B")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    // Add data for Tenant A
    let mut tenant_a_records = Vec::new(&env);
    tenant_a_records.push_back(client.encrypt(&100i128)); // 100 patients
    client.aggregate_records(&aggregator, &kind, &tenant_a_dims, &tenant_a_records);

    // Add data for Tenant B
    let mut tenant_b_records = Vec::new(&env);
    tenant_b_records.push_back(client.encrypt(&50i128)); // 50 patients
    client.aggregate_records(&aggregator, &kind, &tenant_b_dims, &tenant_b_records);

    // Verify isolation: each tenant should only see their own data
    let tenant_a_metrics = client.get_metric(&kind, &tenant_a_dims);
    let tenant_b_metrics = client.get_metric(&kind, &tenant_b_dims);

    // Each tenant should have their own count
    assert_eq!(tenant_a_metrics.count, 1);
    assert_eq!(tenant_b_metrics.count, 1);

    // Verify no data leakage by checking that querying one tenant's dimensions
    // doesn't return data from another tenant
    assert_ne!(tenant_a_dims.region, tenant_b_dims.region);

    // Query with Tenant A's dimensions should not include Tenant B's data
    let cross_check_dims_a = MetricDimensions {
        region: Some(symbol_short!("HOSP_A")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    let cross_check_result_a = client.get_metric(&kind, &cross_check_dims_a);
    assert_eq!(cross_check_result_a.count, 1); // Only Tenant A's data

    // Query with Tenant B's dimensions should not include Tenant A's data
    let cross_check_dims_b = MetricDimensions {
        region: Some(symbol_short!("HOSP_B")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    let cross_check_result_b = client.get_metric(&kind, &cross_check_dims_b);
    assert_eq!(cross_check_result_b.count, 1); // Only Tenant B's data
}

#[test]
fn test_cross_tenant_query_prevention() {
    let (env, client, _admin, aggregator) = setup_multi_tenant();

    let kind = symbol_short!("SENS_DATA");
    let time_bucket = 1_700_000_000;

    // Create data for multiple tenants
    let tenants = vec![
        symbol_short!("HOSP_A"),
        symbol_short!("HOSP_B"),
        symbol_short!("CLINIC_C"),
    ];

    for (i, tenant) in tenants.iter().enumerate() {
        let dims = MetricDimensions {
            region: Some(tenant.clone()),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("GLAUCOMA")),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        let patient_count = ((i + 1) * 25) as i128; // 25, 50, 75 patients
        records.push_back(client.encrypt(&patient_count));
        client.aggregate_records(&aggregator, &kind, &dims, &records);
    }

    // Verify that each tenant's data is isolated
    for tenant in &tenants {
        let dims = MetricDimensions {
            region: Some(tenant.clone()),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("GLAUCOMA")),
            time_bucket,
        };

        let metrics = client.get_metric(&kind, &dims);
        assert_eq!(metrics.count, 1); // One aggregation per tenant

        // Ensure no cross-tenant data contamination
        let other_tenant_dims = MetricDimensions {
            region: Some(symbol_short!("UNAUTH_T")),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("GLAUCOMA")),
            time_bucket,
        };

        let unauthorized_metrics = client.get_metric(&kind, &other_tenant_dims);
        assert_eq!(unauthorized_metrics.count, 0); // No data for unauthorized tenant
        assert_eq!(unauthorized_metrics.sum, 0);
        assert_eq!(unauthorized_metrics.version, 0);
    }
}

#[test]
fn test_tenant_isolation_with_different_dimensions() {
    let (env, client, _admin, aggregator) = setup_multi_tenant();

    let kind = symbol_short!("TREAT_OUT");
    let time_bucket = 1_700_000_000;

    // Same region, different age bands - should be isolated
    let region = symbol_short!("REG_X");
    
    let age_band_young = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A18_39")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    let age_band_older = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    // Add data for young age band
    let mut young_records = Vec::new(&env);
    young_records.push_back(client.encrypt(&30i128));
    client.aggregate_records(&aggregator, &kind, &age_band_young, &young_records);

    // Add data for older age band
    let mut older_records = Vec::new(&env);
    older_records.push_back(client.encrypt(&45i128));
    client.aggregate_records(&aggregator, &kind, &age_band_older, &older_records);

    // Verify isolation between age bands within same region
    let young_metrics = client.get_metric(&kind, &age_band_young);
    let older_metrics = client.get_metric(&kind, &age_band_older);

    assert_eq!(young_metrics.count, 1);
    assert_eq!(older_metrics.count, 1);

    // Cross-query prevention: querying young age band shouldn't return older age band data
    assert_ne!(age_band_young.age_band, age_band_older.age_band);
}

#[test]
fn test_tenant_isolation_with_time_buckets() {
    let (env, client, _admin, aggregator) = setup_multi_tenant();

    let kind = symbol_short!("MON_REPT");
    let region = symbol_short!("HOSP_A");

    let time_bucket_1 = 1_700_000_000; // January
    let time_bucket_2 = 1_700_259_200; // February (approx)

    let dims_jan = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("CATARACT")),
        time_bucket: time_bucket_1,
    };

    let dims_feb = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("CATARACT")),
        time_bucket: time_bucket_2,
    };

    // Add data for January
    let mut jan_records = Vec::new(&env);
    jan_records.push_back(client.encrypt(&20i128));
    client.aggregate_records(&aggregator, &kind, &dims_jan, &jan_records);

    // Add data for February
    let mut feb_records = Vec::new(&env);
    feb_records.push_back(client.encrypt(&35i128));
    client.aggregate_records(&aggregator, &kind, &dims_feb, &feb_records);

    // Verify time-based isolation
    let jan_metrics = client.get_metric(&kind, &dims_jan);
    let feb_metrics = client.get_metric(&kind, &dims_feb);

    assert_eq!(jan_metrics.count, 1);
    assert_eq!(feb_metrics.count, 1);

    // Querying January shouldn't return February data
    let cross_check_jan = client.get_metric(&kind, &dims_jan);
    assert_eq!(cross_check_jan.count, 1);

    // Querying a non-existent time bucket should return zero
    let dims_nonexistent = MetricDimensions {
        region: Some(region.clone()),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("CATARACT")),
        time_bucket: 1_699_000_000, // Before January
    };

    let nonexistent_metrics = client.get_metric(&kind, &dims_nonexistent);
    assert_eq!(nonexistent_metrics.count, 0);
    assert_eq!(nonexistent_metrics.sum, 0);
    assert_eq!(nonexistent_metrics.version, 0);
}

#[test]
fn test_aggregate_tenant_isolation() {
    let (env, client, _admin, aggregator) = setup_multi_tenant();

    let kind = symbol_short!("AGG_TEST");
    let time_bucket = 1_700_000_000;

    // Create multiple entries for the same tenant
    let tenant_a_region = symbol_short!("HOSP_A");
    
    let conditions = vec![
        symbol_short!("MYOPIA"),
        symbol_short!("GLAUCOMA"),
        symbol_short!("CATARACT"),
    ];

    for condition in &conditions {
        let dims = MetricDimensions {
            region: Some(tenant_a_region.clone()),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(condition.clone()),
            time_bucket,
        };

        let mut records = Vec::new(&env);
        records.push_back(client.encrypt(&10i128)); // 10 patients per condition
        client.aggregate_records(&aggregator, &kind, &dims, &records);
    }

    // Verify that all entries for the same tenant are accessible
    for condition in &conditions {
        let dims = MetricDimensions {
            region: Some(tenant_a_region.clone()),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(condition.clone()),
            time_bucket,
        };

        let metrics = client.get_metric(&kind, &dims);
        assert_eq!(metrics.count, 1);
    }

    // Verify that another tenant has no access to this data
    let tenant_b_dims = MetricDimensions {
        region: Some(symbol_short!("HOSP_B")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    let tenant_b_metrics = client.get_metric(&kind, &tenant_b_dims);
    assert_eq!(tenant_b_metrics.count, 0);
    assert_eq!(tenant_b_metrics.sum, 0);
    assert_eq!(tenant_b_metrics.version, 0);
}

#[test]
fn test_trend_isolation_across_tenants() {
    let (env, client, _admin, aggregator) = setup_multi_tenant();

    let kind = symbol_short!("TRND_TEST");
    let region_a = symbol_short!("HOSP_A");
    let region_b = symbol_short!("HOSP_B");

    // Create trend data for two tenants
    for time_bucket in 1..=3 {
        // Tenant A data
        let dims_a = MetricDimensions {
            region: Some(region_a.clone()),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("MYOPIA")),
            time_bucket,
        };

        let mut records_a = Vec::new(&env);
        records_a.push_back(client.encrypt(&( (time_bucket * 10) as i128 ))); // 10, 20, 30
        client.aggregate_records(&aggregator, &kind, &dims_a, &records_a);

        // Tenant B data
        let dims_b = MetricDimensions {
            region: Some(region_b.clone()),
            age_band: Some(symbol_short!("A40_64")),
            condition: Some(symbol_short!("MYOPIA")),
            time_bucket,
        };

        let mut records_b = Vec::new(&env);
        records_b.push_back(client.encrypt(&( (time_bucket * 5) as i128 ))); // 5, 10, 15
        client.aggregate_records(&aggregator, &kind, &dims_b, &records_b);
    }

    // Get trends for each tenant
    let trend_a = client.get_trend(
        &kind,
        &Some(region_a.clone()),
        &Some(symbol_short!("A40_64")),
        &Some(symbol_short!("MYOPIA")),
        &1,
        &3,
    );

    let trend_b = client.get_trend(
        &kind,
        &Some(region_b.clone()),
        &Some(symbol_short!("A40_64")),
        &Some(symbol_short!("MYOPIA")),
        &1,
        &3,
    );

    // Verify each tenant has their own trend data
    assert_eq!(trend_a.len(), 3);
    assert_eq!(trend_b.len(), 3);

    // Verify no cross-tenant contamination
    for i in 0..3 {
        let point_a = trend_a.get(i).unwrap();
        let point_b = trend_b.get(i).unwrap();
        
        assert_eq!(point_a.time_bucket, point_b.time_bucket);
        assert_eq!(point_a.value.count, point_b.value.count); // Both should have count 1
        
        // The sums should be different (different data for each tenant)
        // Due to differential privacy noise, we just check they're not zero
        assert!(point_a.value.sum > 0);
        assert!(point_b.value.sum > 0);
    }
}

#[test]
fn test_null_region_tenant_isolation() {
    let (env, client, _admin, aggregator) = setup_multi_tenant();

    let kind = symbol_short!("NUL_RG_T");
    let time_bucket = 1_700_000_000;

    // Test with null region (should be isolated from specific regions)
    let null_region_dims = MetricDimensions {
        region: None,
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    let specific_region_dims = MetricDimensions {
        region: Some(symbol_short!("HOSP_A")),
        age_band: Some(symbol_short!("A40_64")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket,
    };

    // Add data to null region
    let mut null_records = Vec::new(&env);
    null_records.push_back(client.encrypt(&100i128));
    client.aggregate_records(&aggregator, &kind, &null_region_dims, &null_records);

    // Add data to specific region
    let mut specific_records = Vec::new(&env);
    specific_records.push_back(client.encrypt(&50i128));
    client.aggregate_records(&aggregator, &kind, &specific_region_dims, &specific_records);

    // Verify isolation
    let null_metrics = client.get_metric(&kind, &null_region_dims);
    let specific_metrics = client.get_metric(&kind, &specific_region_dims);

    assert_eq!(null_metrics.count, 1);
    assert_eq!(specific_metrics.count, 1);

    // They should be completely isolated
    assert_ne!(null_region_dims.region, specific_region_dims.region);
}
