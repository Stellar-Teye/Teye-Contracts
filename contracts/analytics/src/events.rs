use soroban_sdk::{contracttype, Address, Symbol};
use crate::{MetricDimensions, MetricValue};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializedEvent {
    pub admin: Address,
    pub aggregator: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricImportedEvent {
    pub caller: Address,
    pub source: Address,
    pub kind: Symbol,
    pub dims: MetricDimensions,
    pub value: MetricValue,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricAggregatedEvent {
    pub caller: Address,
    pub kind: Symbol,
    pub dims: MetricDimensions,
    pub value: MetricValue,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyUpdatedEvent {
    pub admin: Address,
    pub effect: Symbol, // e.g., "AGGREGATOR", "KEYS"
    pub new_version: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataInvalidatedEvent {
    pub kind: Symbol,
    pub dims: MetricDimensions,
    pub old_version: u32,
    pub current_version: u32,
}
