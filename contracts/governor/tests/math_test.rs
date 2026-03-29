#![cfg(test)]

extern crate std;

use governor::voting::{compute_vote_power, isqrt, loyalty_multiplier_scaled, SCALE};
use governor::GovernorContract;
use soroban_sdk::{Address, Env};

fn setup_env() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(GovernorContract, ());
    (env, contract_id)
}

#[test]
fn test_isqrt_max_and_properties() {
    let (_env, _cid) = setup_env();

    let n: i128 = i128::MAX;
    let r = isqrt(n);

    // r*r must be <= n
    let sq = r.checked_mul(r).expect("square should not overflow");
    assert!(sq <= n);

    // (r+1)^2 should be > n or overflow (checked_mul returns None)
    let r1 = r.saturating_add(1);
    let next_sq_ok = r1.checked_mul(r1);
    if let Some(v) = next_sq_ok {
        assert!(v > n);
    }

    // negative input returns 0
    assert_eq!(isqrt(-1), 0);
}

#[test]
fn test_loyalty_multiplier_caps_at_max_days() {
    let (_env, _cid) = setup_env();

    let m = loyalty_multiplier_scaled(u64::MAX);
    // loyalty should cap at 2x SCALE
    assert_eq!(m, SCALE + SCALE);
}

#[test]
fn test_compute_vote_power_with_max_values() {
    let (_env, _cid) = setup_env();

    let staked: i128 = i128::MAX;
    let stake_age: u64 = u64::MAX;

    // compute should not panic and should equal raw*loyalty/SCALE
    let raw = isqrt(staked);
    let loyalty = loyalty_multiplier_scaled(stake_age);
    let expected = raw.saturating_mul(loyalty) / SCALE;

    let got = compute_vote_power(staked, stake_age);
    assert_eq!(got, expected);
}
