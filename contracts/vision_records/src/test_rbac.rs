use super::{Permission, Role, VisionRecordsContract, VisionRecordsContractClient};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, String};

fn setup_test() -> (Env, VisionRecordsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

#[test]
fn test_role_hierarchy_and_inheritance() {
    let (env, client, admin) = setup_test();

    let optometrist = Address::generate(&env);
    client.register_user(
        &admin,
        &optometrist,
        &Role::Optometrist,
        &String::from_str(&env, "Opto"),
    );

    let staff = Address::generate(&env);
    client.register_user(
        &admin,
        &staff,
        &Role::Staff,
        &String::from_str(&env, "Staff"),
    );

    let patient = Address::generate(&env);
    client.register_user(
        &admin,
        &patient,
        &Role::Patient,
        &String::from_str(&env, "Pat"),
    );

    // Admin should have all permissions implicitly
    assert!(client.check_permission(&admin, &Permission::SystemAdmin));
    assert!(client.check_permission(&admin, &Permission::ManageUsers));
    assert!(client.check_permission(&admin, &Permission::WriteRecord));

    // Optometrist should have read/write/access/users but NOT SystemAdmin
    assert!(!client.check_permission(&optometrist, &Permission::SystemAdmin));
    assert!(client.check_permission(&optometrist, &Permission::WriteRecord));
    assert!(client.check_permission(&optometrist, &Permission::ManageUsers));

    // Staff should have ManageUsers but NOT WriteRecord
    assert!(client.check_permission(&staff, &Permission::ManageUsers));
    assert!(!client.check_permission(&staff, &Permission::WriteRecord));

    // Patient has no implicit system permissions
    assert!(!client.check_permission(&patient, &Permission::ManageUsers));
    assert!(!client.check_permission(&patient, &Permission::WriteRecord));
}

#[test]
fn test_custom_permission_grants() {
    let (env, client, admin) = setup_test();

    let staff = Address::generate(&env);
    client.register_user(
        &admin,
        &staff,
        &Role::Staff,
        &String::from_str(&env, "Staff"),
    );

    // Staff originally cannot write records
    assert!(!client.check_permission(&staff, &Permission::WriteRecord));

    // Admin grants WriteRecord to staff
    client.grant_custom_permission(&admin, &staff, &Permission::WriteRecord);

    // Staff can now write records
    assert!(client.check_permission(&staff, &Permission::WriteRecord));

    // Admin revokes WriteRecord
    client.revoke_custom_permission(&admin, &staff, &Permission::WriteRecord);

    // Staff again cannot write records
    assert!(!client.check_permission(&staff, &Permission::WriteRecord));
}

#[test]
fn test_custom_permission_revocations() {
    let (env, client, admin) = setup_test();

    let optometrist = Address::generate(&env);
    client.register_user(
        &admin,
        &optometrist,
        &Role::Optometrist,
        &String::from_str(&env, "Opto"),
    );

    // Optometrist initially has ManageUsers
    assert!(client.check_permission(&optometrist, &Permission::ManageUsers));

    // Admin explicitly revokes ManageUsers from this specific Optometrist
    client.revoke_custom_permission(&admin, &optometrist, &Permission::ManageUsers);

    // They no longer have it, even though their base role does
    assert!(!client.check_permission(&optometrist, &Permission::ManageUsers));

    // But they still have WriteRecord
    assert!(client.check_permission(&optometrist, &Permission::WriteRecord));

    // Admin grants it back
    client.grant_custom_permission(&admin, &optometrist, &Permission::ManageUsers);
    assert!(client.check_permission(&optometrist, &Permission::ManageUsers));
}

#[test]
fn test_role_delegation() {
    let (env, client, admin) = setup_test();

    let pt1 = Address::generate(&env);
    let pt2 = Address::generate(&env);

    client.register_user(&admin, &pt1, &Role::Patient, &String::from_str(&env, "Pt1"));
    client.register_user(&admin, &pt2, &Role::Patient, &String::from_str(&env, "Pt2"));

    // pt1 delegates the Optometrist role (which has ManageAccess) to pt2 with an expiration.
    let future_time = env.ledger().timestamp() + 86400; // 1 day
    client.delegate_role(&pt1, &pt2, &Role::Optometrist, &future_time);

    // To test the delegation practically, pt2 tries to grant access to a doctor for pt1's records.
    let doctor = Address::generate(&env);
    client.register_user(
        &admin,
        &doctor,
        &Role::Optometrist,
        &String::from_str(&env, "Doc"),
    );

    // pt2 should be able to grant access acting for pt1
    // (caller: pt2, patient: pt1, grantee: doctor)
    client.grant_access(&pt2, &pt1, &doctor, &super::AccessLevel::Read, &3600);

    assert_eq!(client.check_access(&pt1, &doctor), super::AccessLevel::Read);
}

#[test]
fn test_role_delegation_expiration() {
    let (env, client, admin) = setup_test();

    let delegator = Address::generate(&env);
    let delegatee = Address::generate(&env);

    client.register_user(
        &admin,
        &delegator,
        &Role::Patient,
        &String::from_str(&env, "Delegator"),
    );
    client.register_user(
        &admin,
        &delegatee,
        &Role::Patient,
        &String::from_str(&env, "Delegatee"),
    );

    // Delegate role expiring immediately (timestamp 0 or already passed)
    // env.ledger().timestamp() is typically 0 at setup, we can advance it.
    env.ledger().set_timestamp(100);

    let expire_at = 50; // In the past
    client.delegate_role(&delegator, &delegatee, &Role::Patient, &expire_at);

    let doctor = Address::generate(&env);
    client.register_user(
        &admin,
        &doctor,
        &Role::Optometrist,
        &String::from_str(&env, "Doc"),
    );

    // Delegatee attempts to act for Delegator and should FAIL
    let result = client.try_grant_access(
        &delegatee,
        &delegator,
        &doctor,
        &super::AccessLevel::Read,
        &3600,
    );
    assert!(result.is_err());
}
