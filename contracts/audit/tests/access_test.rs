#![allow(clippy::unwrap_used)]
use audit::contract::{AuditContract, AuditContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

#[test]
fn test_create_segment_unauthenticated_fails() {
    let env = Env::default();
    // Do NOT mock auths — only the admin's auth will be set up manually.
    let id = env.register(AuditContract, ());
    let client = AuditContractClient::new(&env, &id);
    let admin = Address::generate(&env);

    // Initialize with mock_all_auths so initialize itself passes.
    env.mock_all_auths();
    client.initialize(&admin);

    // Now clear auths so the next call has no authorization.
    env.set_auths(&[]);

    let segment = Symbol::short("UNAUTH");
    let result = client.try_create_segment(&segment);
    assert!(
        result.is_err(),
        "create_segment without admin auth must fail"
    );
}
