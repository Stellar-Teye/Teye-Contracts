#![allow(clippy::unwrap_used)]
use soroban_sdk::{testutils::{Accounts, Ledger}, Env, Symbol, Address};
use audit::contract::{AuditContract, AuditContractClient};

#[test]
fn test_entry_timestamp_and_ledger_advance() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = env.accounts().generate();
    let contract_id = env.register_contract(None, AuditContract);
    let client = AuditContractClient::new(&env, &contract_id);
    client.initialize(&admin);
    let segment = Symbol::short("TIME");
    client.create_segment(&segment).unwrap();
    let actor = admin.clone().into();
    let action = Symbol::short("ACT");
    let target = Symbol::short("TGT");
    let result = Symbol::short("OK");

    // Set initial ledger timestamp
    env.ledger().set_timestamp(1_700_000_000);
    let seq1 = client.append_entry(&segment, &actor, &action, &target, &result).unwrap();
    let entries1 = client.get_entries(&segment).unwrap();
    assert_eq!(entries1[0].timestamp, 1_700_000_000);

    // Advance ledger timestamp
    env.ledger().set_timestamp(1_800_000_000);
    let seq2 = client.append_entry(&segment, &actor, &action, &target, &result).unwrap();
    let entries2 = client.get_entries(&segment).unwrap();
    assert_eq!(entries2[1].timestamp, 1_800_000_000);
}

#[test]
fn test_entry_expiry_bounds() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = env.accounts().generate();
    let contract_id = env.register_contract(None, AuditContract);
    let client = AuditContractClient::new(&env, &contract_id);
    client.initialize(&admin);
    let segment = Symbol::short("EXPIRY");
    client.create_segment(&segment).unwrap();
    let actor = admin.clone().into();
    let action = Symbol::short("ACT");
    let target = Symbol::short("TGT");
    let result = Symbol::short("OK");

    // Set a timestamp far in the future
    env.ledger().set_timestamp(u64::MAX);
    let seq = client.append_entry(&segment, &actor, &action, &target, &result).unwrap();
    let entries = client.get_entries(&segment).unwrap();
    assert_eq!(entries[0].timestamp, u64::MAX);

    // Set a timestamp at zero (epoch)
    env.ledger().set_timestamp(0);
    let seq2 = client.append_entry(&segment, &actor, &action, &target, &result).unwrap();
    let entries2 = client.get_entries(&segment).unwrap();
    assert_eq!(entries2[1].timestamp, 0);
}
