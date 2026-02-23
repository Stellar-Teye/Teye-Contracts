use your_crate_name::migrate::migrate_v1_to_v2;
use your_crate_name::state_v1::StateV1;

#[test]
fn test_state_migration_correctness() {
    let old_state = StateV1 {
        owner: "alice".to_string(),
        balance: 1000,
    };

    let new_state = migrate_v1_to_v2(old_state.clone());

    assert_eq!(new_state.owner, old_state.owner);
    assert_eq!(new_state.balance, old_state.balance);
    assert_eq!(new_state.is_frozen, false);
}
