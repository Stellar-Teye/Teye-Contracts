//! Policy enforcement tests for KeyManagerContract.
//!
//! Covers issue #484: not_before / not_after time windows, max_uses cap,
//! revoked-key rejection, and non-owner / non-admin access denial.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use key_manager::{
    ContractError, KeyLevel, KeyManagerContract, KeyManagerContractClient, KeyPolicy, KeyStatus,
    KeyType,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, Vec,
};

fn setup() -> (Env, KeyManagerContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(KeyManagerContract, ());
    let client = KeyManagerContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let identity = Address::generate(&env);
    client.initialize(&admin, &identity);
    (env, client, admin)
}

fn unrestricted_policy(env: &Env) -> KeyPolicy {
    KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(env),
    }
}

fn make_key(
    env: &Env,
    client: &KeyManagerContractClient,
    admin: &Address,
    policy: KeyPolicy,
) -> BytesN<32> {
    let key_bytes = BytesN::from_array(env, &[1u8; 32]);
    client.create_master_key(admin, &KeyType::Signing, &policy, &0u64, &key_bytes)
}

// ─────────────────────────────────────────────────────────────────────────────
// not_before enforcement
// ─────────────────────────────────────────────────────────────────────────────

/// A key with `not_before` in the future must be rejected when used before
/// that timestamp.
#[test]
fn test_use_key_before_not_before_is_rejected() {
    let (env, client, admin) = setup();

    env.ledger().set_timestamp(1_000);

    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 2_000, // key not valid until t=2000
        not_after: 0,
        allowed_ops: Vec::new(&env),
    };
    let key_id = make_key(&env, &client, &admin, policy);

    // Current time (1000) < not_before (2000) — must be denied.
    let result = client.try_use_key(&admin, &key_id, &symbol_short!("SIGN"));
    assert_eq!(result, Err(Ok(ContractError::PolicyViolation)));
}

/// The same key must succeed once the ledger advances past `not_before`.
#[test]
fn test_use_key_after_not_before_is_allowed() {
    let (env, client, admin) = setup();

    env.ledger().set_timestamp(1_000);

    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 2_000,
        not_after: 0,
        allowed_ops: Vec::new(&env),
    };
    let key_id = make_key(&env, &client, &admin, policy);

    // Advance past not_before.
    env.ledger().set_timestamp(2_001);

    let result = client.try_use_key(&admin, &key_id, &symbol_short!("SIGN"));
    assert!(result.is_ok(), "use_key must succeed after not_before");
}

// ─────────────────────────────────────────────────────────────────────────────
// not_after enforcement
// ─────────────────────────────────────────────────────────────────────────────

/// A key with `not_after` in the past must be rejected.
#[test]
fn test_use_key_past_not_after_is_rejected() {
    let (env, client, admin) = setup();

    env.ledger().set_timestamp(500);

    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 1_000, // expires at t=1000
        allowed_ops: Vec::new(&env),
    };
    let key_id = make_key(&env, &client, &admin, policy);

    // Advance past not_after.
    env.ledger().set_timestamp(1_001);

    let result = client.try_use_key(&admin, &key_id, &symbol_short!("SIGN"));
    assert_eq!(result, Err(Ok(ContractError::PolicyViolation)));
}

/// Using a key exactly at `not_after` must still be allowed (boundary = open).
#[test]
fn test_use_key_at_not_after_boundary_is_allowed() {
    let (env, client, admin) = setup();

    env.ledger().set_timestamp(500);

    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 1_000,
        allowed_ops: Vec::new(&env),
    };
    let key_id = make_key(&env, &client, &admin, policy);

    env.ledger().set_timestamp(1_000); // exactly at not_after
    let result = client.try_use_key(&admin, &key_id, &symbol_short!("SIGN"));
    assert!(result.is_ok(), "use_key at not_after must still be allowed");
}

/// A key where not_after <= not_before must be rejected at creation time as
/// an invalid policy.
#[test]
fn test_create_key_with_inverted_time_window_rejected() {
    let (env, client, admin) = setup();

    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 5_000,
        not_after: 4_000, // not_after <= not_before — invalid
        allowed_ops: Vec::new(&env),
    };
    let key_bytes = BytesN::from_array(&env, &[2u8; 32]);
    let result =
        client.try_create_master_key(&admin, &KeyType::Signing, &policy, &0u64, &key_bytes);
    assert_eq!(result, Err(Ok(ContractError::InvalidPolicy)));
}

// ─────────────────────────────────────────────────────────────────────────────
// max_uses enforcement
// ─────────────────────────────────────────────────────────────────────────────

/// A key with `max_uses = 1` must reject any usage beyond the first.
#[test]
fn test_max_uses_one_allows_single_use_then_rejects() {
    let (env, client, admin) = setup();

    let policy = KeyPolicy {
        max_uses: 1,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(&env),
    };
    let key_id = make_key(&env, &client, &admin, policy);

    // First use must succeed.
    assert!(client
        .try_use_key(&admin, &key_id, &symbol_short!("SIGN"))
        .is_ok());

    // Second use must be denied.
    let result = client.try_use_key(&admin, &key_id, &symbol_short!("SIGN"));
    assert_eq!(result, Err(Ok(ContractError::PolicyViolation)));
}

