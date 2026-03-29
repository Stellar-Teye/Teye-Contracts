use soroban_sdk::{Env};

use crate::contract::AnalyticsContract;
use crate::contract::AnalyticsContractClient;

fn setup() -> (Env, AnalyticsContractClient<'static>) {
    let env = Env::default();
    let contract_id = env.register_contract(None, AnalyticsContract);
    let client = AnalyticsContractClient::new(&env, &contract_id);
    (env, client)
}

#[test]
fn test_aggregation_precision_large_dataset() {
    let (env, client) = setup();

    let mut expected_total: i128 = 0;

    // Simulate large dataset
    for i in 1..1000 {
        let value = (i * 1000) as i128; // fixed-point scaled
        expected_total += value;

        client.record_value(&value);
    }

    let aggregated = client.get_total();

    // Allow tiny tolerance if using fixed-point rounding
    let diff = (aggregated - expected_total).abs();

    assert!(diff <= 1, "Aggregation precision error too large");
}