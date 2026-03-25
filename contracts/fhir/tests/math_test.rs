use soroban_sdk::Env;

#[test]
fn test_safe_overflow() {
    let env = Env::default();
    env.mock_all_auths();

    let result = u64::MAX.checked_add(1);

    assert_eq!(result, None);
}
