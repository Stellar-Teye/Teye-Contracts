use crate::{ContractError, KeyLevel};

pub fn validate_child_level(parent: KeyLevel, child: KeyLevel) -> Result<(), ContractError> {
    match (parent, child) {
        (KeyLevel::Master, KeyLevel::Contract)
        | (KeyLevel::Contract, KeyLevel::Operation)
        | (KeyLevel::Operation, KeyLevel::Session) => Ok(()),
        _ => Err(ContractError::InvalidHierarchy),
    }
}
