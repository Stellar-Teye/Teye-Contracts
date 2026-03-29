use soroban_sdk::{Address, Env};

use crate::contract::DelegationContract;
use crate::contract::DelegationContractClient;

// Helper (duplicate is fine for clarity in tests)
fn set_ledger_timestamp(env: &Env, timestamp: u64) {
    env.ledger().with_mut(|li| {
        li.timestamp = timestamp;
    });
}

#[test]
fn test_renew_delegation_before_expiry() {
    let env = Env::default();
    let contract_id = env.register_contract(None, DelegationContract);
    let client = DelegationContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let delegate = Address::generate(&env);

    let initial_expiry = 2000;
    let new_expiry = 3000;

    set_ledger_timestamp(&env, 1000);
    client.delegate_access(&owner, &delegate, &initial_expiry);

    // Renew before expiry
    set_ledger_timestamp(&env, 1500);
    client.renew_delegation(&owner, &delegate, &new_expiry);

    // Still valid after original expiry
    set_ledger_timestamp(&env, 2500);
    assert!(client.has_access(&delegate));

    // Expired after new expiry
    set_ledger_timestamp(&env, new_expiry + 1);
    assert!(!client.has_access(&delegate));
}

#[test]
#[should_panic]
fn test_renew_after_expiry_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, DelegationContract);
    let client = DelegationContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let delegate = Address::generate(&env);

    let expiry = 2000;

    set_ledger_timestamp(&env, 1000);
    client.delegate_access(&owner, &delegate, &expiry);

    // Move past expiry
    set_ledger_timestamp(&env, expiry + 1);

    // Should panic
    client.renew_delegation(&owner, &delegate, &3000);
}
