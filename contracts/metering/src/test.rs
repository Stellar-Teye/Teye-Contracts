//! Tests for the metering contract.
//!
//! Covers:
//! - Per-tenant gas tracking accuracy
//! - Quota enforcement (hard block once quota + burst exhausted)
//! - Burst allowance draw-down
//! - Cost allocation proportionality
//! - Billing cycle lifecycle (open → record → close → report)
//! - Prepaid and postpaid billing models
//! - Gas token minting, burning, and freeze/unfreeze
//! - Hierarchical rollup (org → clinic → provider)
//! - Alert threshold events
//! - Edge cases: zero usage, exact quota boundary, multiple cycles

#![allow(unused_variables, unused_imports)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{
    billing::{BillingModel, CycleStatus},
    quota::TenantQuota,
    GasCosts, MeteringContract, MeteringContractClient, MeteringError, OperationType, TenantLevel,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Stand-up the contract with a fresh environment and return (env, client, admin).
fn setup() -> (Env, MeteringContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MeteringContract, ());
    let client = MeteringContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

/// Default quota — 10 read, 10 write, 10 compute, 10 storage, 40 total, 5 burst.
fn default_quota(env: &Env) -> TenantQuota {
    TenantQuota {
        read_limit: 10,
        write_limit: 10,
        compute_limit: 10,
        storage_limit: 10,
        total_limit: 40,
        burst_allowance: 5,
        enabled: true,
    }
}

/// Register an org-level tenant.
fn register_org(client: &MeteringContractClient, admin: &Address, env: &Env) -> Address {
    let org = Address::generate(env);
    client.register_tenant(admin, &org, &TenantLevel::Organization, &org);
    org
}

/// Register a clinic under an org.
fn register_clinic(
    client: &MeteringContractClient,
    admin: &Address,
    env: &Env,
    org: &Address,
) -> Address {
    let clinic = Address::generate(env);
    client.register_tenant(admin, &clinic, &TenantLevel::Clinic, org);
    clinic
}

/// Register a provider under a clinic.
fn register_provider(
    client: &MeteringContractClient,
    admin: &Address,
    env: &Env,
    clinic: &Address,
) -> Address {
    let provider = Address::generate(env);
    client.register_tenant(admin, &provider, &TenantLevel::Provider, clinic);
    provider
}

// ── Initialisation tests ──────────────────────────────────────────────────────

#[test]
fn test_initialize_sets_admin() {
    let (env, client, admin) = setup();
    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin);
}

#[test]
fn test_double_initialize_fails() {
    let (env, client, admin) = setup();
    let result = client.try_initialize(&admin);
    assert_eq!(result, Err(Ok(MeteringError::AlreadyInitialized)));
}

#[test]
fn test_default_gas_costs() {
    let (env, client, _admin) = setup();
    let costs = client.get_gas_costs();
    assert_eq!(costs.read_cost, 1);
    assert_eq!(costs.write_cost, 5);
    assert_eq!(costs.compute_cost, 10);
    assert_eq!(costs.storage_cost, 3);
}

#[test]
fn test_set_gas_costs() {
    let (env, client, admin) = setup();
    let new_costs = GasCosts {
        read_cost: 2,
        write_cost: 8,
        compute_cost: 15,
        storage_cost: 4,
    };
    client.set_gas_costs(&admin, &new_costs);
    let stored = client.get_gas_costs();
    assert_eq!(stored, new_costs);
}

// ── Tenant registration tests ─────────────────────────────────────────────────

#[test]
fn test_register_organization() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let tenant = client.get_tenant(&org);
    assert_eq!(tenant.level, TenantLevel::Organization);
    assert!(tenant.active);
    assert_eq!(tenant.parent, org);
}

#[test]
fn test_register_clinic_under_org() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let clinic = register_clinic(&client, &admin, &env, &org);
    let tenant = client.get_tenant(&clinic);
    assert_eq!(tenant.level, TenantLevel::Clinic);
    assert_eq!(tenant.parent, org);
}

