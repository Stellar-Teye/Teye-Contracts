//! Timestamp Manipulation & Expiry Bounds Tests for `emr_bridge`
//!
//! Validates that:
//! - `registered_at`, `timestamp`, and `verified_at` fields faithfully capture
//!   the ledger timestamp at the exact moment of each operation.
//! - Advancing the ledger clock does NOT retroactively mutate stored timestamp fields.
//! - Boundary timestamps (0 and u64::MAX) are stored and retrieved without overflow
//!   or silent truncation.
//! - Chronological ordering is preserved: verification timestamps are always >=
//!   exchange timestamps when time advances monotonically.
//! - Suspended providers are rejected at ANY timestamp, simulating authorization expiry.
//! - A full provider→exchange→verification lifecycle produces strictly ordered timestamps.

use emr_bridge::{
    types::{DataExchangeRecord, DataFormat, EmrSystem, ExchangeDirection, SyncStatus},
    EmrBridgeContract, EmrBridgeContractClient, EmrBridgeError,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String, Vec,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Initialise a fresh contract and return (env, client, admin).
fn setup() -> (Env, EmrBridgeContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(EmrBridgeContract, ());
    let client = EmrBridgeContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

/// Register a provider and activate it.  Returns the provider_id as a `soroban_sdk::String`.
fn register_active_provider(
    env: &Env,
    client: &EmrBridgeContractClient,
    admin: &Address,
    id: &str,
) -> String {
    let pid = String::from_str(env, id);
    client.register_provider(
        admin,
        &pid,
        &String::from_str(env, &format!("Provider {id}")),
        &EmrSystem::EpicFhir,
        &String::from_str(env, "https://emr.example.com/fhir"),
        &DataFormat::FhirR4,
    );
    client.activate_provider(admin, &pid);
    pid
}

/// Record a single data exchange and return the resulting `DataExchangeRecord`.
fn do_exchange(
    env: &Env,
    client: &EmrBridgeContractClient,
    admin: &Address,
    exchange_id: &str,
    provider_id: &String,
    patient_id: &str,
) -> DataExchangeRecord {
    client.record_data_exchange(
        admin,
        &String::from_str(env, exchange_id),
        provider_id,
        &String::from_str(env, patient_id),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(env, "Patient"),
        &String::from_str(env, &format!("hash_{exchange_id}")),
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. TIMESTAMP CAPTURE CORRECTNESS
//    Each operation must snapshot the ledger clock at call time.
// ═══════════════════════════════════════════════════════════════════════════════

/// `EmrProvider::registered_at` must equal the ledger timestamp at registration.
#[test]
fn test_provider_registered_at_captures_ledger_timestamp() {
    let (env, client, admin) = setup();
    let ts: u64 = 1_000_000;
    env.ledger().set_timestamp(ts);

    let pid = String::from_str(&env, "ts-capture-provider");
    client.register_provider(
        &admin,
        &pid,
        &String::from_str(&env, "TS Capture Provider"),
        &EmrSystem::EpicFhir,
        &String::from_str(&env, "https://emr.example.com/fhir"),
        &DataFormat::FhirR4,
    );

    let provider = client.get_provider(&pid);
    assert_eq!(
        provider.registered_at, ts,
        "registered_at must equal the ledger timestamp at registration time"
    );
}

/// `DataExchangeRecord::timestamp` must equal the ledger timestamp at exchange creation.
#[test]
fn test_exchange_timestamp_set_at_creation_time() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "ex-ts-provider");

    let ts: u64 = 5_000;
    env.ledger().set_timestamp(ts);

    let record = do_exchange(&env, &client, &admin, "ex-ts-001", &pid, "pat-ts");
    assert_eq!(
        record.timestamp, ts,
        "DataExchangeRecord::timestamp must equal the ledger timestamp at creation"
    );
}

/// `SyncVerification::verified_at` must equal the ledger timestamp at the verification call.
#[test]
fn test_verification_timestamp_captures_time_of_call() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "ver-ts-provider");

    env.ledger().set_timestamp(100);
    do_exchange(&env, &client, &admin, "ver-ts-ex", &pid, "pat-ver");

    // Advance ledger before calling verify_sync
    let verify_ts: u64 = 9_000;
    env.ledger().set_timestamp(verify_ts);

    let hash = String::from_str(&env, "consistent_hash");
    let verification = client.verify_sync(
        &admin,
        &String::from_str(&env, "ver-ts-v1"),
        &String::from_str(&env, "ver-ts-ex"),
        &hash,
        &hash,
        &Vec::new(&env),
    );

    assert_eq!(
        verification.verified_at, verify_ts,
        "verified_at must equal the ledger timestamp at the moment verify_sync is called"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. TIMESTAMP IMMUTABILITY AFTER LEDGER ADVANCE
//    Stored timestamps are frozen at creation; ledger advances must not alter them.
// ═══════════════════════════════════════════════════════════════════════════════

/// Advancing the ledger clock after registration must NOT change `registered_at`.
#[test]
fn test_provider_registered_at_immutable_after_time_advance() {
    let (env, client, admin) = setup();
    let registration_ts: u64 = 200;
    env.ledger().set_timestamp(registration_ts);

    let pid = String::from_str(&env, "immutable-provider");
    client.register_provider(
        &admin,
        &pid,
        &String::from_str(&env, "Immutable Provider"),
        &EmrSystem::CernerMillennium,
        &String::from_str(&env, "https://cerner.example.com"),
        &DataFormat::Hl7V2,
    );

    // Advance time significantly, then re-fetch
    env.ledger().set_timestamp(99_999_999);

    let provider = client.get_provider(&pid);
    assert_eq!(
        provider.registered_at, registration_ts,
        "registered_at must remain frozen at creation time after ledger advances"
    );
}

/// Updating exchange status after a time advance must NOT alter `DataExchangeRecord::timestamp`.
#[test]
fn test_exchange_timestamp_immutable_after_status_update() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "imm-ex-provider");

    let creation_ts: u64 = 777;
    env.ledger().set_timestamp(creation_ts);
    let eid = String::from_str(&env, "imm-ex-001");
    do_exchange(&env, &client, &admin, "imm-ex-001", &pid, "pat-imm");

    // Advance time and mutate status — timestamp must be unaffected
    env.ledger().set_timestamp(u64::MAX);
    client.update_exchange_status(&admin, &eid, &SyncStatus::Completed);

    let record = client.get_exchange(&eid);
    assert_eq!(
        record.timestamp, creation_ts,
        "exchange timestamp must remain frozen at creation time after a status update"
    );
}

