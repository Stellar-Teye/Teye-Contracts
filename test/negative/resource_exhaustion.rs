#[derive(Debug, PartialEq, Eq)]
enum ContractError {
    ResourceExhausted,
}

struct Contract {
    _owner: String,
}

impl Contract {
    fn new(owner: String) -> Self {
        Self { _owner: owner }
    }

    fn consume_resource(&self, units: u64) -> Result<(), ContractError> {
        if units > 10_000 {
            return Err(ContractError::ResourceExhausted);
        }
        Ok(())
    }
}

#[test]
fn test_resource_exhaustion() {
    let contract = Contract::new("owner".to_string());

    let result = contract.consume_resource(50_000);

    assert_eq!(result, Err(ContractError::ResourceExhausted));
}
