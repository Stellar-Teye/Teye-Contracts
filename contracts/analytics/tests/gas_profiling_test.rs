#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]

//! Gas profiling for the analytics engine.
//!
//! Measures CPU instructions and memory bytes via `env.cost_estimate().budget()`
//! for the three high-cost paths identified in issue #454:
//!   1. Aggregation logic (homomorphic sum + decrypt + DP noise per batch)
//!   2. `get_trend` cross-period queries (one storage read per time bucket)
//!   3. Year-scale query performance (52 weekly buckets)
//!
//! All profiling tests print results with `--nocapture` so CI captures them.
//! Regression guards assert hard ceilings and are non-ignored.

extern crate std;

use analytics::{
    homomorphic::{PaillierPrivateKey, PaillierPublicKey},
    AnalyticsContract, AnalyticsContractClient, MetricDimensions, MetricValue,
};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Vec};

// ── Shared keys ───────────────────────────────────────────────────────────────

const PUB_KEY: PaillierPublicKey = PaillierPublicKey { n: 33, nn: 1089, g: 34 };
const PRIV_KEY: PaillierPrivateKey = PaillierPrivateKey { lambda: 20, mu: 5 };

// ── Setup ─────────────────────────────────────────────────────────────────────

fn setup() -> (Env, AnalyticsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AnalyticsContract, ());
    let client = AnalyticsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let aggregator = Address::generate(&env);
    client.initialize(&admin, &aggregator, &PUB_KEY, &Some(PRIV_KEY));
    (env, client, aggregator)
}

fn dims(bucket: u64) -> MetricDimensions {
    MetricDimensions {
        region: Some(symbol_short!("NG")),
        age_band: Some(symbol_short!("A18_39")),
        condition: Some(symbol_short!("MYOPIA")),
        time_bucket: bucket,
    }
}

/// Build a batch of `n` encrypted values of `1` each.
fn batch(env: &Env, client: &AnalyticsContractClient, n: u32) -> Vec<i128> {
    let mut v = Vec::new(env);
    for _ in 0..n {
        v.push_back(client.encrypt(&1i128));
    }
    v
}

/// Measure cost and print a labelled line compatible with CI log scraping.
fn measure<F: FnOnce()>(env: &Env, label: &str, f: F) -> (u64, u64) {
    env.cost_estimate().budget().reset_default();
    f();
    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let mem = env.cost_estimate().budget().memory_bytes_cost();
    std::println!("[GAS] {label}: cpu={cpu}, mem={mem}");
    (cpu, mem)
}

// ── Aggregation cost profile ──────────────────────────────────────────────────

#[test]
fn bench_aggregate_records_batch_1() {
    let (env, client, aggregator) = setup();
    let kind = symbol_short!("REC_CNT");
    let d = dims(1_000);
    let records = batch(&env, &client, 1);

    measure(&env, "aggregate_records_batch_1", || {
        client.aggregate_records(&aggregator, &kind, &d, &records);
    });
}

#[test]
fn bench_aggregate_records_batch_10() {
    let (env, client, aggregator) = setup();
    let kind = symbol_short!("REC_CNT");
    let d = dims(1_001);
    let records = batch(&env, &client, 10);

    measure(&env, "aggregate_records_batch_10", || {
        client.aggregate_records(&aggregator, &kind, &d, &records);
    });
}

#[test]
fn bench_aggregate_records_batch_50() {
    let (env, client, aggregator) = setup();
    let kind = symbol_short!("REC_CNT");
    let d = dims(1_002);
    let records = batch(&env, &client, 50);

    measure(&env, "aggregate_records_batch_50", || {
        client.aggregate_records(&aggregator, &kind, &d, &records);
    });
}

/// The aggregation cost must scale at most linearly with batch size.
/// Batch of 50 must cost no more than 100× the baseline batch of 1.
#[test]
fn regression_aggregation_cost_scales_linearly() {
    let (env, client, aggregator) = setup();
    let kind = symbol_short!("REC_CNT");

    let (cpu_1, _) = measure(&env, "agg_scaling_batch_1", || {
        client.aggregate_records(&aggregator, &kind, &dims(2_000), &batch(&env, &client, 1));
    });

    let (cpu_50, _) = measure(&env, "agg_scaling_batch_50", || {
        client.aggregate_records(&aggregator, &kind, &dims(2_001), &batch(&env, &client, 50));
    });

    let ceiling = cpu_1.saturating_mul(100);
    std::println!("[REGRESSION] agg_scaling: cpu_1={cpu_1}, cpu_50={cpu_50}, ceiling={ceiling}");
    assert!(
        cpu_50 <= ceiling,
        "aggregation cost scaled super-linearly: batch_1={cpu_1}, batch_50={cpu_50}"
    );
}

// ── get_trend cross-period cost profile ──────────────────────────────────────