/// Exactly max_uses are permitted; the (max_uses + 1)-th use is rejected.
#[test]
fn test_max_uses_boundary_at_exact_limit() {
    let (env, client, admin) = setup();

    let max = 5u32;
    let policy = KeyPolicy {
        max_uses: max,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(&env),
    };
    let key_id = make_key(&env, &client, &admin, policy);

    for _ in 0..max {
        assert!(client
            .try_use_key(&admin, &key_id, &symbol_short!("SIGN"))
            .is_ok());
    }

    // One more beyond the limit.
    let result = client.try_use_key(&admin, &key_id, &symbol_short!("SIGN"));
    assert_eq!(result, Err(Ok(ContractError::PolicyViolation)));
}

/// max_uses = 0 means unlimited; many calls must all succeed.
#[test]
fn test_max_uses_zero_means_unlimited() {
    let (env, client, admin) = setup();

    let policy = unrestricted_policy(&env);
    let key_id = make_key(&env, &client, &admin, policy);

    for _ in 0..20 {
        assert!(client
            .try_use_key(&admin, &key_id, &symbol_short!("SIGN"))
            .is_ok());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Revoked key rejection
// ─────────────────────────────────────────────────────────────────────────────

/// Using a revoked key must return KeyRevoked.
#[test]
fn test_use_revoked_key_returns_key_revoked() {
    let (env, client, admin) = setup();

    let key_id = make_key(&env, &client, &admin, unrestricted_policy(&env));
    client.revoke_key(&admin, &key_id);

    let result = client.try_use_key(&admin, &key_id, &symbol_short!("SIGN"));
    assert_eq!(result, Err(Ok(ContractError::KeyRevoked)));
}

/// Deriving a child key from a revoked parent must return KeyRevoked.
#[test]
fn test_derive_from_revoked_parent_returns_key_revoked() {
    let (env, client, admin) = setup();

    let parent_id = make_key(&env, &client, &admin, unrestricted_policy(&env));
    client.revoke_key(&admin, &parent_id);

    let result = client.try_derive_key(
        &admin,
        &parent_id,
        &KeyLevel::Contract,
        &0u32,
        &false,
        &KeyType::Signing,
        &unrestricted_policy(&env),
        &0u64,
    );
    assert_eq!(result, Err(Ok(ContractError::KeyRevoked)));
}

/// The stored key record must reflect KeyStatus::Revoked after revocation.
#[test]
fn test_revoked_key_record_shows_revoked_status() {
    let (env, client, admin) = setup();

    let key_id = make_key(&env, &client, &admin, unrestricted_policy(&env));
    client.revoke_key(&admin, &key_id);

    let record = client.get_key_record(&key_id).unwrap();
    assert_eq!(record.status, KeyStatus::Revoked);
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-owner / non-admin rejection
// ─────────────────────────────────────────────────────────────────────────────

/// A stranger (not the key owner, not the admin) must be denied on use_key.
#[test]
fn test_non_owner_non_admin_cannot_use_key() {
    let (env, client, admin) = setup();

    let key_id = make_key(&env, &client, &admin, unrestricted_policy(&env));

    let stranger = Address::generate(&env);
    let result = client.try_use_key(&stranger, &key_id, &symbol_short!("SIGN"));
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

/// A stranger must be denied on rotate_key.
#[test]
fn test_non_owner_non_admin_cannot_rotate_key() {
    let (env, client, admin) = setup();

    let key_id = make_key(&env, &client, &admin, unrestricted_policy(&env));

    let stranger = Address::generate(&env);
    let result = client.try_rotate_key(&stranger, &key_id);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

/// A stranger must be denied on revoke_key.
#[test]
fn test_non_owner_non_admin_cannot_revoke_key() {
    let (env, client, admin) = setup();

    let key_id = make_key(&env, &client, &admin, unrestricted_policy(&env));

    let stranger = Address::generate(&env);
    let result = client.try_revoke_key(&stranger, &key_id);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

/// A stranger must be denied on create_master_key (admin-only operation).
#[test]
fn test_non_admin_cannot_create_master_key() {
    let (env, client, _admin) = setup();

    let stranger = Address::generate(&env);
    let key_bytes = BytesN::from_array(&env, &[3u8; 32]);
    let result = client.try_create_master_key(
        &stranger,
        &KeyType::Signing,
        &unrestricted_policy(&env),
        &0u64,
        &key_bytes,
    );
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

// ─────────────────────────────────────────────────────────────────────────────
// allowed_ops enforcement
// ─────────────────────────────────────────────────────────────────────────────

/// When allowed_ops is non-empty, only listed operations must be accepted.
#[test]
fn test_allowed_ops_filters_disallowed_operation() {
    let (env, client, admin) = setup();

    let mut ops = Vec::new(&env);
    ops.push_back(symbol_short!("ENC"));

    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 0,
        allowed_ops: ops,
    };
    let key_id = make_key(&env, &client, &admin, policy);

    // Allowed operation.
    assert!(client
        .try_use_key(&admin, &key_id, &symbol_short!("ENC"))
        .is_ok());

    // Disallowed operation.
    let result = client.try_use_key(&admin, &key_id, &symbol_short!("SIGN"));
    assert_eq!(result, Err(Ok(ContractError::PolicyViolation)));
}

/// When allowed_ops is empty, any operation is permitted.
#[test]
fn test_empty_allowed_ops_permits_any_operation() {
    let (env, client, admin) = setup();

    let key_id = make_key(&env, &client, &admin, unrestricted_policy(&env));

    assert!(client
        .try_use_key(&admin, &key_id, &symbol_short!("SIGN"))
        .is_ok());
    assert!(client
        .try_use_key(&admin, &key_id, &symbol_short!("ENC"))
        .is_ok());
    assert!(client
        .try_use_key(&admin, &key_id, &symbol_short!("AUTH"))
        .is_ok());
}
