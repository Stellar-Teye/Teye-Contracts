#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::Address as _, Address, Env};
use staking::{StakingContract, StakingContractClient};

/// Actions modelling all staking entry points plus admin operations.
///
/// Each variant carries the minimal data needed for execution. Values are
/// bounded to realistic ranges to avoid wasting fuzz cycles on trivially
/// rejected inputs.
#[derive(Arbitrary, Debug)]
pub enum FuzzAction {
    Stake { amount: u64 },
    Unstake { amount: u64 },
    ClaimRewards,
    SetRewardRate { new_rate: u32 },
    Pause,
    Unpause,
    AdvanceTime { delta: u16 },
}

fuzz_target!(|actions: Vec<FuzzAction>| {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let stake_tok = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let reward_tok = env.register_stellar_asset_contract_v2(Address::generate(&env));

    let contract_id = env.register(StakingContract, ());
    let client = StakingContractClient::new(&env, &contract_id);

    if client
        .try_initialize(&admin, &stake_tok.address(), &reward_tok.address(), &1000i128, &3600u64)
        .is_err()
    {
        return;
    }

    // Mint reward tokens to contract for claim operations.
    soroban_sdk::token::StellarAssetClient::new(&env, &reward_tok.address())
        .mint(&contract_id, &1_000_000_000i128);

    let mut users = vec![admin.clone()];
    for _ in 0..4 {
        let u = Address::generate(&env);
        // Mint stake tokens so Stake actions can succeed.
        soroban_sdk::token::StellarAssetClient::new(&env, &stake_tok.address())
            .mint(&u, &1_000_000_000i128);
        users.push(u);
    }

    // ── Invariant: total_staked must never go negative ──
    // We check this after every action.

    for (i, action) in actions.into_iter().enumerate() {
        let caller = &users[i % users.len()];
        match action {
            FuzzAction::Stake { amount } => {
                let amt = (amount as i128).max(1);
                let _ = client.try_stake(caller, &amt);
            }
            FuzzAction::Unstake { amount } => {
                let amt = (amount as i128).max(1);
                let _ = client.try_request_unstake(caller, &amt);
            }
            FuzzAction::ClaimRewards => {
                let _ = client.try_claim_rewards(caller);
            }
            FuzzAction::SetRewardRate { new_rate } => {
                let _ = client.try_set_reward_rate(&admin, &(new_rate as i128), &0u64);
            }
            FuzzAction::Pause => {
                let _ = client.try_pause(&admin);
            }
            FuzzAction::Unpause => {
                let _ = client.try_unpause(&admin);
            }
            FuzzAction::AdvanceTime { delta } => {
                let ts = env.ledger().timestamp().saturating_add(delta as u64);
                env.ledger().set_timestamp(ts);
            }
        }

        // ── Post-action invariant checks ──
        let total = client.get_total_staked();
        assert!(total >= 0, "INVARIANT VIOLATION: total_staked went negative: {}", total);

        // Individual stakes must not exceed total.
        for u in &users {
            let staked = client.get_staked(u);
            assert!(staked >= 0, "INVARIANT VIOLATION: user stake negative");
            assert!(staked <= total, "INVARIANT VIOLATION: user stake > total");
        }
    }
});