/// Seed one data point per time bucket in [start, end].
fn seed_trend_data(
    env: &Env,
    client: &AnalyticsContractClient,
    aggregator: &Address,
    start: u64,
    end: u64,
) {
    let kind = symbol_short!("REC_CNT");
    let records = batch(env, client, 1);
    for bucket in start..=end {
        client.aggregate_records(aggregator, &kind, &dims(bucket), &records);
    }
}

#[test]
fn bench_get_trend_single_bucket() {
    let (env, client, aggregator) = setup();
    seed_trend_data(&env, &client, &aggregator, 3_000, 3_000);

    let kind = symbol_short!("REC_CNT");
    measure(&env, "get_trend_1_bucket", || {
        let _ = client.get_trend(&kind, &Some(symbol_short!("NG")), &Some(symbol_short!("A18_39")), &Some(symbol_short!("MYOPIA")), &3_000, &3_000);
    });
}

#[test]
fn bench_get_trend_monthly_year() {
    let (env, client, aggregator) = setup();
    // 12 monthly buckets representing one year
    seed_trend_data(&env, &client, &aggregator, 4_000, 4_011);

    let kind = symbol_short!("REC_CNT");
    let (cpu, mem) = measure(&env, "get_trend_12_months", || {
        let trend = client.get_trend(&kind, &Some(symbol_short!("NG")), &Some(symbol_short!("A18_39")), &Some(symbol_short!("MYOPIA")), &4_000, &4_011);
        assert_eq!(trend.len(), 12);
    });

    // All 12 points must be returned with non-zero counts (we seeded them).
    let trend = client.get_trend(&kind, &Some(symbol_short!("NG")), &Some(symbol_short!("A18_39")), &Some(symbol_short!("MYOPIA")), &4_000, &4_011);
    for point in trend.iter() {
        assert_eq!(point.value.count, 1, "bucket {} has count 0", point.time_bucket);
    }
    let _ = (cpu, mem);
}

#[test]
fn bench_get_trend_weekly_year() {
    let (env, client, aggregator) = setup();
    // 52 weekly buckets representing one year
    seed_trend_data(&env, &client, &aggregator, 5_000, 5_051);

    let kind = symbol_short!("REC_CNT");
    let (cpu, mem) = measure(&env, "get_trend_52_weeks", || {
        let trend = client.get_trend(&kind, &Some(symbol_short!("NG")), &Some(symbol_short!("A18_39")), &Some(symbol_short!("MYOPIA")), &5_000, &5_051);
        assert_eq!(trend.len(), 52);
    });
    let _ = (cpu, mem);
}

// ── Cross-period comparison cost profile ─────────────────────────────────────

/// A cross-period comparison fetches two separate trend ranges and compares
/// their aggregated counts — the dominant cost path for analytics dashboards.
#[test]
fn bench_cross_period_comparison_h1_vs_h2() {
    let (env, client, aggregator) = setup();
    // H1: 26 weeks, H2: 26 weeks (full year split in half)
    seed_trend_data(&env, &client, &aggregator, 6_000, 6_025); // H1
    seed_trend_data(&env, &client, &aggregator, 6_026, 6_051); // H2

    let kind = symbol_short!("REC_CNT");

    let (cpu_h1, _) = measure(&env, "cross_period_H1_26w", || {
        let _ = client.get_trend(&kind, &Some(symbol_short!("NG")), &Some(symbol_short!("A18_39")), &Some(symbol_short!("MYOPIA")), &6_000, &6_025);
    });

    let (cpu_h2, _) = measure(&env, "cross_period_H2_26w", || {
        let _ = client.get_trend(&kind, &Some(symbol_short!("NG")), &Some(symbol_short!("A18_39")), &Some(symbol_short!("MYOPIA")), &6_026, &6_051);
    });

    // Both halves must have equal cost ± 10 % (symmetric workload).
    let diff = if cpu_h1 > cpu_h2 { cpu_h1 - cpu_h2 } else { cpu_h2 - cpu_h1 };
    let tolerance = cpu_h1 / 10;
    std::println!("[REGRESSION] cross_period_symmetry: h1={cpu_h1}, h2={cpu_h2}, diff={diff}, tol={tolerance}");
    assert!(
        diff <= tolerance,
        "cross-period query cost asymmetry too large: H1={cpu_h1}, H2={cpu_h2}"
    );
}

