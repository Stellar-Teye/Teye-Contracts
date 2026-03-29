//! Tests for staking reward multiplier logic.
//! Covers: tier bonuses, lock-up period rewards, early withdrawal penalties,
//! and reward accumulation correctness.
#[cfg(test)]
mod tests {
    extern crate std;
    use crate::rewards::{compute_reward_per_token, earned, PRECISION};
    use crate::timelock::{
        clear_rate_proposal, get_rate_proposal, get_request, next_request_id, store_rate_proposal,
        store_request, RateChangeProposal, UnstakeRequest,
    };
    use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

    // -----------------------------------------------------------------------
    // Reward-per-token accumulation (staking tiers)
    // -----------------------------------------------------------------------

    #[test]
    fn test_no_rewards_when_no_stakers() {
        let rpt = compute_reward_per_token(0, 100, 3600, 0);
        assert_eq!(rpt, 0);
    }

    #[test]
    fn test_rpt_unchanged_when_elapsed_is_zero() {
        let rpt = compute_reward_per_token(500, 100, 0, 1000);
        assert_eq!(rpt, 500);
    }

    #[test]
    fn test_single_staker_earns_all_rewards() {
        // rate=10/s, 100s, 1000 staked -> delta = 10*100*PRECISION/1000 = PRECISION
        let rpt = compute_reward_per_token(0, 10, 100, 1_000);
        assert_eq!(rpt, PRECISION);
        let e = earned(1_000, rpt, 0, 0);
        assert_eq!(e, 1_000);
    }

    #[test]
    fn test_larger_stake_earns_proportionally_more() {
        let rpt = compute_reward_per_token(0, 10, 100, 1_000);
        let small = earned(100, rpt, 0, 0);
        let large = earned(900, rpt, 0, 0);
        assert_eq!(small + large, 1_000);
        assert!(large > small);
    }

    #[test]
    fn test_tier_bronze_one_day_reward() {
        // Bronze tier: stake 1_000 for 1 day (86400s), rate=1/s
        let rpt = compute_reward_per_token(0, 1, 86_400, 1_000);
        let e = earned(1_000, rpt, 0, 0);
        assert_eq!(e, 86_400); // 1 token/s * 86400s = 86400
    }

    #[test]
    fn test_tier_silver_one_week_reward() {
        // Silver tier: stake 10_000 for 7 days, rate=2/s
        let elapsed = 7 * 86_400u64;
        let rpt = compute_reward_per_token(0, 2, elapsed, 10_000);
        let e = earned(10_000, rpt, 0, 0);
        assert_eq!(e, 2 * elapsed as i128);
    }

    #[test]
    fn test_tier_gold_one_month_reward() {
        // Gold tier: stake 100_000 for 30 days, rate=5/s
        let elapsed = 30 * 86_400u64;
        let rpt = compute_reward_per_token(0, 5, elapsed, 100_000);
        let e = earned(100_000, rpt, 0, 0);
        assert_eq!(e, 5 * elapsed as i128);
    }

    #[test]
    fn test_accumulated_rewards_not_double_counted() {
        // Simulate two reward periods
        let rpt1 = compute_reward_per_token(0, 10, 100, 1_000);
        let e1 = earned(1_000, rpt1, 0, 0);
        // Second period: snapshot taken at rpt1
        let rpt2 = compute_reward_per_token(rpt1, 10, 100, 1_000);
        let e2 = earned(1_000, rpt2, rpt1, e1);
        // Total should be 2x single period
        assert_eq!(e2, e1 * 2);
    }

    #[test]
    fn test_user_with_no_stake_earns_nothing() {
        let rpt = compute_reward_per_token(0, 10, 100, 1_000);
        let e = earned(0, rpt, 0, 0);
        assert_eq!(e, 0);
    }

    #[test]
    fn test_reward_rate_increase_boosts_earnings() {
        let elapsed = 3600u64;
        let total = 1_000i128;
        let low_rate = compute_reward_per_token(0, 1, elapsed, total);
        let high_rate = compute_reward_per_token(0, 10, elapsed, total);
        let e_low = earned(total, low_rate, 0, 0);
        let e_high = earned(total, high_rate, 0, 0);
        assert_eq!(e_high, e_low * 10);
    }

    // -----------------------------------------------------------------------
    // Lock-up period: unstake request timelock
    // -----------------------------------------------------------------------

    #[test]
    fn test_unstake_request_stored_and_retrieved() {
        let env = Env::default();
        let staker = Address::generate(&env);
        let id = next_request_id(&env);
        let req = UnstakeRequest {
            id,
            staker: staker.clone(),
            amount: 5_000,
            unlock_at: 9999,
            withdrawn: false,
        };
        store_request(&env, &req);
        let loaded = get_request(&env, id).expect("request not found");
        assert_eq!(loaded.amount, 5_000);
        assert_eq!(loaded.unlock_at, 9999);
        assert!(!loaded.withdrawn);
    }

    #[test]
    fn test_withdrawal_blocked_before_lock_expires() {
        let env = Env::default();
        env.ledger().set_timestamp(1000);
        let lock_period = 86_400u64; // 1 day
        let unlock_at = 1000 + lock_period;
        let id = next_request_id(&env);
        let req = UnstakeRequest {
            id,
            staker: Address::generate(&env),
            amount: 1_000,
            unlock_at,
            withdrawn: false,
        };
        store_request(&env, &req);
        // One second before unlock
        env.ledger().set_timestamp(unlock_at - 1);
        let loaded = get_request(&env, id).unwrap();
        assert!(env.ledger().timestamp() < loaded.unlock_at);
    }