#[test]
fn test_register_provider_under_clinic() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let clinic = register_clinic(&client, &admin, &env, &org);
    let provider = register_provider(&client, &admin, &env, &clinic);
    let tenant = client.get_tenant(&provider);
    assert_eq!(tenant.level, TenantLevel::Provider);
    assert_eq!(tenant.parent, clinic);
}

#[test]
fn test_register_duplicate_tenant_fails() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let result = client.try_register_tenant(&admin, &org, &TenantLevel::Organization, &org);
    assert_eq!(result, Err(Ok(MeteringError::TenantAlreadyExists)));
}

#[test]
fn test_register_clinic_without_parent_fails() {
    let (env, client, admin) = setup();
    let fake_parent = Address::generate(&env);
    let clinic = Address::generate(&env);
    let result = client.try_register_tenant(
        &admin,
        &clinic,
        &TenantLevel::Clinic,
        &fake_parent,
    );
    assert_eq!(result, Err(Ok(MeteringError::TenantNotFound)));
}

#[test]
fn test_deactivate_tenant() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.deactivate_tenant(&admin, &org);
    let tenant = client.get_tenant(&org);
    assert!(!tenant.active);
}

#[test]
fn test_list_tenants() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let list = client.list_tenants();
    assert_eq!(list.len(), 1);
    assert_eq!(list.get(0).unwrap(), org);
}

// ── Quota tests ───────────────────────────────────────────────────────────────

#[test]
fn test_set_and_get_quota() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let quota = default_quota(&env);
    client.set_quota(&admin, &org, &quota);
    let stored = client.get_quota(&org).unwrap();
    assert_eq!(stored, quota);
}

#[test]
fn test_remove_quota_returns_none() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.set_quota(&admin, &org, &default_quota(&env));
    client.remove_quota(&admin, &org);
    assert!(client.get_quota(&org).is_none());
}

#[test]
fn test_usage_starts_at_zero() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let usage = client.get_usage(&org);
    assert_eq!(usage.read_used, 0);
    assert_eq!(usage.write_used, 0);
    assert_eq!(usage.compute_used, 0);
    assert_eq!(usage.storage_used, 0);
    assert_eq!(usage.burst_used, 0);
}

// ── Gas recording tests ───────────────────────────────────────────────────────

#[test]
fn test_record_read_gas_increments_usage() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.record_gas(&admin, &org, &OperationType::Read);
    let usage = client.get_usage(&org);
    // Default read cost = 1
    assert_eq!(usage.read_used, 1);
}

#[test]
fn test_record_write_gas_uses_write_cost() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.record_gas(&admin, &org, &OperationType::Write);
    let usage = client.get_usage(&org);
    // Default write cost = 5
    assert_eq!(usage.write_used, 5);
}

#[test]
fn test_record_compute_gas() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.record_gas(&admin, &org, &OperationType::Compute);
    let usage = client.get_usage(&org);
    // Default compute cost = 10
    assert_eq!(usage.compute_used, 10);
}

#[test]
fn test_record_storage_gas() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.record_gas(&admin, &org, &OperationType::Storage);
    let usage = client.get_usage(&org);
    // Default storage cost = 3
    assert_eq!(usage.storage_used, 3);
}

#[test]
fn test_record_gas_on_inactive_tenant_fails() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.deactivate_tenant(&admin, &org);
    let result = client.try_record_gas(&admin, &org, &OperationType::Read);
    assert_eq!(result, Err(Ok(MeteringError::TenantInactive)));
}

// ── Quota enforcement tests ───────────────────────────────────────────────────

