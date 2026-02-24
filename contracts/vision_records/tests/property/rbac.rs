#![allow(clippy::unwrap_used, clippy::expect_used, clippy::arithmetic_side_effects)]
//! Property-based tests for the RBAC module.
//!
//! Invariants tested:
//! - Admin role always holds all system permissions
//! - Patient role never holds WriteRecord, ManageUsers, or SystemAdmin
//! - Custom permission grants always add the specified permission
//! - Explicit revokes override base-role permissions
//! - Expired delegations are always denied

use proptest::prelude::*;
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Env, String};
use vision_records::{Permission, Role, VisionRecordsContract, VisionRecordsContractClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, VisionRecordsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

fn register(
    env: &Env,
    client: &VisionRecordsContractClient<'_>,
    caller: &Address,
    role: Role,
) -> Address {
    let user = Address::generate(env);
    client.register_user(caller, &user, &role, &String::from_str(env, "Test User"));
    user
}

// ── proptest! blocks ──────────────────────────────────────────────────────────

proptest! {
    /// Admin always holds all system-level permissions regardless of any other state.
    #[test]
    fn prop_admin_always_has_all_perms(_seed in 0u8..=255u8) {
        let (env, client, admin) = setup();
        // Admin is initialized but not registered via register_user —
        // the RBAC engine uses the stored ADMIN key to grant SystemAdmin implicitly.
        // After register_user as Admin, the role assignment is explicit.
        let admin_user = register(&env, &client, &admin, Role::Admin);

        prop_assert!(client.check_permission(&admin_user, &Permission::SystemAdmin));
        prop_assert!(client.check_permission(&admin_user, &Permission::ManageUsers));
        prop_assert!(client.check_permission(&admin_user, &Permission::WriteRecord));
        prop_assert!(client.check_permission(&admin_user, &Permission::ReadAnyRecord));
        prop_assert!(client.check_permission(&admin_user, &Permission::ManageAccess));
    }

    /// Patient role must never grant WriteRecord, ManageUsers, or SystemAdmin.
    #[test]
    fn prop_patient_has_no_system_perms(_seed in 0u8..=255u8) {
        let (env, client, admin) = setup();
        let patient = register(&env, &client, &admin, Role::Patient);

        prop_assert!(!client.check_permission(&patient, &Permission::WriteRecord));
        prop_assert!(!client.check_permission(&patient, &Permission::ManageUsers));
        prop_assert!(!client.check_permission(&patient, &Permission::SystemAdmin));
    }

    /// Staff has ManageUsers but NOT WriteRecord or SystemAdmin.
    #[test]
    fn prop_staff_permissions_correct(_seed in 0u8..=255u8) {
        let (env, client, admin) = setup();
        let staff = register(&env, &client, &admin, Role::Staff);

        prop_assert!(client.check_permission(&staff, &Permission::ManageUsers));
        prop_assert!(!client.check_permission(&staff, &Permission::WriteRecord));
        prop_assert!(!client.check_permission(&staff, &Permission::SystemAdmin));
    }

    /// Optometrist has WriteRecord, ReadAnyRecord, ManageAccess, ManageUsers but NOT SystemAdmin.
    #[test]
    fn prop_optometrist_permissions_correct(_seed in 0u8..=255u8) {
        let (env, client, admin) = setup();
        let opto = register(&env, &client, &admin, Role::Optometrist);

        prop_assert!(client.check_permission(&opto, &Permission::WriteRecord));
        prop_assert!(client.check_permission(&opto, &Permission::ReadAnyRecord));
        prop_assert!(client.check_permission(&opto, &Permission::ManageAccess));
        prop_assert!(client.check_permission(&opto, &Permission::ManageUsers));
        prop_assert!(!client.check_permission(&opto, &Permission::SystemAdmin));
    }

    /// A custom grant on a user must always result in that permission being present.
    #[test]
    fn prop_custom_grant_adds_permission(_seed in 0u8..=255u8) {
        let (env, client, admin) = setup();
        // Staff does NOT have WriteRecord by default
        let staff = register(&env, &client, &admin, Role::Staff);
        prop_assert!(!client.check_permission(&staff, &Permission::WriteRecord));

        client.grant_custom_permission(&admin, &staff, &Permission::WriteRecord);
        prop_assert!(client.check_permission(&staff, &Permission::WriteRecord));
    }

    /// An explicit revoke must override what the user's base role provides.
    #[test]
    fn prop_custom_revoke_overrides_base(_seed in 0u8..=255u8) {
        let (env, client, admin) = setup();
        // Optometrist HAS ManageUsers by base role
        let opto = register(&env, &client, &admin, Role::Optometrist);
        prop_assert!(client.check_permission(&opto, &Permission::ManageUsers));

        client.revoke_custom_permission(&admin, &opto, &Permission::ManageUsers);
        prop_assert!(!client.check_permission(&opto, &Permission::ManageUsers));

        // Other permissions from the base role must still be intact
        prop_assert!(client.check_permission(&opto, &Permission::WriteRecord));
    }

    /// A delegation whose `expires_at` is in the past must be denied when the
    /// delegatee tries to perform a delegated action.
    #[test]
    fn prop_expired_delegation_denied(_seed in 0u8..=255u8) {
        let (env, client, admin) = setup();

        let delegator = register(&env, &client, &admin, Role::Patient);
        let delegatee = register(&env, &client, &admin, Role::Patient);
        let doctor = register(&env, &client, &admin, Role::Optometrist);

        // Advance ledger timestamp so that the delegation has already expired
        env.ledger().set_timestamp(1000);
        let already_expired = 500u64; // in the past relative to current ledger time

        client.delegate_role(&delegator, &delegatee, &Role::Optometrist, &already_expired);

        // Delegatee tries to grant access on behalf of delegator — must fail
        let result = client.try_grant_access(
            &delegatee,
            &delegator,
            &doctor,
            &vision_records::AccessLevel::Read,
            &3600,
        );
        prop_assert!(result.is_err(), "Expired delegation should be denied");
    }

    /// A valid (non-expired) delegation must allow the delegatee to act on behalf
    /// of the delegator.
    #[test]
    fn prop_active_delegation_allowed(_seed in 0u8..=255u8) {
        let (env, client, admin) = setup();

        let delegator = register(&env, &client, &admin, Role::Patient);
        let delegatee = register(&env, &client, &admin, Role::Patient);
        let doctor = register(&env, &client, &admin, Role::Optometrist);

        env.ledger().set_timestamp(100);
        let future_expiry = 86_400u64; // far in the future

        // Delegate Optometrist role (has ManageAccess) from delegator → delegatee
        client.delegate_role(&delegator, &delegatee, &Role::Optometrist, &future_expiry);

        // delegatee grants access on behalf of delegator — must succeed
        client.grant_access(
            &delegatee,
            &delegator,
            &doctor,
            &vision_records::AccessLevel::Read,
            &3600,
        );

        prop_assert_eq!(
            client.check_access(&delegator, &doctor),
            vision_records::AccessLevel::Read
        );
    }
}