/// Re-fetching a verification record after a ledger advance must preserve `verified_at`.
#[test]
fn test_verification_timestamp_immutable_after_time_advance() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "imm-ver-provider");

    do_exchange(&env, &client, &admin, "imm-ver-ex", &pid, "pat-imm-ver");

    let verify_ts: u64 = 4_444;
    env.ledger().set_timestamp(verify_ts);

    let hash = String::from_str(&env, "immut_hash");
    client.verify_sync(
        &admin,
        &String::from_str(&env, "imm-ver-v1"),
        &String::from_str(&env, "imm-ver-ex"),
        &hash,
        &hash,
        &Vec::new(&env),
    );

    // Advance time, then re-read the verification record
    env.ledger().set_timestamp(u64::MAX);
    let v = client.get_verification(&String::from_str(&env, "imm-ver-v1"));
    assert_eq!(
        v.verified_at, verify_ts,
        "verified_at must remain frozen at the time verify_sync was called"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. BOUNDARY TIMESTAMPS — EPOCH ZERO AND u64::MAX
// ═══════════════════════════════════════════════════════════════════════════════

/// Registration at ledger timestamp 0 (genesis) must store registered_at = 0.
#[test]
fn test_provider_registration_at_epoch_zero() {
    let (env, client, admin) = setup();
    env.ledger().set_timestamp(0);

    let pid = String::from_str(&env, "genesis-provider");
    client.register_provider(
        &admin,
        &pid,
        &String::from_str(&env, "Genesis Provider"),
        &EmrSystem::Custom,
        &String::from_str(&env, "https://genesis.example.com"),
        &DataFormat::Custom,
    );

    let provider = client.get_provider(&pid);
    assert_eq!(provider.registered_at, 0, "registered_at must be 0 at genesis timestamp");
}

/// Exchange creation at ledger timestamp 0 must store timestamp = 0.
#[test]
fn test_exchange_timestamp_at_epoch_zero() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "genesis-ex-provider");

    env.ledger().set_timestamp(0);
    let record = do_exchange(&env, &client, &admin, "genesis-ex", &pid, "pat-genesis");
    assert_eq!(record.timestamp, 0, "exchange timestamp must be 0 at genesis timestamp");
}

