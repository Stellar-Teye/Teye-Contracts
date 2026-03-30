#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::vec;

#[test]
fn test_initialize_zero_value_owner() {
    let env = Env::default();
    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    assert!(client.is_owner_active(&owner));
}

#[test]
#[should_panic] // You can add expected error message if known exactly
fn test_double_initialization_fails() {
    let env = Env::default();
    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);
    
    client.initialize(&owner);
}

#[test]
fn test_recovery_threshold_edge_cases() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    // 1. Zero threshold should fail
    let res_zero = client.try_prepare_set_recovery_threshold(&owner, &0);
    assert!(res_zero.is_err());

    // 2. Threshold higher than guardian count should fail
    let res_high = client.try_prepare_set_recovery_threshold(&owner, &1);
    assert!(res_high.is_err());

    // 3. Add a guardian and set threshold to 1
    let guardian = Address::generate(&env);
    client.add_guardian(&owner, &guardian);
    let res_valid = client.try_prepare_set_recovery_threshold(&owner, &1);
    assert!(res_valid.is_ok());
}

#[test]
fn test_empty_credential_bindings() {
    let env = Env::default();
    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    
    let bound = client.get_bound_credentials(&user);
    assert_eq!(bound.len(), 0);

    let mock_id = BytesN::from_array(&env, &[0u8; 32]);
    assert!(!client.is_credential_bound(&user, &mock_id));
}

#[test]
fn test_zk_verification_with_empty_proofs() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let empty_bytes = soroban_sdk::Bytes::new(&env);
    let resource_id = BytesN::from_array(&env, &[0u8; 32]);
    let public_inputs = vec![&env];

    let result = client.try_verify_zk_credential(
        &user,
        &resource_id,
        &empty_bytes,
        &empty_bytes,
        &empty_bytes,
        &public_inputs,
        &0,
    );

    assert!(result.is_err());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ink::test]
    fn test_upgrade_preserves_owner() {
        let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
        let mut contract = Identity::new(accounts.alice);

        contract.set_attribute("email".into(), "alice@example.com".into());
        contract.upgrade(2);

        assert_eq!(contract.owner, accounts.alice);
        assert_eq!(contract.get_attribute("email".into()), Some("alice@example.com".into()));
    }

    #[ink::test]
    fn test_upgrade_does_not_clear_attributes() {
        let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
        let mut contract = Identity::new(accounts.bob);

        contract.set_attribute("phone".into(), "123456".into());
        contract.upgrade(2);

        assert_eq!(contract.get_attribute("phone".into()), Some("123456".into()));
    }

    #[ink::test]
    fn test_upgrade_increments_version_only() {
        let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
        let mut contract = Identity::new(accounts.charlie);

        contract.upgrade(2);
        assert_eq!(contract.version, 2);
        assert_eq!(contract.owner, accounts.charlie);
    }
}
