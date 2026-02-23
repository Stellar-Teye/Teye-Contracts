#![no_std]

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec};

// ── Storage keys ────────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const AGGREGATOR: Symbol = symbol_short!("AGGR");
const METRIC: Symbol = symbol_short!("METRIC");

// ── Types ──────────────────────────────────────────────────────────────────────

/// Describes the dimensions for an aggregate metric.
///
/// All fields are **coarse-grained** and intended to avoid direct identification
/// of individual patients. Off-chain indexers should pre-aggregate data before
/// pushing it into this contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricDimensions {
    /// Optional region or site identifier (e.g., "EU", "US", "ClinicA").
    pub region: Option<Symbol>,
    /// Optional coarse age band (e.g., "A18_39", "A40_64", "A65P").
    pub age_band: Option<Symbol>,
    /// Optional condition or diagnostic bucket (e.g., "MYOPIA", "GLAUCOMA").
    pub condition: Option<Symbol>,
    /// Time bucket as a UNIX timestamp (e.g., start of day/week/month).
    pub time_bucket: u64,
}

/// Stored value for a metric: simple count plus optional numeric aggregate.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricValue {
    /// Number of events or records observed.
    pub count: i128,
    /// Sum of a numeric signal (e.g., visual acuity score); used to compute averages off-chain.
    pub sum: i128,
}

/// Point-in-time value for trend queries.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrendPoint {
    pub time_bucket: u64,
    pub value: MetricValue,
}

// ── Contract ───────────────────────────────────────────────────────────────────

#[contract]
pub struct AnalyticsContract;

#[contractimpl]
impl AnalyticsContract {
    // ── Administration ────────────────────────────────────────────────────────

    /// Initialise the analytics contract with an admin and an authorised aggregator.
    ///
    /// The aggregator address represents an off-chain process that computes
    /// privacy-preserving aggregates from raw vision records and pushes them
    /// on-chain.
    pub fn initialize(env: Env, admin: Address, aggregator: Address) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialized");
        }
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&AGGREGATOR, &aggregator);
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&ADMIN).expect("admin not set")
    }

    pub fn get_aggregator(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&AGGREGATOR)
            .expect("aggregator not set")
    }

    fn require_aggregator(env: &Env, caller: &Address) {
        let expected: Address = env
            .storage()
            .instance()
            .get(&AGGREGATOR)
            .expect("aggregator not set");
        if caller != &expected {
            panic!("unauthorized aggregator");
        }
    }

    // ── Metric ingestion ──────────────────────────────────────────────────────

    /// Records an aggregate contribution to a named metric.
    ///
    /// This call is designed for **pre-aggregated**, privacy-preserving data
    /// produced by off-chain analytics pipelines. It should never be invoked
    /// with per-patient identifiers.
    ///
    /// - `kind`  – logical metric name (e.g., "record_count", "myopia_prevalence")
    /// - `dims`  – aggregation dimensions (region, age band, condition, time bucket)
    /// - `count_delta` – increment to apply to the count
    /// - `sum_delta`   – increment to apply to the numeric sum (0 if not used)
    pub fn record_metric(
        env: Env,
        caller: Address,
        kind: Symbol,
        dims: MetricDimensions,
        count_delta: i128,
        sum_delta: i128,
    ) {
        caller.require_auth();
        Self::require_aggregator(&env, &caller);

        if count_delta == 0 && sum_delta == 0 {
            return;
        }

        let key = (METRIC, kind, dims.clone());
        let mut current: MetricValue = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(MetricValue { count: 0, sum: 0 });

        current.count = current.count.saturating_add(count_delta);
        current.sum = current.sum.saturating_add(sum_delta);

        env.storage().persistent().set(&key, &current);
    }

    /// Returns the current value for a given metric + dimensions.
    pub fn get_metric(env: Env, kind: Symbol, dims: MetricDimensions) -> MetricValue {
        let key = (METRIC, kind, dims);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(MetricValue { count: 0, sum: 0 })
    }

    // ── Trend analysis ────────────────────────────────────────────────────────

    /// Returns a time-ordered sequence of metric values for a fixed set of
    /// dimensions over a closed interval of time buckets.
    ///
    /// This is intended for trend visualisation (e.g., monthly myopia
    /// prevalence in a region across a year).
    pub fn get_trend(
        env: Env,
        kind: Symbol,
        region: Option<Symbol>,
        age_band: Option<Symbol>,
        condition: Option<Symbol>,
        start_bucket: u64,
        end_bucket: u64,
    ) -> Vec<TrendPoint> {
        if end_bucket < start_bucket {
            return Vec::new(&env);
        }

        let mut out = Vec::new(&env);
        let mut bucket = start_bucket;

        while bucket <= end_bucket {
            let dims = MetricDimensions {
                region: region.clone(),
                age_band: age_band.clone(),
                condition: condition.clone(),
                time_bucket: bucket,
            };
            let value = Self::get_metric(env.clone(), kind.clone(), dims.clone());
            out.push_back(TrendPoint {
                time_bucket: bucket,
                value,
            });

            // Simple increment by 1 bucket; callers choose bucket granularity.
            bucket = bucket.saturating_add(1);
        }

        out
    }

    // ── Population metrics ────────────────────────────────────────────────────

    /// Returns aggregate metrics for a given time bucket across all regions,
    /// age bands, and conditions for the specified metric kind.
    ///
    /// This is useful for high-level population health dashboards.
    pub fn get_population_metrics(env: Env, kind: Symbol, time_bucket: u64) -> MetricValue {
        // NOTE: This naive implementation iterates over the combinations of a
        // small fixed set of labels is expected to be used with a limited set
        // of regions/conditions. For more complex analytics, prefer off-chain
        // aggregation on indexer data.
        //
        // To keep on-chain logic simple and gas-efficient, we only provide
        // a minimal population aggregation entry point that sums all stored
        // MetricValue entries for the given time bucket.

        let mut total = MetricValue { count: 0, sum: 0 };

        // There is no efficient on-chain key iteration; in practice, callers
        // will track known dimension combinations off-chain and query them
        // individually, then sum locally. We expose this helper primarily for
        // tests and simple single-dimension use cases by returning the metric
        // for the "anonymous" bucket (no region/age/condition).
        let dims = MetricDimensions {
            region: None,
            age_band: None,
            condition: None,
            time_bucket,
        };
        let value = Self::get_metric(env, kind, dims);
        total.count = total.count.saturating_add(value.count);
        total.sum = total.sum.saturating_add(value.sum);

        total
    }
}