/// Exchange creation at u64::MAX must be stored without overflow or wrap-around.
#[test]
fn test_exchange_timestamp_at_max_u64_no_overflow() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "max-ts-provider");

    env.ledger().set_timestamp(u64::MAX);
    let record = do_exchange(&env, &client, &admin, "max-ts-ex", &pid, "pat-max");

    assert_eq!(
        record.timestamp,
        u64::MAX,
        "exchange timestamp must equal u64::MAX without overflow"
    );
    // Ensure it did not silently wrap around to 0
    assert_ne!(record.timestamp, 0);
}

/// Verification at u64::MAX timestamp is stored without overflow.
#[test]
fn test_verification_timestamp_at_max_u64() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "max-ver-provider");

    env.ledger().set_timestamp(u64::MAX);
    do_exchange(&env, &client, &admin, "max-ver-ex", &pid, "pat-max-ver");

    let hash = String::from_str(&env, "max_hash");
    let verification = client.verify_sync(
        &admin,
        &String::from_str(&env, "max-ver-v1"),
        &String::from_str(&env, "max-ver-ex"),
        &hash,
        &hash,
        &Vec::new(&env),
    );

    assert_eq!(
        verification.verified_at,
        u64::MAX,
        "verified_at must equal u64::MAX without overflow"
    );
}

/// Provider registered at u64::MAX - 1 has correct registered_at.
#[test]
fn test_provider_registration_near_max_timestamp() {
    let (env, client, admin) = setup();
    let near_max = u64::MAX - 1;
    env.ledger().set_timestamp(near_max);

    let pid = String::from_str(&env, "near-max-provider");
    client.register_provider(
        &admin,
        &pid,
        &String::from_str(&env, "Near-Max Provider"),
        &EmrSystem::Athenahealth,
        &String::from_str(&env, "https://athena.example.com"),
        &DataFormat::CcdA,
    );

    let provider = client.get_provider(&pid);
    assert_eq!(
        provider.registered_at, near_max,
        "registered_at must equal u64::MAX - 1"
    );
    // Confirm it did not wrap to 0 or u64::MAX
    assert_ne!(provider.registered_at, 0);
    assert_ne!(provider.registered_at, u64::MAX);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. CHRONOLOGICAL ORDERING & EXPIRY-WINDOW SIMULATION
//    When ledger time advances monotonically between operations, the ordering
//    of stored timestamps must reflect the real event sequence.
// ═══════════════════════════════════════════════════════════════════════════════

/// `verified_at` must be >= `exchange.timestamp` when time advances between the two calls.
#[test]
fn test_verification_timestamp_not_before_exchange_timestamp() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "order-provider");

    let exchange_ts: u64 = 1_000;
    env.ledger().set_timestamp(exchange_ts);
    do_exchange(&env, &client, &admin, "order-ex", &pid, "pat-order");

    // Advance time — verification happens later
    let verify_ts: u64 = 5_000;
    env.ledger().set_timestamp(verify_ts);

    let hash = String::from_str(&env, "order_hash");
    let verification = client.verify_sync(
        &admin,
        &String::from_str(&env, "order-ver"),
        &String::from_str(&env, "order-ex"),
        &hash,
        &hash,
        &Vec::new(&env),
    );

    let exchange = client.get_exchange(&String::from_str(&env, "order-ex"));
    assert!(
        verification.verified_at >= exchange.timestamp,
        "verified_at ({}) must be >= exchange.timestamp ({})",
        verification.verified_at,
        exchange.timestamp
    );
}

