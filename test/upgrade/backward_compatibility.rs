use your_crate_name::state_v1::StateV1;
use your_crate_name::state_v2::StateV2;

#[test]
fn test_backward_compatibility_deserialization() {
    let old_state = StateV1 {
        owner: "bob".to_string(),
        balance: 500,
    };

    let serialized = serde_json::to_string(&old_state).unwrap();

    // Simulate loading old state into new contract version
    let migrated: StateV2 =
        serde_json::from_str(&serialized).unwrap_or_else(|_| {
            // fallback migration
            your_crate_name::migrate::migrate_v1_to_v2(old_state.clone())
        });

    assert_eq!(migrated.owner, "bob");
    assert_eq!(migrated.balance, 500);
}
