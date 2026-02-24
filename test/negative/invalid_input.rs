use your_crate_name::*;
use your_crate_name::errors::ContractError;

#[test]
fn test_invalid_large_value() {
    let contract = Contract::new("owner".to_string());

    let result = contract.set_value(2_000_000);

    assert_eq!(result, Err(ContractError::InvalidInput));
}