/// Multiple exchanges at different ledger timestamps must each preserve the correct timestamp.
#[test]
fn test_multiple_exchanges_preserve_distinct_timestamps() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "multi-ts-provider");
    let patient = String::from_str(&env, "pat-multi");

    let cases: &[(u64, &str)] = &[
        (100, "multi-ex-a"),
        (500, "multi-ex-b"),
        (u64::MAX / 2, "multi-ex-c"),
        (u64::MAX - 1, "multi-ex-d"),
    ];

    for (ts, eid) in cases {
        env.ledger().set_timestamp(*ts);
        client.record_data_exchange(
            &admin,
            &String::from_str(&env, eid),
            &pid,
            &patient,
            &ExchangeDirection::Import,
            &DataFormat::FhirR4,
            &String::from_str(&env, "Patient"),
            &String::from_str(&env, &format!("hash_{eid}")),
        );
    }

    for (expected_ts, eid) in cases {
        let record = client.get_exchange(&String::from_str(&env, eid));
        assert_eq!(
            record.timestamp, *expected_ts,
            "exchange '{eid}' must have timestamp {expected_ts}"
        );
    }
}

/// Two providers registered at distinct times must have distinct `registered_at` values.
#[test]
fn test_providers_registered_at_different_times_have_distinct_timestamps() {
    let (env, client, admin) = setup();

    let ts1: u64 = 1_111;
    env.ledger().set_timestamp(ts1);
    let pid1 = String::from_str(&env, "ts-p1");
    client.register_provider(
        &admin,
        &pid1,
        &String::from_str(&env, "Provider One"),
        &EmrSystem::EpicFhir,
        &String::from_str(&env, "https://ep1.example.com"),
        &DataFormat::FhirR4,
    );

    let ts2: u64 = 2_222;
    env.ledger().set_timestamp(ts2);
    let pid2 = String::from_str(&env, "ts-p2");
    client.register_provider(
        &admin,
        &pid2,
        &String::from_str(&env, "Provider Two"),
        &EmrSystem::CernerMillennium,
        &String::from_str(&env, "https://ep2.example.com"),
        &DataFormat::Hl7V2,
    );

    let p1 = client.get_provider(&pid1);
    let p2 = client.get_provider(&pid2);
    assert_eq!(p1.registered_at, ts1);
    assert_eq!(p2.registered_at, ts2);
    assert_ne!(
        p1.registered_at, p2.registered_at,
        "providers registered at different times must have different registered_at values"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. TIME REGRESSION & ROLLBACK SAFETY
//    Setting the ledger to a lower timestamp must not corrupt historic records.
// ═══════════════════════════════════════════════════════════════════════════════

/// Simulated ledger time regression must not mutate already-stored exchange timestamps.
#[test]
fn test_time_regression_does_not_corrupt_exchange_timestamp() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "rollback-provider");

    let high_ts: u64 = 9_999;
    env.ledger().set_timestamp(high_ts);
    do_exchange(&env, &client, &admin, "rollback-ex", &pid, "pat-rollback");

    // Simulate ledger rollback
    env.ledger().set_timestamp(1);
    let record = client.get_exchange(&String::from_str(&env, "rollback-ex"));
    assert_eq!(
        record.timestamp, high_ts,
        "exchange timestamp must survive a simulated ledger time regression"
    );
}