#[test]
fn bench_cross_period_comparison_year_vs_year() {
    let (env, client, aggregator) = setup();
    seed_trend_data(&env, &client, &aggregator, 7_000, 7_051); // year A
    seed_trend_data(&env, &client, &aggregator, 8_000, 8_051); // year B

    let kind = symbol_short!("REC_CNT");

    let (cpu_a, mem_a) = measure(&env, "cross_period_year_A_52w", || {
        let _ = client.get_trend(&kind, &Some(symbol_short!("NG")), &Some(symbol_short!("A18_39")), &Some(symbol_short!("MYOPIA")), &7_000, &7_051);
    });

    let (cpu_b, mem_b) = measure(&env, "cross_period_year_B_52w", || {
        let _ = client.get_trend(&kind, &Some(symbol_short!("NG")), &Some(symbol_short!("A18_39")), &Some(symbol_short!("MYOPIA")), &8_000, &8_051);
    });

    std::println!("[GAS] year_vs_year: year_A cpu={cpu_a} mem={mem_a}, year_B cpu={cpu_b} mem={mem_b}");

    // Both full-year queries must have equal cost ± 10 %.
    let diff = if cpu_a > cpu_b { cpu_a - cpu_b } else { cpu_b - cpu_a };
    let tolerance = cpu_a / 10;
    assert!(
        diff <= tolerance,
        "year-vs-year query cost asymmetry: A={cpu_a}, B={cpu_b}"
    );
}

// ── Year-scale benchmark ──────────────────────────────────────────────────────

/// Store one encrypted record per week for a year, then query all 52 buckets.
/// This is the primary "simulated year of data" benchmark from the checklist.
#[test]
fn bench_full_year_ingestion_and_query() {
    let (env, client, aggregator) = setup();
    let kind = symbol_short!("REC_CNT");

    // Ingestion: 52 weekly aggregate_records calls
    let (cpu_ingest, mem_ingest) = measure(&env, "full_year_ingestion_52w", || {
        for week in 0u64..52 {
            let records = batch(&env, &client, 1);
            client.aggregate_records(&aggregator, &kind, &dims(9_000 + week), &records);
        }
    });

    // Query: single get_trend over all 52 weeks
    let (cpu_query, mem_query) = measure(&env, "full_year_query_52w", || {
        let trend = client.get_trend(
            &kind,
            &Some(symbol_short!("NG")),
            &Some(symbol_short!("A18_39")),
            &Some(symbol_short!("MYOPIA")),
            &9_000,
            &9_051,
        );
        assert_eq!(trend.len(), 52);
    });

    std::println!(
        "[SUMMARY] full_year: ingest cpu={cpu_ingest} mem={mem_ingest}; query cpu={cpu_query} mem={mem_query}"
    );

    // Correctness: each bucket must have the seeded data.
    let trend = client.get_trend(
        &kind,
        &Some(symbol_short!("NG")),
        &Some(symbol_short!("A18_39")),
        &Some(symbol_short!("MYOPIA")),
        &9_000,
        &9_051,
    );
    assert_eq!(trend.len(), 52);
    for point in trend.iter() {
        assert_eq!(point.value.count, 1, "week {} missing data", point.time_bucket - 9_000);
    }
}

/// Verify that `get_metric` (single-bucket read) is cheaper than
/// `get_trend` over 52 buckets — ensuring per-bucket cost is meaningful.
#[test]
fn regression_trend_cost_exceeds_single_metric_read() {
    let (env, client, aggregator) = setup();
    let kind = symbol_short!("REC_CNT");
    seed_trend_data(&env, &client, &aggregator, 10_000, 10_051);

    let (cpu_single, _) = measure(&env, "regression_single_metric_read", || {
        let _ = client.get_metric(&kind, &dims(10_000));
    });

    let (cpu_trend_52, _) = measure(&env, "regression_trend_52w_read", || {
        let _ = client.get_trend(&kind, &Some(symbol_short!("NG")), &Some(symbol_short!("A18_39")), &Some(symbol_short!("MYOPIA")), &10_000, &10_051);
    });

    std::println!(
        "[REGRESSION] single_vs_trend: single={cpu_single}, trend_52={cpu_trend_52}"
    );
    assert!(
        cpu_trend_52 > cpu_single,
        "get_trend(52) should cost more than get_metric(1): trend={cpu_trend_52}, single={cpu_single}"
    );
}

// ── get_metric returns zero for unseeded buckets ──────────────────────────────

/// Querying a trend over unseeded buckets (cold storage) must return zero-value
/// MetricValues and must not panic — gas cost is also measured.
#[test]
fn bench_get_trend_cold_storage_52_buckets() {
    let (env, client, _aggregator) = setup();
    let kind = symbol_short!("REC_CNT");

    let (cpu, _) = measure(&env, "get_trend_cold_52w", || {
        let trend = client.get_trend(
            &kind,
            &Some(symbol_short!("NG")),
            &None,
            &None,
            &11_000,
            &11_051,
        );
        assert_eq!(trend.len(), 52);
        for point in trend.iter() {
            assert_eq!(point.value, MetricValue { count: 0, sum: 0, version: 0 });
        }
    });

    // Cold storage reads are expected to be cheaper than warm reads
    // because there is no deserialization of stored values.
    // Simply ensure the query completes without hitting the budget ceiling.
    let _ = cpu;
}
