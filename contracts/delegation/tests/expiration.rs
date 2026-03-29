use soroban_sdk::{Address, Env};

use crate::contract::DelegationContract;
use crate::contract::DelegationContractClient;

// Helper
fn set_ledger_timestamp(env: &Env, timestamp: u64) {
    env.ledger().with_mut(|li| {
        li.timestamp = timestamp;
    });
}

#[test]
fn test_delegation_expires_exactly() {
    let env = Env::default();
    let contract_id = env.register_contract(None, DelegationContract);
    let client = DelegationContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let delegate = Address::generate(&env);

    let start_time = 1000;
    let expiry = 2000;

    set_ledger_timestamp(&env, start_time);

    client.delegate_access(&owner, &delegate, &expiry);

    // Before expiry
    set_ledger_timestamp(&env, expiry - 1);
    assert!(client.has_access(&delegate));

    // At expiry (should be expired)
    set_ledger_timestamp(&env, expiry);
    assert!(!client.has_access(&delegate));
}

#[test]
fn test_reject_after_expiration() {
    let env = Env::default();
    let contract_id = env.register_contract(None, DelegationContract);
    let client = DelegationContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let delegate = Address::generate(&env);

    let expiry = 2000;

    set_ledger_timestamp(&env, 1000);
    client.delegate_access(&owner, &delegate, &expiry);

    // After expiry
    set_ledger_timestamp(&env, expiry + 1);
    assert!(!client.has_access(&delegate));
}
