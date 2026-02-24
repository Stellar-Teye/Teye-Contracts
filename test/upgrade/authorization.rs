use your_crate_name::contract::UpgradeManager;
use your_crate_name::errors::ContractError;

#[test]
fn test_upgrade_authorized() {
    let manager = UpgradeManager {
        owner: "admin".to_string(),
    };

    assert!(manager.authorize_upgrade("admin").is_ok());
}

#[test]
fn test_upgrade_unauthorized() {
    let manager = UpgradeManager {
        owner: "admin".to_string(),
    };

    let result = manager.authorize_upgrade("hacker");

    assert_eq!(result, Err(ContractError::Unauthorized));
}
