#![allow(clippy::unwrap_used, clippy::expect_used, clippy::arithmetic_side_effects)]
mod common;

use common::{create_test_user, setup_test_env};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::Address;
use vision_records::{AccessLevel, Permission, Role};

#[test]
fn test_role_hierarchy_and_inheritance() {
    let ctx = setup_test_env();

    let optometrist = create_test_user(&ctx, Role::Optometrist, "Opto");
    let staff = create_test_user(&ctx, Role::Staff, "Staff");
    let patient = create_test_user(&ctx, Role::Patient, "Pat");

    assert!(ctx
        .client
        .check_permission(&ctx.admin, &Permission::SystemAdmin));
    assert!(ctx
        .client
        .check_permission(&ctx.admin, &Permission::ManageUsers));
    assert!(ctx
        .client
        .check_permission(&ctx.admin, &Permission::WriteRecord));

    assert!(!ctx
        .client
        .check_permission(&optometrist, &Permission::SystemAdmin));
    assert!(ctx
        .client
        .check_permission(&optometrist, &Permission::WriteRecord));
    assert!(ctx
        .client
        .check_permission(&optometrist, &Permission::ManageUsers));

    assert!(ctx
        .client
        .check_permission(&staff, &Permission::ManageUsers));
    assert!(!ctx
        .client
        .check_permission(&staff, &Permission::WriteRecord));

    assert!(!ctx
        .client
        .check_permission(&patient, &Permission::ManageUsers));
    assert!(!ctx
        .client
        .check_permission(&patient, &Permission::WriteRecord));
}

#[test]
fn test_custom_permission_grants() {
    let ctx = setup_test_env();

    let staff = create_test_user(&ctx, Role::Staff, "Staff");
    assert!(!ctx
        .client
        .check_permission(&staff, &Permission::WriteRecord));

    ctx.client
        .grant_custom_permission(&ctx.admin, &staff, &Permission::WriteRecord);
    assert!(ctx
        .client
        .check_permission(&staff, &Permission::WriteRecord));

    ctx.client
        .revoke_custom_permission(&ctx.admin, &staff, &Permission::WriteRecord);
    assert!(!ctx
        .client
        .check_permission(&staff, &Permission::WriteRecord));
}

#[test]
fn test_custom_permission_revocations() {
    let ctx = setup_test_env();

    let optometrist = create_test_user(&ctx, Role::Optometrist, "Opto");
    assert!(ctx
        .client
        .check_permission(&optometrist, &Permission::ManageUsers));

    ctx.client
        .revoke_custom_permission(&ctx.admin, &optometrist, &Permission::ManageUsers);
    assert!(!ctx
        .client
        .check_permission(&optometrist, &Permission::ManageUsers));

    assert!(ctx
        .client
        .check_permission(&optometrist, &Permission::WriteRecord));

    ctx.client
        .grant_custom_permission(&ctx.admin, &optometrist, &Permission::ManageUsers);
    ctx.client
        .grant_custom_permission(&ctx.admin, &optometrist, &Permission::SystemAdmin);

    assert!(ctx
        .client
        .check_permission(&optometrist, &Permission::ManageUsers));
    assert!(ctx
        .client
        .check_permission(&optometrist, &Permission::SystemAdmin));

    // Revoke ManageUsers and prove SystemAdmin remains (catches `!=` mutated to `==`)
    ctx.client
        .revoke_custom_permission(&ctx.admin, &optometrist, &Permission::ManageUsers);

    assert!(!ctx
        .client
        .check_permission(&optometrist, &Permission::ManageUsers));
    assert!(ctx
        .client
        .check_permission(&optometrist, &Permission::SystemAdmin));
}

