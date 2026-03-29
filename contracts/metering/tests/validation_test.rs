#![allow(clippy::unwrap_used, clippy::expect_used)]

use metering::{
    billing::BillingModel, quota::TenantQuota, GasCosts, MeteringContract, MeteringContractClient,
    MeteringError, OperationType, TenantLevel,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup_uninitialized() -> (Env, MeteringContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MeteringContract, ());
    let client = MeteringContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    (env, client, admin)
}

fn setup_initialized() -> (Env, MeteringContractClient<'static>, Address, Address) {
    let (env, client, admin) = setup_uninitialized();
    let tenant = Address::generate(&env);

    client.initialize(&admin);
    client.register_tenant(&admin, &tenant, &TenantLevel::Organization, &tenant);

    (env, client, admin, tenant)
}

#[test]
fn minting_zero_tokens_reverts_without_mutating_balances() {
    let (_env, client, admin, tenant) = setup_initialized();

    let result = client.try_mint_gas_tokens(&admin, &tenant, &0u64);

    assert_eq!(result, Err(Ok(MeteringError::ZeroMintAmount)));
    assert_eq!(client.gas_token_balance(&tenant), 0);
    assert_eq!(client.total_gas_token_supply(), 0);
}

#[test]
fn zero_gas_costs_are_handled_as_a_no_op_charge() {
    let (_env, client, admin, tenant) = setup_initialized();
    let zero_costs = GasCosts {
        read_cost: 0,
        write_cost: 0,
        compute_cost: 0,
        storage_cost: 0,
    };

    client.set_gas_costs(&admin, &zero_costs);
    client.record_gas(&admin, &tenant, &OperationType::Read);

    assert_eq!(client.get_gas_costs(), zero_costs);
    let usage = client.get_usage(&tenant);
    assert_eq!(usage.read_used, 0);
    assert_eq!(usage.write_used, 0);
    assert_eq!(usage.compute_used, 0);
    assert_eq!(usage.storage_used, 0);
    assert_eq!(usage.burst_used, 0);
}

#[test]
fn enabled_zero_quota_reliably_blocks_the_first_metered_operation() {
    let (_env, client, admin, tenant) = setup_initialized();
    let zero_quota = TenantQuota {
        read_limit: 0,
        write_limit: 0,
        compute_limit: 0,
        storage_limit: 0,
        total_limit: 0,
        burst_allowance: 0,
        enabled: true,
    };

    client.set_quota(&admin, &tenant, &zero_quota);

    let result = client.try_record_gas(&admin, &tenant, &OperationType::Read);

    assert_eq!(result, Err(Ok(MeteringError::QuotaExceeded)));
    assert_eq!(client.get_usage(&tenant).read_used, 0);
}

#[test]
fn random_unregistered_addresses_revert_consistently_in_state_mutations() {
    let (env, client, admin, tenant) = setup_initialized();
    let missing_tenant = Address::generate(&env);
    let missing_parent = Address::generate(&env);
    let clinic = Address::generate(&env);

    let quota = TenantQuota {
        read_limit: 1,
        write_limit: 1,
        compute_limit: 1,
        storage_limit: 1,
        total_limit: 1,
        burst_allowance: 0,
        enabled: true,
    };

    assert_eq!(
        client.try_register_tenant(&admin, &clinic, &TenantLevel::Clinic, &missing_parent),
        Err(Ok(MeteringError::TenantNotFound))
    );
    assert_eq!(
        client.try_set_quota(&admin, &missing_tenant, &quota),
        Err(Ok(MeteringError::TenantNotFound))
    );
    assert_eq!(
        client.try_deactivate_tenant(&admin, &missing_tenant),
        Err(Ok(MeteringError::TenantNotFound))
    );
    assert_eq!(
        client.try_record_gas(&admin, &missing_tenant, &OperationType::Read),
        Err(Ok(MeteringError::TenantNotFound))
    );
    assert!(client.get_tenant(&tenant).active);
}

#[test]
fn billing_cycle_handles_empty_tenant_list_without_panicking() {
    let (_env, client, admin) = setup_uninitialized();

    client.initialize(&admin);
    let cycle_id = client.open_billing_cycle(&admin);
    let report = client.close_billing_cycle(&admin);

    assert_eq!(cycle_id, 1);
    assert_eq!(report.cycle_id, 1);
    assert_eq!(report.records.len(), 0);
    assert_eq!(client.current_cycle_id(), 1);
}

#[test]
fn zero_cost_prepaid_reads_do_not_consume_empty_balances() {
    let (_env, client, admin, tenant) = setup_initialized();
    let zero_costs = GasCosts {
        read_cost: 0,
        write_cost: 0,
        compute_cost: 0,
        storage_cost: 0,
    };

    client.set_billing_model(&admin, &tenant, &BillingModel::Prepaid);
    client.set_gas_costs(&admin, &zero_costs);
    client.record_gas(&admin, &tenant, &OperationType::Read);

    assert_eq!(client.gas_token_balance(&tenant), 0);
    assert_eq!(client.get_usage(&tenant).read_used, 0);
}