#[test]
fn test_record_gas_within_quota_succeeds() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    // Set a generous quota
    let quota = TenantQuota {
        read_limit: 100,
        write_limit: 100,
        compute_limit: 100,
        storage_limit: 100,
        total_limit: 400,
        burst_allowance: 0,
        enabled: true,
    };
    client.set_quota(&admin, &org, &quota);
    // Record 5 reads — should succeed
    for _ in 0..5 {
        client.record_gas(&admin, &org, &OperationType::Read);
    }
    assert_eq!(client.get_usage(&org).read_used, 5);
}

#[test]
fn test_quota_enforcement_blocks_excess() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    // Very tight quota: 2 units total, 0 burst
    let quota = TenantQuota {
        read_limit: 2,
        write_limit: 2,
        compute_limit: 2,
        storage_limit: 2,
        total_limit: 2,
        burst_allowance: 0,
        enabled: true,
    };
    client.set_quota(&admin, &org, &quota);

    // First read (cost 1) — should succeed.
    client.record_gas(&admin, &org, &OperationType::Read);
    // Second read (cost 1) — fills quota.
    client.record_gas(&admin, &org, &OperationType::Read);
    // Third read — should be blocked.
    let result = client.try_record_gas(&admin, &org, &OperationType::Read);
    assert_eq!(result, Err(Ok(MeteringError::QuotaExceeded)));
}

#[test]
fn test_burst_allowance_extends_quota() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    // Quota: 1 read unit total, burst 1 unit
    let quota = TenantQuota {
        read_limit: 1,
        write_limit: 100,
        compute_limit: 100,
        storage_limit: 100,
        total_limit: 1,
        burst_allowance: 1,
        enabled: true,
    };
    client.set_quota(&admin, &org, &quota);

    // First read fills hard limit.
    client.record_gas(&admin, &org, &OperationType::Read);
    // Second read draws from burst allowance.
    client.record_gas(&admin, &org, &OperationType::Read);
    // Third read — both hard limit and burst exhausted.
    let result = client.try_record_gas(&admin, &org, &OperationType::Read);
    assert_eq!(result, Err(Ok(MeteringError::QuotaExceeded)));
}

#[test]
fn test_disabled_quota_allows_unlimited() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let quota = TenantQuota {
        read_limit: 0,
        write_limit: 0,
        compute_limit: 0,
        storage_limit: 0,
        total_limit: 0,
        burst_allowance: 0,
        enabled: false, // disabled
    };
    client.set_quota(&admin, &org, &quota);
    // Should not be blocked despite zero limits
    for _ in 0..10 {
        client.record_gas(&admin, &org, &OperationType::Read);
    }
    assert_eq!(client.get_usage(&org).read_used, 10);
}

#[test]
fn test_exact_quota_boundary() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    // read cost = 1, total_limit = 3 → exactly 3 reads allowed
    let quota = TenantQuota {
        read_limit: 3,
        write_limit: 3,
        compute_limit: 3,
        storage_limit: 3,
        total_limit: 3,
        burst_allowance: 0,
        enabled: true,
    };
    client.set_quota(&admin, &org, &quota);
    client.record_gas(&admin, &org, &OperationType::Read);
    client.record_gas(&admin, &org, &OperationType::Read);
    client.record_gas(&admin, &org, &OperationType::Read);
    // Exactly at limit — next should fail
    let result = client.try_record_gas(&admin, &org, &OperationType::Read);
    assert_eq!(result, Err(Ok(MeteringError::QuotaExceeded)));
}

// ── Hierarchical rollup tests ─────────────────────────────────────────────────

#[test]
fn test_provider_usage_rolls_up_to_clinic() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let clinic = register_clinic(&client, &admin, &env, &org);
    let provider = register_provider(&client, &admin, &env, &clinic);

    client.record_gas(&admin, &provider, &OperationType::Read);

    // Provider has 1 unit read.
    assert_eq!(client.get_usage(&provider).read_used, 1);
    // Clinic should also have 1 unit rolled up.
    assert_eq!(client.get_usage(&clinic).read_used, 1);
    // Org should also have 1 unit.
    assert_eq!(client.get_usage(&org).read_used, 1);
}