#[test]
fn test_role_delegation() {
    let ctx = setup_test_env();

    let pt1 = create_test_user(&ctx, Role::Patient, "Pt1");
    let pt2 = create_test_user(&ctx, Role::Patient, "Pt2");
    let future_time = ctx.env.ledger().timestamp() + 86400;
    ctx.client
        .delegate_role(&pt1, &pt2, &Role::Optometrist, &future_time);

    let doctor = create_test_user(&ctx, Role::Optometrist, "Doc");
    ctx.client
        .grant_access(&pt2, &pt1, &doctor, &AccessLevel::Read, &3600);

    assert_eq!(ctx.client.check_access(&pt1, &doctor), AccessLevel::Read);
}

#[test]
fn test_role_delegation_expiration() {
    let ctx = setup_test_env();

    let delegator = create_test_user(&ctx, Role::Patient, "Delegator");
    let delegatee = create_test_user(&ctx, Role::Patient, "Delegatee");

    ctx.env.ledger().set_timestamp(100);
    let expire_at = 100;
    ctx.client
        .delegate_role(&delegator, &delegatee, &Role::Optometrist, &expire_at);

    let doctor = create_test_user(&ctx, Role::Optometrist, "Doc");
    let result =
        ctx.client
            .try_grant_access(&delegatee, &delegator, &doctor, &AccessLevel::Read, &3600);
    assert!(result.is_err()); // `>=` mutant killed here since exact == fails access

    ctx.env.ledger().set_timestamp(99);
    let result2 =
        ctx.client
            .try_grant_access(&delegatee, &delegator, &doctor, &AccessLevel::Read, &3600);
    assert!(result2.is_ok()); // `<` mutant killed here since strictly less than is allowed

    // Test infinite duration `expires_at == 0` bound
    ctx.client
        .delegate_role(&delegator, &delegatee, &Role::Optometrist, &0);

    // Jump forward in time 10 years to ensure it never expires
    ctx.env.ledger().set_timestamp(315360000);

    let result =
        ctx.client
            .try_grant_access(&delegatee, &delegator, &doctor, &AccessLevel::Read, &3600);
    assert!(result.is_ok());
}

#[test]
fn test_role_assignment_expiration() {
    let ctx = setup_test_env();

    let user = create_test_user(&ctx, Role::Patient, "User");

    ctx.env.ledger().set_timestamp(100);
    let expire_at = 100;
    ctx.env.as_contract(&ctx.client.address, || {
        vision_records::rbac::assign_role(&ctx.env, user.clone(), Role::Optometrist, expire_at);
    });

    // At timestamp 100, role is EXPIRED (must be strictly > 100)
    assert!(!ctx.client.check_permission(&user, &Permission::WriteRecord));

    // Rewind to timestamp 99, role is VALID
    ctx.env.ledger().set_timestamp(99);
    assert!(ctx.client.check_permission(&user, &Permission::WriteRecord));

    // Test infinite duration `expires_at == 0` bound
    ctx.env.as_contract(&ctx.client.address, || {
        vision_records::rbac::assign_role(&ctx.env, user.clone(), Role::Optometrist, 0);
    });

    // Jump forward in time 10 years to ensure it never expires
    ctx.env.ledger().set_timestamp(315360000);
    assert!(ctx.client.check_permission(&user, &Permission::WriteRecord));
}

#[test]
fn test_record_factory_creates_default_data() {
    let ctx = setup_test_env();
    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");

    let id = common::create_test_record(
        &ctx,
        &provider,
        &patient,
        &provider,
        vision_records::RecordType::Diagnosis,
        "e3b0c44298fc1c149afbf4c8996fb924",
    );
    let record = ctx.client.get_record(&id);
    assert_eq!(record.id, id);
    assert_eq!(record.patient, patient);
}

#[test]
fn test_user_factory_returns_unique_users() {
    let ctx = setup_test_env();
    let a = create_test_user(&ctx, Role::Staff, "UserA");
    let b = create_test_user(&ctx, Role::Staff, "UserB");
    assert_ne!(a, b);
}

#[test]
fn test_access_control_with_generated_addresses() {
    let ctx = setup_test_env();
    let patient = Address::generate(&ctx.env);
    let grantee = Address::generate(&ctx.env);
    assert_eq!(
        ctx.client.check_access(&patient, &grantee),
        AccessLevel::None
    );
}