/// Simulated rollback must not change a provider's `registered_at`.
#[test]
fn test_time_regression_does_not_corrupt_provider_registered_at() {
    let (env, client, admin) = setup();

    let high_ts: u64 = 50_000;
    env.ledger().set_timestamp(high_ts);

    let pid = String::from_str(&env, "rollback-prov");
    client.register_provider(
        &admin,
        &pid,
        &String::from_str(&env, "Rollback Provider"),
        &EmrSystem::Allscripts,
        &String::from_str(&env, "https://allscripts.example.com"),
        &DataFormat::Hl7V2,
    );

    // Rollback to an earlier time
    env.ledger().set_timestamp(0);

    let provider = client.get_provider(&pid);
    assert_eq!(
        provider.registered_at, high_ts,
        "registered_at must survive a simulated ledger time regression"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. AUTHORIZATION EXPIRY SIMULATION
//    A suspended provider must be rejected at any timestamp, simulating the
//    expiry of its integration authorization.
// ═══════════════════════════════════════════════════════════════════════════════

/// A suspended provider must be rejected even far in the future (timestamp = u64::MAX).
#[test]
fn test_suspended_provider_rejected_at_max_timestamp() {
    let (env, client, admin) = setup();
    let pid = register_active_provider(&env, &client, &admin, "suspend-exp-provider");

    client.suspend_provider(&admin, &pid);

    // Attempt exchange at the maximum possible future timestamp
    env.ledger().set_timestamp(u64::MAX);
    let result = client.try_record_data_exchange(
        &admin,
        &String::from_str(&env, "suspended-max-ex"),
        &pid,
        &String::from_str(&env, "pat-suspended"),
        &ExchangeDirection::Import,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Patient"),
        &String::from_str(&env, "hash_suspended"),
    );

    assert_eq!(
        result,
        Err(Ok(EmrBridgeError::ProviderNotActive)),
        "suspended provider must be rejected regardless of timestamp"
    );
}

/// A pending (never activated) provider must also be rejected at any timestamp.
#[test]
fn test_pending_provider_rejected_at_arbitrary_timestamp() {
    let (env, client, admin) = setup();

    let pid = String::from_str(&env, "pending-provider");
    client.register_provider(
        &admin,
        &pid,
        &String::from_str(&env, "Pending Provider"),
        &EmrSystem::EpicFhir,
        &String::from_str(&env, "https://ep.example.com"),
        &DataFormat::FhirR4,
    );
    // Deliberately skip activate_provider

    env.ledger().set_timestamp(12_345_678);
    let result = client.try_record_data_exchange(
        &admin,
        &String::from_str(&env, "pending-ex"),
        &pid,
        &String::from_str(&env, "pat-pending"),
        &ExchangeDirection::Export,
        &DataFormat::FhirR4,
        &String::from_str(&env, "Observation"),
        &String::from_str(&env, "hash_pending"),
    );

    assert_eq!(
        result,
        Err(Ok(EmrBridgeError::ProviderNotActive)),
        "pending provider must be rejected regardless of timestamp"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. FULL LIFECYCLE — STRICT CHRONOLOGICAL ORDERING
// ═══════════════════════════════════════════════════════════════════════════════

/// A complete provider→exchange→verification workflow must produce timestamps that satisfy:
///   provider.registered_at  <  exchange.timestamp  <  verification.verified_at
/// when each operation is performed at a strictly increasing ledger time.
#[test]
fn test_full_lifecycle_timestamps_are_strictly_ordered() {
    let (env, client, admin) = setup();

    let reg_ts: u64 = 1_000;
    env.ledger().set_timestamp(reg_ts);
    let pid = register_active_provider(&env, &client, &admin, "lifecycle-provider");

    let ex_ts: u64 = 2_000;
    env.ledger().set_timestamp(ex_ts);
    do_exchange(&env, &client, &admin, "lifecycle-ex", &pid, "pat-lc");

    let ver_ts: u64 = 3_000;
    env.ledger().set_timestamp(ver_ts);
    let hash = String::from_str(&env, "lifecycle_hash");
    let verification = client.verify_sync(
        &admin,
        &String::from_str(&env, "lifecycle-ver"),
        &String::from_str(&env, "lifecycle-ex"),
        &hash,
        &hash,
        &Vec::new(&env),
    );

    let provider = client.get_provider(&String::from_str(&env, "lifecycle-provider"));
    let exchange = client.get_exchange(&String::from_str(&env, "lifecycle-ex"));

    // Absolute value checks
    assert_eq!(provider.registered_at, reg_ts);
    assert_eq!(exchange.timestamp, ex_ts);
    assert_eq!(verification.verified_at, ver_ts);

    // Ordering checks
    assert!(
        provider.registered_at < exchange.timestamp,
        "registration must precede exchange (registered_at={}, exchange.timestamp={})",
        provider.registered_at,
        exchange.timestamp
    );
    assert!(
        exchange.timestamp < verification.verified_at,
        "exchange must precede verification (exchange.timestamp={}, verified_at={})",
        exchange.timestamp,
        verification.verified_at
    );
}
