#![allow(clippy::unwrap_used, clippy::expect_used)]

extern crate std;

use analytics::{
    homomorphic::{PaillierPrivateKey, PaillierPublicKey},
    AnalyticsContract, AnalyticsContractClient, InitializedEvent, MetricAggregatedEvent,
    MetricDimensions, MetricImportedEvent, MetricValue,
};
use soroban_sdk::{
    contract, contractimpl, symbol_short, testutils::Address as _, testutils::Events, xdr::ScVal,
    Address, Env, IntoVal, Symbol, TryFromVal, Val, Vec,
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

    client.initialize(&admin, &aggregator, &pub_key, &Some(priv_key));

    (env, client, admin, aggregator)
}

#[contract]
pub struct MockMetricSourceContract;

#[contractimpl]
impl MockMetricSourceContract {
    pub fn read_metric(_env: Env, _kind: Symbol, _dims: MetricDimensions) -> MetricValue {
        MetricValue { count: 9, sum: 27 }
    }
}

fn assert_last_event<T>(env: &Env, expected_topics: Vec<Val>, expected_data: &T)
where
    T: Clone + IntoVal<Env, Val>,
{
    let events = env.events().all();
    let event = events.events().last().unwrap();
    let soroban_sdk::xdr::ContractEventBody::V0(body) = &event.body;

    let mut expected_topics_scval = std::vec::Vec::new();
    for topic in expected_topics.iter() {
        expected_topics_scval.push(ScVal::try_from_val(env, &topic).unwrap());
    }
    assert_eq!(body.topics.as_slice(), expected_topics_scval.as_slice());

    let expected_val: Val = expected_data.clone().into_val(env);
    let expected_data_scval = ScVal::try_from_val(env, &expected_val).unwrap();
    assert_eq!(body.data, expected_data_scval);
}

#[test]
fn test_initialize_emits_expected_event() {
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

    let expected_topics: Vec<Val> = (
        symbol_short!("INIT"),
        admin.clone(),
        aggregator.clone(),
    )
        .into_val(&env);
    let expected_data = InitializedEvent { admin, aggregator };

    assert_eq!(env.events().all().len(), 1);
    assert_last_event(&env, expected_topics, &expected_data);
}

#[test]
fn test_aggregate_records_emits_metric_aggregated_event() {
    let (env, client, _admin, aggregator) = setup();
    let events_before = env.events().all().len();

    let kind = symbol_short!("REC_CNT");
    let dims = MetricDimensions {
        region: Some(symbol_short!("NG")),
        age_band: Some(symbol_short!("A18_39")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket: 77,
    };

    let mut records = Vec::new(&env);
    records.push_back(client.encrypt(&4));
    records.push_back(client.encrypt(&6));

    client.aggregate_records(&aggregator, &kind, &dims, &records);

    let expected_topics: Vec<Val> = (
        symbol_short!("M_AGG"),
        kind.clone(),
        aggregator.clone(),
    )
        .into_val(&env);
    let expected_data = MetricAggregatedEvent {
        caller: aggregator,
        kind: kind.clone(),
        dims: dims.clone(),
        value: client.get_metric(&kind, &dims),
    };

    assert_eq!(env.events().all().len(), events_before + 1);
    assert_last_event(&env, expected_topics, &expected_data);
}

#[test]
fn test_import_metric_from_source_emits_metric_imported_event() {
    let (env, client, _admin, aggregator) = setup();
    let source_id = env.register(MockMetricSourceContract, ());
    let events_before = env.events().all().len();

    let kind = symbol_short!("REC_CNT");
    let dims = MetricDimensions {
        region: Some(symbol_short!("EU")),
        age_band: None,
        condition: None,
        time_bucket: 88,
    };

    let imported = client.import_metric_from_source(&aggregator, &source_id, &kind, &dims);

    let expected_topics: Vec<Val> = (
        symbol_short!("M_IMPORT"),
        kind.clone(),
        aggregator.clone(),
    )
        .into_val(&env);
    let expected_data = MetricImportedEvent {
        caller: aggregator,
        source: source_id,
        kind: kind.clone(),
        dims: dims.clone(),
        value: imported,
    };

    assert_eq!(env.events().all().len(), events_before + 1);
    assert_last_event(&env, expected_topics, &expected_data);
}
