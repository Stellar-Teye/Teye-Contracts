use soroban_sdk::{Address, Env};

use crate::contract::DelegationContract;
use crate::contract::DelegationContractClient;

// Helper (reuse pattern from other test files)
fn setup() -> (Env, DelegationContractClient<'static>) {
    let env = Env::default();
    let contract_id = env.register_contract(None, DelegationContract);
    let client = DelegationContractClient::new(&env, &contract_id);
    (env, client)
}

#[test]
fn test_recursive_revocation_full_chain() {
    let (env, client) = setup();

    let owner = Address::generate(&env);
    let delegate_a = Address::generate(&env);
    let delegate_b = Address::generate(&env);
    let delegate_c = Address::generate(&env);

    let expiry = 5000;

    // Build chain: owner → A → B → C
    client.delegate_access(&owner, &delegate_a, &expiry);
    client.delegate_access(&delegate_a, &delegate_b, &expiry);
    client.delegate_access(&delegate_b, &delegate_c, &expiry);

    // Ensure all have access
    assert!(client.has_access(&delegate_a));
    assert!(client.has_access(&delegate_b));
    assert!(client.has_access(&delegate_c));

    // Revoke top-level (A)
    client.revoke_delegation(&owner, &delegate_a);

    // EVERYTHING below should be revoked
    assert!(!client.has_access(&delegate_a));
    assert!(!client.has_access(&delegate_b));
    assert!(!client.has_access(&delegate_c));
}
