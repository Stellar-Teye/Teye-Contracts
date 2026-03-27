#![allow(clippy::unwrap_used, clippy::expect_used)]

use metering::{
    billing::BillingModel, quota::TenantQuota, GasCosts, MeteringContract, MeteringContractClient,
    MeteringError, TenantLevel,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (
    Env,
    MeteringContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MeteringContract, ());
    let client = MeteringContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let outsider = Address::generate(&env);
    let tenant = Address::generate(&env);

    client.initialize(&admin);
    client.register_tenant(&admin, &tenant, &TenantLevel::Organization, &tenant);

    (env, client, admin, outsider, tenant)
}

fn sample_costs() -> GasCosts {
    GasCosts {
        read_cost: 2,
        write_cost: 7,
        compute_cost: 11,
        storage_cost: 4,
    }
}

fn sample_quota() -> TenantQuota {
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

#[test]
fn unauthorized_addresses_cannot_call_admin_configuration_functions() {
    let (env, client, _admin, outsider, tenant) = setup();
    let clinic = Address::generate(&env);

    assert_eq!(
        client.try_set_gas_costs(&outsider, &sample_costs()),
        Err(Ok(MeteringError::Unauthorized))
    );
    assert_eq!(
        client.try_register_tenant(&outsider, &clinic, &TenantLevel::Clinic, &tenant),
        Err(Ok(MeteringError::Unauthorized))
    );
    assert_eq!(
        client.try_deactivate_tenant(&outsider, &tenant),
        Err(Ok(MeteringError::Unauthorized))
    );
    assert_eq!(
        client.try_set_quota(&outsider, &tenant, &sample_quota()),
        Err(Ok(MeteringError::Unauthorized))
    );
    assert_eq!(
        client.try_remove_quota(&outsider, &tenant),
        Err(Ok(MeteringError::Unauthorized))
    );
    assert_eq!(
        client.try_set_billing_model(&outsider, &tenant, &BillingModel::Prepaid),
        Err(Ok(MeteringError::Unauthorized))
    );
}

#[test]
fn unauthorized_addresses_cannot_call_admin_billing_or_token_functions() {
    let (_env, client, _admin, outsider, tenant) = setup();

    assert_eq!(
        client.try_open_billing_cycle(&outsider),
        Err(Ok(MeteringError::Unauthorized))
    );
    assert_eq!(
        client.try_close_billing_cycle(&outsider),
        Err(Ok(MeteringError::Unauthorized))
    );
    assert_eq!(
        client.try_mint_gas_tokens(&outsider, &tenant, &100u64),
        Err(Ok(MeteringError::Unauthorized))
    );
    assert_eq!(
        client.try_freeze_gas_token_account(&outsider, &tenant),
        Err(Ok(MeteringError::Unauthorized))
    );
    assert_eq!(
        client.try_unfreeze_gas_token_account(&outsider, &tenant),
        Err(Ok(MeteringError::Unauthorized))
    );
}