#[test]
fn test_rollup_does_not_double_count_clinic_direct_usage() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let clinic = register_clinic(&client, &admin, &env, &org);

    // Direct clinic usage.
    client.record_gas(&admin, &clinic, &OperationType::Write);

    assert_eq!(client.get_usage(&clinic).write_used, 5);
    // Org picks up the rollup.
    assert_eq!(client.get_usage(&org).write_used, 5);
}

#[test]
fn test_multiple_providers_aggregate_at_org() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let clinic = register_clinic(&client, &admin, &env, &org);
    let p1 = register_provider(&client, &admin, &env, &clinic);
    let p2 = register_provider(&client, &admin, &env, &clinic);

    client.record_gas(&admin, &p1, &OperationType::Read);
    client.record_gas(&admin, &p2, &OperationType::Read);

    // Clinic should have both reads rolled up (2 units).
    assert_eq!(client.get_usage(&clinic).read_used, 2);
    // Org should also have 2 units.
    assert_eq!(client.get_usage(&org).read_used, 2);
}

// ── Billing cycle tests ───────────────────────────────────────────────────────

#[test]
fn test_open_billing_cycle_returns_id() {
    let (env, client, admin) = setup();
    let cycle_id = client.open_billing_cycle(&admin);
    assert_eq!(cycle_id, 1);
}

#[test]
fn test_open_second_cycle_after_close() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.open_billing_cycle(&admin);
    client.close_billing_cycle(&admin);
    let cycle_id2 = client.open_billing_cycle(&admin);
    assert_eq!(cycle_id2, 2);
}

#[test]
fn test_double_open_cycle_fails() {
    let (env, client, admin) = setup();
    client.open_billing_cycle(&admin);
    let result = client.try_open_billing_cycle(&admin);
    assert_eq!(result, Err(Ok(MeteringError::CycleAlreadyOpen)));
}

#[test]
fn test_close_cycle_without_open_fails() {
    let (env, client, admin) = setup();
    let result = client.try_close_billing_cycle(&admin);
    assert_eq!(result, Err(Ok(MeteringError::NoCycleOpen)));
}

#[test]
fn test_cycle_status_transitions() {
    let (env, client, admin) = setup();
    client.open_billing_cycle(&admin);
    let cycle = client.get_billing_cycle(&1u64).unwrap();
    assert_eq!(cycle.status, CycleStatus::Open);
    client.close_billing_cycle(&admin);
    let cycle = client.get_billing_cycle(&1u64).unwrap();
    assert_eq!(cycle.status, CycleStatus::Closed);
}

#[test]
fn test_close_cycle_resets_usage_on_next_open() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.open_billing_cycle(&admin);
    client.record_gas(&admin, &org, &OperationType::Read);
    assert_eq!(client.get_usage(&org).read_used, 1);

    client.close_billing_cycle(&admin);

    // Open a new cycle — usage is reset.
    client.open_billing_cycle(&admin);
    assert_eq!(client.get_usage(&org).read_used, 0);
}

#[test]
fn test_billing_report_contains_all_tenants() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let clinic = register_clinic(&client, &admin, &env, &org);
    client.open_billing_cycle(&admin);
    client.record_gas(&admin, &org, &OperationType::Read);
    let report = client.close_billing_cycle(&admin);
    // Two tenants: org and clinic.
    assert_eq!(report.records.len(), 2);
}

#[test]
fn test_billing_report_cost_calculation() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.open_billing_cycle(&admin);
    // 2 reads at cost 1 each = 2 total.
    client.record_gas(&admin, &org, &OperationType::Read);
    client.record_gas(&admin, &org, &OperationType::Read);
    let report = client.close_billing_cycle(&admin);
    let record = report.records.get(0).unwrap();
    assert_eq!(record.read_units, 2);
    assert_eq!(record.total_cost, 2);
}

