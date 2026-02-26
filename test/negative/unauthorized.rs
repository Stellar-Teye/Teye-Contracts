#[derive(Debug, PartialEq, Eq)]
enum ContractError {
    Unauthorized,
}

struct Contract {
    owner: String,
}

impl Contract {
    fn new(owner: String) -> Self {
        Self { owner }
    }

    fn restricted_action(&self, caller: &str) -> Result<(), ContractError> {
        if caller != self.owner {
            return Err(ContractError::Unauthorized);
        }
        Ok(())
    }
}

#[test]
fn test_unauthorized_access() {
    let contract = Contract::new("owner".to_string());

    let result = contract.restricted_action("attacker");

    assert_eq!(result, Err(ContractError::Unauthorized));
}
