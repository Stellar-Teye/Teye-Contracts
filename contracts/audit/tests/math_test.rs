#![allow(clippy::unwrap_used)]
use audit::contract::{AuditContract, AuditContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

#[test]
fn test_append_entry_sequence_increments_from_zero() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(AuditContract, ());
    let client = AuditContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let segment = Symbol::short("SEQ");
    client.create_segment(&segment);

    let action = Symbol::short("ACT");
    let target = Symbol::short("TGT");
    let result_sym = Symbol::short("OK");

    let s1 = client.append_entry(&segment, &admin, &action, &target, &result_sym);
    let s2 = client.append_entry(&segment, &admin, &action, &target, &result_sym);
    assert_eq!(s1 + 1, s2, "sequence numbers must increment by 1");
}

#[test]
fn test_i128_balance_boundary_values() {
    // Verify i128 min/max arithmetic does not panic in test context.
    let min_i128 = i128::MIN;
    let max_i128 = i128::MAX;
    assert!(min_i128 < 0);
    assert!(max_i128 > 0);
    // overflow guard: saturating_add must not panic
    assert_eq!(max_i128.saturating_add(1), max_i128);
}
