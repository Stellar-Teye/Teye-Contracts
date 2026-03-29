#![allow(clippy::unwrap_used, unused_variables)]
use audit::contract::{AuditContract, AuditContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, Symbol,
};

fn deploy() -> (Env, AuditContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(AuditContract, ());
    let client = AuditContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

#[test]
fn test_entry_timestamp_and_ledger_advance() {
    let (env, client, admin) = deploy();
    let segment = Symbol::short("TIME");
    client.create_segment(&segment);
    let action = Symbol::short("ACT");
    let target = Symbol::short("TGT");
    let result_sym = Symbol::short("OK");

    // First entry at T=1_700_000_000.
    env.ledger().set_timestamp(1_700_000_000);
    let _seq1 = client.append_entry(&segment, &admin, &action, &target, &result_sym);
    let entries1 = client.get_entries(&segment);
    assert_eq!(entries1.get(0).unwrap().timestamp, 1_700_000_000);

    // Advance ledger: second entry must carry the later timestamp.
    env.ledger().set_timestamp(1_800_000_000);
    let _seq2 = client.append_entry(&segment, &admin, &action, &target, &result_sym);
    let entries2 = client.get_entries(&segment);
    assert_eq!(entries2.get(1).unwrap().timestamp, 1_800_000_000);
}

#[test]
fn test_sequence_numbers_increment_monotonically() {
    let (env, client, admin) = deploy();
    let segment = Symbol::short("SEQ");
    client.create_segment(&segment);
    let action = Symbol::short("ACT");
    let target = Symbol::short("TGT");
    let result_sym = Symbol::short("OK");

    env.ledger().set_timestamp(1_000);
    let seq1 = client.append_entry(&segment, &admin, &action, &target, &result_sym);

    env.ledger().set_timestamp(2_000);
    let seq2 = client.append_entry(&segment, &admin, &action, &target, &result_sym);

    env.ledger().set_timestamp(3_000);
    let seq3 = client.append_entry(&segment, &admin, &action, &target, &result_sym);

    assert_eq!(seq1, 1, "first entry has sequence 1");
    assert_eq!(seq2, 2, "second entry has sequence 2");
    assert_eq!(seq3, 3, "third entry has sequence 3");
}

#[test]
fn test_same_ledger_timestamp_allowed_for_multiple_entries() {
    // Multiple events in the same ledger slot share a timestamp — this is valid
    // and must NOT be rejected as out-of-order.
    let (env, client, admin) = deploy();
    let segment = Symbol::short("SAME");
    client.create_segment(&segment);
    let action = Symbol::short("A");
    let target = Symbol::short("T");
    let result_sym = Symbol::short("OK");

    env.ledger().set_timestamp(5_000);
    client.append_entry(&segment, &admin, &action, &target, &result_sym);
    client.append_entry(&segment, &admin, &action, &target, &result_sym);

    let entries = client.get_entries(&segment);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries.get(0).unwrap().timestamp, 5_000);
    assert_eq!(entries.get(1).unwrap().timestamp, 5_000);
}

#[test]
fn test_entry_count_increments_correctly() {
    let (env, client, admin) = deploy();
    let segment = Symbol::short("CNT");
    client.create_segment(&segment);
    let action = Symbol::short("A");
    let target = Symbol::short("T");
    let result_sym = Symbol::short("OK");

    assert_eq!(client.get_entry_count(&segment), 0);

    env.ledger().set_timestamp(100);
    client.append_entry(&segment, &admin, &action, &target, &result_sym);
    assert_eq!(client.get_entry_count(&segment), 1);

    env.ledger().set_timestamp(200);
    client.append_entry(&segment, &admin, &action, &target, &result_sym);
    assert_eq!(client.get_entry_count(&segment), 2);
}

#[test]
fn test_paginated_retrieval_via_get_entries() {
    // Append 5 entries and verify full retrieval in one call.
    let (env, client, admin) = deploy();
    let segment = Symbol::short("PAGE");
    client.create_segment(&segment);
    let action = Symbol::short("A");
    let target = Symbol::short("T");
    let result_sym = Symbol::short("OK");

    for ts in [100u64, 200, 300, 400, 500] {
        env.ledger().set_timestamp(ts);
        client.append_entry(&segment, &admin, &action, &target, &result_sym);
    }

    let entries = client.get_entries(&segment);
    assert_eq!(entries.len(), 5, "all 5 entries must be retrievable");

    // Verify timestamps are in ascending order (chronological).
    for (i, ts) in [100u64, 200, 300, 400, 500].iter().enumerate() {
        assert_eq!(
            entries.get(i as u32).unwrap().timestamp,
            *ts,
            "entry {i} must have timestamp {ts}"
        );
    }
}