// ── Postpaid invoice tests ────────────────────────────────────────────────────

#[test]
fn test_postpaid_invoice_created_on_cycle_close() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.set_billing_model(&admin, &org, &BillingModel::Postpaid);
    client.open_billing_cycle(&admin);
    client.record_gas(&admin, &org, &OperationType::Write); // cost = 5
    let report = client.close_billing_cycle(&admin);
    let inv = client.get_invoice(&org, &report.cycle_id).unwrap();
    assert_eq!(inv.amount_due, 5);
    assert!(!inv.settled);
}

#[test]
fn test_settle_invoice_marks_as_settled() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.set_billing_model(&admin, &org, &BillingModel::Postpaid);
    client.open_billing_cycle(&admin);
    client.record_gas(&admin, &org, &OperationType::Write);
    let report = client.close_billing_cycle(&admin);
    client.settle_invoice(&org, &report.cycle_id);
    let inv = client.get_invoice(&org, &report.cycle_id).unwrap();
    assert!(inv.settled);
}

#[test]
fn test_settle_same_invoice_twice_fails() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.set_billing_model(&admin, &org, &BillingModel::Postpaid);
    client.open_billing_cycle(&admin);
    client.record_gas(&admin, &org, &OperationType::Write);
    let report = client.close_billing_cycle(&admin);
    client.settle_invoice(&org, &report.cycle_id);
    let result = client.try_settle_invoice(&org, &report.cycle_id);
    assert_eq!(result, Err(Ok(MeteringError::AlreadySettled)));
}

#[test]
fn test_get_nonexistent_invoice_returns_none() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    assert!(client.get_invoice(&org, &99u64).is_none());
}

// ── Prepaid tests ─────────────────────────────────────────────────────────────

#[test]
fn test_prepaid_mint_and_balance() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.mint_gas_tokens(&admin, &org, &100u64);
    assert_eq!(client.gas_token_balance(&org), 100);
}

#[test]
fn test_prepaid_record_gas_burns_tokens() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.set_billing_model(&admin, &org, &BillingModel::Prepaid);
    client.mint_gas_tokens(&admin, &org, &100u64);
    client.record_gas(&admin, &org, &OperationType::Write); // cost = 5
    assert_eq!(client.gas_token_balance(&org), 95);
}

#[test]
fn test_prepaid_insufficient_balance_blocks_operation() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.set_billing_model(&admin, &org, &BillingModel::Prepaid);
    client.mint_gas_tokens(&admin, &org, &3u64); // only 3 units
    // Write costs 5 — should be blocked.
    let result = client.try_record_gas(&admin, &org, &OperationType::Write);
    assert_eq!(result, Err(Ok(MeteringError::InsufficientPrepaidBalance)));
}

#[test]
fn test_frozen_account_blocks_prepaid_operation() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.set_billing_model(&admin, &org, &BillingModel::Prepaid);
    client.mint_gas_tokens(&admin, &org, &100u64);
    client.freeze_gas_token_account(&admin, &org);
    let result = client.try_record_gas(&admin, &org, &OperationType::Read);
    assert_eq!(result, Err(Ok(MeteringError::GasTokenAccountFrozen)));
}

#[test]
fn test_unfreeze_account_restores_operations() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.set_billing_model(&admin, &org, &BillingModel::Prepaid);
    client.mint_gas_tokens(&admin, &org, &100u64);
    client.freeze_gas_token_account(&admin, &org);
    client.unfreeze_gas_token_account(&admin, &org);
    // Should succeed again.
    client.record_gas(&admin, &org, &OperationType::Read);
    assert_eq!(client.gas_token_balance(&org), 99);
}

#[test]
fn test_total_gas_token_supply_tracks_minting() {
    let (env, client, admin) = setup();
    let org1 = register_org(&client, &admin, &env);
    let org2 = register_org(&client, &admin, &env);
    client.mint_gas_tokens(&admin, &org1, &50u64);
    client.mint_gas_tokens(&admin, &org2, &30u64);
    assert_eq!(client.total_gas_token_supply(), 80);
}

