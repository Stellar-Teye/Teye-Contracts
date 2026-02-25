#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::Address as _, Address, Env};
use staking::{StakingContract, StakingContractClient};

#[derive(Arbitrary, Debug)]
pub enum FuzzAction {
    Stake { amount: u64 },
    Unstake { amount: u64 },
    ClaimRewards,
}

fuzz_target!(|actions: Vec<FuzzAction>| {
    let env = Env::default();
    let admin = Address::generate(&env);

    // For fuzzing, we ignore the external token dependencies and focus on logical behavior and panics
    // by using a mocked token or ignoring them if they require a real setup.
    // If the contract requires complex initialization to work, we mock it.
    let contract_id = env.register(StakingContract, ());
    let client = StakingContractClient::new(&env, &contract_id);

    // Note: Fuzz targets can only discover logic panics, not full correctness
    // unless assertions are defined.
    // Here we're mostly looking for out of bounds bounds/integer overflow panics.
    let _ = client.try_initialize(&admin, &admin, &admin, &1000i128, &3600u64);

    let mut users = vec![admin.clone()];
    for _ in 0..5 {
        users.push(Address::generate(&env));
    }

    // Call functions with arbitrary parameters to find unhandled panics
    // (e.g., overflow from missing math protection).
    for (i, action) in actions.into_iter().enumerate() {
        let caller = &users[i % users.len()];
        match action {
            FuzzAction::Stake { amount } => {
                let amt = amount as i128;
                let _ = client.try_stake(caller, &amt);
            }
            FuzzAction::Unstake { amount } => {
                let amt = amount as i128;
                let _ = client.try_request_unstake(caller, &amt);
            }
            FuzzAction::ClaimRewards => {
                let _ = client.try_claim_rewards(caller);
            }
        }
    }
});
