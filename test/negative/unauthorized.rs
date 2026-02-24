use your_crate_name::*;
use your_crate_name::errors::ContractError;

#[test]
fn test_unauthorized_access() {
    let contract = Contract::new("owner".to_string());

    let result = contract.restricted_action("attacker");

    assert_eq!(result, Err(ContractError::Unauthorized));
}