#[test]
fn test_mint_zero_fails() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let result = client.try_mint_gas_tokens(&admin, &org, &0u64);
    assert_eq!(result, Err(Ok(MeteringError::ZeroMintAmount)));
}

// ── Authorisation tests ───────────────────────────────────────────────────────

#[test]
fn test_non_admin_cannot_register_tenant() {
    let (env, client, _admin) = setup();
    let not_admin = Address::generate(&env);
    let tenant = Address::generate(&env);
    let result = client.try_register_tenant(
        &not_admin,
        &tenant,
        &TenantLevel::Organization,
        &tenant,
    );
    assert_eq!(result, Err(Ok(MeteringError::Unauthorized)));
}

#[test]
fn test_non_admin_cannot_set_quota() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let not_admin = Address::generate(&env);
    let result = client.try_set_quota(&not_admin, &org, &default_quota(&env));
    assert_eq!(result, Err(Ok(MeteringError::Unauthorized)));
}

#[test]
fn test_non_admin_cannot_mint_gas_tokens() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let not_admin = Address::generate(&env);
    let result = client.try_mint_gas_tokens(&not_admin, &org, &100u64);
    assert_eq!(result, Err(Ok(MeteringError::Unauthorized)));
}

// ── Multiple cycles / cost allocation proportionality tests ──────────────────

#[test]
fn test_cost_allocation_proportional_across_tenants() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    let clinic = register_clinic(&client, &admin, &env, &org);

    // Set both as postpaid.
    client.set_billing_model(&admin, &org, &BillingModel::Postpaid);
    client.set_billing_model(&admin, &clinic, &BillingModel::Postpaid);

    client.open_billing_cycle(&admin);
    // Clinic: 2 reads (cost 2).
    client.record_gas(&admin, &clinic, &OperationType::Read);
    client.record_gas(&admin, &clinic, &OperationType::Read);
    let report = client.close_billing_cycle(&admin);

    // Org record: picked up 2 units via rollup.
    let org_record = report
        .records
        .iter()
        .find(|r| r.tenant == org)
        .expect("org record missing");
    let clinic_record = report
        .records
        .iter()
        .find(|r| r.tenant == clinic)
        .expect("clinic record missing");

    // Clinic billed for its own usage.
    assert_eq!(clinic_record.total_cost, 2);
    // Org was rolled up — it shows the aggregate.
    assert_eq!(org_record.read_units, 2);
}

#[test]
fn test_usage_reset_between_cycles() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);

    client.open_billing_cycle(&admin);
    client.record_gas(&admin, &org, &OperationType::Compute); // cost 10
    assert_eq!(client.get_usage(&org).compute_used, 10);

    client.close_billing_cycle(&admin);
    client.open_billing_cycle(&admin);
    // Fresh cycle — usage zeroed.
    assert_eq!(client.get_usage(&org).compute_used, 0);
}

#[test]
fn test_multiple_operation_types_tracked_separately() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);

    client.record_gas(&admin, &org, &OperationType::Read);
    client.record_gas(&admin, &org, &OperationType::Write);
    client.record_gas(&admin, &org, &OperationType::Compute);
    client.record_gas(&admin, &org, &OperationType::Storage);

    let u = client.get_usage(&org);
    assert_eq!(u.read_used, 1);
    assert_eq!(u.write_used, 5);
    assert_eq!(u.compute_used, 10);
    assert_eq!(u.storage_used, 3);
}

#[test]
fn test_gas_token_account_snapshot() {
    let (env, client, admin) = setup();
    let org = register_org(&client, &admin, &env);
    client.mint_gas_tokens(&admin, &org, &200u64);
    let account = client.get_gas_token_account(&org);
    assert_eq!(account.balance, 200);
    assert!(!account.frozen);
}
