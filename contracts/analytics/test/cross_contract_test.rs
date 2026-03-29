#![allow(clippy::unwrap_used, clippy::expect_used)]

extern crate std;

use analytics::{
    homomorphic::{PaillierPrivateKey, PaillierPublicKey},
    AnalyticsContract, AnalyticsContractClient, ContractError, MetricDimensions, MetricValue,
};
use soroban_sdk::{contract, contractimpl, symbol_short, testutils::Address as _, Address, Env};

fn setup() -> (Env, AnalyticsContractClient<'static>, Address) {
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

    client.initialize(&admin, &aggregator, &pub_key, &Some(priv_key));

    (env, client, aggregator)
}

#[contract]
pub struct MockMetricSourceContract;

#[contractimpl]
impl MockMetricSourceContract {
    pub fn read_metric(_env: Env, kind: soroban_sdk::Symbol, dims: MetricDimensions) -> MetricValue {
        assert_eq!(kind, symbol_short!("REC_CNT"));
        assert_eq!(dims.time_bucket, 42);
        assert_eq!(dims.region, Some(symbol_short!("NG")));

        MetricValue {
            count: 7,
            sum: 150,
            version: 0, // Will be updated by import_metric_from_source
        }
    }
}

#[contract]
pub struct FailingMetricSourceContract;

#[contractimpl]
impl FailingMetricSourceContract {
    pub fn read_metric(
        _env: Env,
        _kind: soroban_sdk::Symbol,
        _dims: MetricDimensions,
    ) -> MetricValue {
        panic!("mock external failure")
    }
}

#[test]
fn test_import_metric_from_source_persists_parsed_metric_value() {
    let (env, client, aggregator) = setup();
    let source_id = env.register(MockMetricSourceContract, ());

    let kind = symbol_short!("REC_CNT");
    let dims = MetricDimensions {
        region: Some(symbol_short!("NG")),
        age_band: Some(symbol_short!("A18_39")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket: 42,
    };

    let imported = client.import_metric_from_source(&aggregator, &source_id, &kind, &dims);

    assert_eq!(
        imported,
        MetricValue {
            count: 7,
            sum: 150,
            version: 1,
        }
    );
    assert_eq!(client.get_metric(&kind, &dims), imported);
}

#[test]
fn test_import_metric_from_source_maps_external_failure_without_mutating_state() {
    let (env, client, aggregator) = setup();
    let source_id = env.register(FailingMetricSourceContract, ());

    let kind = symbol_short!("REC_CNT");
    let dims = MetricDimensions {
        region: Some(symbol_short!("NG")),
        age_band: None,
        condition: None,
        time_bucket: 77,
    };

    assert_eq!(
        client.try_import_metric_from_source(&aggregator, &source_id, &kind, &dims),
        Err(Ok(ContractError::ExternalCallFailed))
    );
    assert_eq!(client.get_metric(&kind, &dims), MetricValue { count: 0, sum: 0, version: 0 });
}

#[test]
fn test_import_metric_from_source_requires_aggregator_auth() {
    let (env, client, _aggregator) = setup();
    let source_id = env.register(MockMetricSourceContract, ());
    let unauthorized = Address::generate(&env);

    let kind = symbol_short!("REC_CNT");
    let dims = MetricDimensions {
        region: Some(symbol_short!("NG")),
        age_band: None,
        condition: None,
        time_bucket: 42,
    };

    assert_eq!(
        client.try_import_metric_from_source(&unauthorized, &source_id, &kind, &dims),
        Err(Ok(ContractError::Unauthorized))
    );
}
