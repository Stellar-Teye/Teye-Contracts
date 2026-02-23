/// Fixed-point scaling factor.
///
/// All reward-per-token values are multiplied by this constant before storage
/// to preserve sub-unit precision without floating-point arithmetic.
/// Using 10^12 gives 12 decimal places of precision, which is more than
/// sufficient for token amounts up to 10^18.
pub const PRECISION: i128 = 1_000_000_000_000;

// ── Core reward engine ──────────────────────────────────────────────────────

/// Recompute the global `reward_per_token_stored` value.
///
/// This is the fundamental O(1) accumulation step:
///
/// ```text
/// Δrpt = reward_rate × elapsed_seconds × PRECISION / total_staked
/// new_rpt = stored_rpt + Δrpt
/// ```
///
/// When `total_staked` is zero we return `stored` unchanged — no stakers
/// means no distribution, preventing division-by-zero and orphaned rewards.
///
/// # Arguments
/// * `stored`       – current `reward_per_token_stored` (scaled by PRECISION)
/// * `reward_rate`  – tokens emitted per second across *all* stakers
/// * `elapsed`      – seconds since the last update
/// * `total_staked` – sum of all active stakes
#[allow(clippy::arithmetic_side_effects)]
pub fn compute_reward_per_token(
    stored: i128,
    reward_rate: i128,
    elapsed: u64,
    total_staked: i128,
) -> i128 {
    if total_staked <= 0 {
        return stored;
    }

    // Widen to i128 — reward_rate and PRECISION fit comfortably.
    // elapsed is u64; cast to i128 is safe since u64::MAX < i128::MAX.
    let delta = reward_rate
        .saturating_mul(elapsed as i128)
        .saturating_mul(PRECISION)
        / total_staked;

    stored.saturating_add(delta)
}

/// Calculate the total rewards earned by a single staker.
///
/// ```text
/// earned = staked × (current_rpt − user_rpt_paid) / PRECISION + user_earned
/// ```
///
/// The subtraction `current_rpt − user_rpt_paid` isolates only the
/// accumulation that happened *since the user's last snapshot*, so prior
/// claims/snapshots are never double-counted.
///
/// # Arguments
/// * `staked`        – user's current staked balance
/// * `current_rpt`   – latest global `reward_per_token_stored`
/// * `user_rpt_paid` – the snapshot taken at the user's last interaction
/// * `user_earned`   – already-accumulated rewards not yet claimed
#[allow(clippy::arithmetic_side_effects)]
pub fn earned(staked: i128, current_rpt: i128, user_rpt_paid: i128, user_earned: i128) -> i128 {
    let new_rewards = staked.saturating_mul(current_rpt.saturating_sub(user_rpt_paid)) / PRECISION;

    user_earned.saturating_add(new_rewards)
}

// ── Unit tests ──────────────────────────────────────────────────────────────
// These are pure-math tests with no Soroban environment dependency.

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn rpt_zero_when_no_stakers() {
        let rpt = compute_reward_per_token(500, 100, 60, 0);
        assert_eq!(rpt, 500, "RPT must not change when total_staked is zero");
    }

    #[test]
    fn rpt_accumulates_correctly() {
        // reward_rate=10 tokens/s, elapsed=100s, total_staked=1_000 tokens
        // Δrpt = 10 × 100 × PRECISION / 1_000 = 1_000 × PRECISION / 1_000 = PRECISION
        let rpt = compute_reward_per_token(0, 10, 100, 1_000);
        assert_eq!(rpt, PRECISION);
    }

    #[test]
    fn earned_zero_when_no_new_accumulation() {
        // If user's snapshot equals current RPT, no new rewards.
        let e = earned(500, 100, 100, 50);
        assert_eq!(e, 50);
    }

    #[test]
    fn earned_proportional_to_stake() {
        // RPT increased by PRECISION since last snapshot.
        // staked=1_000 → earned_new = 1_000 × PRECISION / PRECISION = 1_000
        let e = earned(1_000, PRECISION, 0, 0);
        assert_eq!(e, 1_000);
    }

    #[test]
    fn earned_does_not_overflow_large_amounts() {
        // Stress test: large stake × large RPT delta.
        // i128::MAX ≈ 1.7 × 10^38; with PRECISION = 10^12 and typical token
        // decimals of 7 (Stellar), staked values up to 10^15 are realistic.
        // saturating_mul clamps at i128::MAX rather than wrapping, so the
        // result must be positive and the call must not panic.
        let large_stake: i128 = 1_000_000_000_000_000; // 10^15
        let rpt_delta = PRECISION.saturating_mul(1_000); // large accumulation
        let e = earned(large_stake, rpt_delta, 0, 0);
        assert!(e > 0);
        assert_eq!(e, large_stake.saturating_mul(1_000)); // 10^15 × 1000 = 10^18
    }
}