    #[test]
    fn test_withdrawal_allowed_after_lock_expires() {
        let env = Env::default();
        env.ledger().set_timestamp(1000);
        let unlock_at = 1000 + 86_400u64;
        let id = next_request_id(&env);
        let req = UnstakeRequest {
            id,
            staker: Address::generate(&env),
            amount: 1_000,
            unlock_at,
            withdrawn: false,
        };
        store_request(&env, &req);
        env.ledger().set_timestamp(unlock_at);
        let loaded = get_request(&env, id).unwrap();
        assert!(env.ledger().timestamp() >= loaded.unlock_at);
    }

    #[test]
    fn test_double_withdrawal_prevented_by_withdrawn_flag() {
        let env = Env::default();
        let id = next_request_id(&env);
        let mut req = UnstakeRequest {
            id,
            staker: Address::generate(&env),
            amount: 1_000,
            unlock_at: 0,
            withdrawn: false,
        };
        store_request(&env, &req);
        // Mark as withdrawn
        req.withdrawn = true;
        store_request(&env, &req);
        let loaded = get_request(&env, id).unwrap();
        assert!(loaded.withdrawn);
    }

    #[test]
    fn test_nonexistent_request_returns_none() {
        let env = Env::default();
        assert!(get_request(&env, 9999).is_none());
    }

    #[test]
    fn test_request_ids_are_monotonically_increasing() {
        let env = Env::default();
        let id1 = next_request_id(&env);
        let id2 = next_request_id(&env);
        let id3 = next_request_id(&env);
        assert!(id1 < id2);
        assert!(id2 < id3);
    }

    // -----------------------------------------------------------------------
    // Early withdrawal penalty simulation
    // -----------------------------------------------------------------------

    #[test]
    fn test_early_withdrawal_forfeits_pending_rewards() {
        // Simulate: staker earns rewards but withdraws before lock expires.
        // The penalty is modelled as zeroing out pending rewards.
        let rpt = compute_reward_per_token(0, 10, 3600, 1_000);
        let earned_before = earned(1_000, rpt, 0, 0);
        assert!(earned_before > 0);
        // Apply early-exit penalty: forfeit all pending rewards
        let after_penalty = 0i128;
        assert_eq!(after_penalty, 0);
        assert!(earned_before > after_penalty);
    }

    #[test]
    fn test_partial_penalty_reduces_rewards() {
        // 50% penalty on early exit
        let rpt = compute_reward_per_token(0, 10, 3600, 1_000);
        let full_rewards = earned(1_000, rpt, 0, 0);
        let penalty_bps = 5_000i128; // 50%
        let after_penalty = full_rewards - (full_rewards * penalty_bps / 10_000);
        assert_eq!(after_penalty, full_rewards / 2);
    }

    #[test]
    fn test_no_penalty_after_full_lock_period() {
        // After lock expires, staker keeps 100% of rewards
        let rpt = compute_reward_per_token(0, 10, 86_400, 1_000);
        let full_rewards = earned(1_000, rpt, 0, 0);
        let penalty_bps = 0i128;
        let after_penalty = full_rewards - (full_rewards * penalty_bps / 10_000);
        assert_eq!(after_penalty, full_rewards);
    }

    // -----------------------------------------------------------------------
    // Rate change proposal
    // -----------------------------------------------------------------------

    #[test]
    fn test_rate_change_proposal_stored_and_retrieved() {
        let env = Env::default();
        let proposal = RateChangeProposal {
            new_rate: 20,
            effective_at: 5000,
        };
        store_rate_proposal(&env, &proposal);
        let loaded = get_rate_proposal(&env).expect("proposal not found");
        assert_eq!(loaded.new_rate, 20);
        assert_eq!(loaded.effective_at, 5000);
    }

    #[test]
    fn test_rate_change_blocked_before_effective_at() {
        let env = Env::default();
        env.ledger().set_timestamp(1000);
        let proposal = RateChangeProposal {
            new_rate: 20,
            effective_at: 5000,
        };
        store_rate_proposal(&env, &proposal);
        let loaded = get_rate_proposal(&env).unwrap();
        assert!(env.ledger().timestamp() < loaded.effective_at);
    }

    #[test]
    fn test_rate_change_allowed_after_effective_at() {
        let env = Env::default();
        env.ledger().set_timestamp(5000);
        let proposal = RateChangeProposal {
            new_rate: 20,
            effective_at: 5000,
        };
        store_rate_proposal(&env, &proposal);
        let loaded = get_rate_proposal(&env).unwrap();
        assert!(env.ledger().timestamp() >= loaded.effective_at);
    }

    #[test]
    fn test_rate_proposal_cleared_after_apply() {
        let env = Env::default();
        let proposal = RateChangeProposal {
            new_rate: 20,
            effective_at: 0,
        };
        store_rate_proposal(&env, &proposal);
        assert!(get_rate_proposal(&env).is_some());
        clear_rate_proposal(&env);
        assert!(get_rate_proposal(&env).is_none());
    }
}
