# Analytics Contract

This document describes the on-chain analytics capabilities provided by the `analytics` Soroban contract.

## Goals

- Provide **privacy-preserving**, aggregate analytics for vision care data.
- Support **trend analysis** over time for key metrics (e.g., record volume, condition prevalence).
- Enable **population health** insights without storing personally identifiable information (PII) on-chain.

## Design Overview

The analytics contract is intentionally minimal and focused on **aggregated, pre-processed data**:

- Off-chain indexers and data pipelines:
  - Ingest detailed vision records (e.g., from the `vision_records` contract and external systems).
  - Perform de-identification, bucketing, and aggregation (by region, age band, condition, time bucket).
  - Push only aggregated metrics on-chain via the `AnalyticsContract`.
- On-chain contract:
  - Stores `MetricValue` entries keyed by high-level `MetricDimensions` and a logical metric name (`kind`).
  - Exposes read-only views for:
    - Point-in-time metric values.
    - Trend series across time buckets.
    - Simple population metrics (for the anonymous/no-dimension bucket).

No patient identifiers or raw measurement values are stored on-chain; only **aggregates** are recorded.

## Data Structures

The contract defines the following key types:

- **`MetricDimensions`**:
  - `region: Option<Symbol>` — coarse location tag (e.g., `"EU"`, `"US"`, `"ClinicA"`).
  - `age_band: Option<Symbol>` — coarse age band (`"A18_39"`, `"A40_64"`, `"A65P"`, etc.).
  - `condition: Option<Symbol>` — diagnostic bucket (`"MYOPIA"`, `"GLAUCOMA"`, etc.).
  - `time_bucket: u64` — integer time bucket (e.g., day/week/month start as UNIX timestamp).

- **`MetricValue`**:
  - `count: i128` — number of events/records in the bucket.
  - `sum: i128` — sum of a numeric signal (e.g., visual acuity score); used to compute averages off-chain.

- **`TrendPoint`**:
  - `time_bucket: u64`
  - `value: MetricValue`

## Key Contract Methods

- **Administration**
  - `initialize(admin, aggregator)`:
    - Sets the admin and the **aggregator** address (an off-chain analytics service).
  - `get_admin()` / `get_aggregator()`:
    - Return current configuration.

- **Aggregation**
  - `record_metric(caller, kind, dims, count_delta, sum_delta)`:
    - Only callable by the configured `aggregator`.
    - Applies deltas to `MetricValue` for the given metric `kind` and `MetricDimensions`.
    - Intended for **pre-aggregated** data; never pass per-patient details.

  - `get_metric(kind, dims) -> MetricValue`:
    - Returns the stored value, or `{ count: 0, sum: 0 }` if none exists.

- **Trend Analysis**
  - `get_trend(kind, region, age_band, condition, start_bucket, end_bucket) -> Vec<TrendPoint>`:
    - Returns a time-ordered series of `TrendPoint` for the given dimensions over the closed interval [`start_bucket`, `end_bucket`].
    - Each point corresponds to a `MetricDimensions` with the specified region/age/condition and a varying `time_bucket`.

- **Population Metrics**
  - `get_population_metrics(kind, time_bucket) -> MetricValue`:
    - Returns the value for the **anonymous bucket** (`region=None`, `age_band=None`, `condition=None`) at `time_bucket`.
    - Useful for global dashboards (total counts, overall averages).

## Privacy Considerations

- The contract:
  - Does **not** accept addresses or patient IDs as part of any public interface.
  - Only stores aggregated counts and sums.
  - Relies on the off-chain aggregator to:
    - Enforce minimum cohort sizes (k-anonymity style constraints).
    - Apply noise or other privacy techniques if needed.
- Consumers should treat on-chain analytics as:
  - A verifiable, append-only log of **aggregated** health metrics.
  - Not as a raw data lake.

## Example Flows

### Recording Monthly Myopia Prevalence

1. Off-chain indexer scans new records for the month, filters for `"MYOPIA"`.
2. Groups by `(region="EU", age_band="A18_39", time_bucket=<month_start>)`.
3. Computes:
   - `count` = number of qualifying records
   - `sum` = sum of some numeric score, if applicable (or `0` if unused)
4. Calls:

   ```text
   record_metric(
     caller      = aggregator,
     kind        = "myopia_prevalence",
     dims        = { region: "EU", age_band: "A18_39", condition: "MYOPIA", time_bucket: month_start },
     count_delta = count,
     sum_delta   = sum
   )
   ```

### Querying a Trend

To fetch a 12‑month trend of record volumes for `"US"`:

1. Compute `start_bucket` and `end_bucket` for the 12-month range (e.g., month indices).
2. Call:

   ```text
   get_trend(
     kind        = "record_count",
     region      = Some("US"),
     age_band    = None,
     condition   = None,
     start_bucket,
     end_bucket,
   )
   ```

3. Plot the returned `TrendPoint` values in a dashboard.

## Testing

Analytics behaviour is covered by unit tests in:

- `contracts/analytics/src/test.rs`

These tests verify:

- Initialisation and admin/aggregator configuration
- Metric accumulation for a given `MetricDimensions`
- Trend generation across time buckets
- Population metrics for the anonymous bucket

