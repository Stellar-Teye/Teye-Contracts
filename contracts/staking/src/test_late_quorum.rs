#![cfg(test)]

use crate::{ContractError, StakingContract, StakingContractClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    token::StellarAssetClient,
    Address, BytesN, Env, Vec,
};

fn setup() -> (
    Env,
    StakingContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy two SAC tokens.
    let stake_token = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let reward_token = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let stake_token_id = stake_token.address();
    let reward_token_id = reward_token.address();

    let contract_id = env.register(StakingContract, ());
    let client = StakingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(
        &admin,
        &stake_token_id,
        &reward_token_id,
        &1000,  // reward_rate
        &86400, // lock_period
    );

    // Pre-fund the contract with reward tokens
    StellarAssetClient::new(&env, &reward_token_id)
        .mock_all_auths()
        .mint(&contract_id, &1_000_000_000i128);

    (env, client, admin, stake_token_id, reward_token_id)
}

#[test]
fn test_late_quorum_delay_flow() {
    let (env, client, admin, stake_token, _) = setup();

    // 1. Configure multisig (2-of-2)
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone(), s2.clone()]);
    client.configure_multisig(&admin, &signers, &2);

    // 2. Configure rate change delay (1 hour)
    client.set_rate_change_delay(&admin, &3600);
    assert_eq!(client.get_rate_change_delay(), 3600);

    // 3. Setup a staker to monitor rewards
    let staker = Address::generate(&env);
    StellarAssetClient::new(&env, &stake_token).mint(&staker, &10_000);

    env.ledger().set_timestamp(0);
    client.stake(&staker, &10_000);

    // 4. Propose rate change (increase to 2000)
    let action = symbol_short!("RWD_RATE");
    let data_hash = BytesN::from_array(&env, &[1u8; 32]);
    let proposal_id = client.propose_admin_action(&s1, &action, &data_hash);

    // 5. Reach quorum (s2 approves)
    env.ledger().set_timestamp(100);
    client.approve_admin_action(&s2, &proposal_id);

    // 6. Execute set_reward_rate (reached late quorum)
    client.set_reward_rate(&admin, &2000, &proposal_id);

    // 7. Verify rate is still 1000 (delay active)
    assert_eq!(client.get_reward_rate(), 1000);
    let proposal = client.get_pending_rate_change();
    assert_eq!(proposal.new_rate, 2000);
    assert_eq!(proposal.effective_at, 100 + 3600);

    // 8. Wait for delay to pass
    env.ledger().set_timestamp(3701);

    // Rewards should still be calculated at old rate (1000) for the interval [0, 3701]
    // 1000 * 3701 = 3,701,000
    assert_eq!(client.get_pending_rewards(&staker), 3_701_000);

    // 9. Apply the rate change
    client.apply_reward_rate(&admin);
    assert_eq!(client.get_reward_rate(), 2000);

    // 10. Verify rewards start accruing at new rate
    env.ledger().set_timestamp(3801); // +100 seconds
                                      // 3,701,000 + (2000 * 100) = 3,701,000 + 200,000 = 3,901,000
    assert_eq!(client.get_pending_rewards(&staker), 3_901_000);
}

#[test]
fn test_apply_too_early_fails() {
    let (env, client, admin, _, _) = setup();

    // Configure delay
    client.set_rate_change_delay(&admin, &3600);

    // Single admin path for simplicity (multisig integration already tested above)
    env.ledger().set_timestamp(100);
    client.set_reward_rate(&admin, &2000, &0);

    // Attempt to apply at 3699 (threshold is 3700)
    env.ledger().set_timestamp(3699);
    let res = client.try_apply_reward_rate(&admin);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::RateChangeNotReady);
}
