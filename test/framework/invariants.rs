//! # State Invariant Definitions & Verification
//!
//! Defines invariants that must hold across all contract state transitions.
//! Invariants are checked after every action during state exploration and
//! can be composed via the `InvariantSet` builder.
//!
//! ## Complexity
//!
//! - Each invariant check runs in O(n) time where n = number of tracked users,
//!   since we iterate all user stakes to compute sums.
//! - Full invariant verification after each action is O(k·n) where k = number
//!   of active invariants. With typical k ≤ 10 and n ≤ 50, this is negligible.

extern crate std;

use std::string::String;
use std::vec::Vec;

use super::StakingSnapshot;

// ── Invariant Trait ──────────────────────────────────────────────────────────

/// A named invariant that can be verified against a state snapshot.
pub trait Invariant {
    /// Human-readable name for error messages.
    fn name(&self) -> &str;

    /// Check the invariant. Returns `Ok(())` on success, `Err(description)` on violation.
    fn check(&self, snapshot: &StakingSnapshot) -> Result<(), String>;
}

// ── Built-in Invariants ──────────────────────────────────────────────────────

/// **Total Stake Consistency**: `total_staked == Σ(user_stakes)`
///
/// This is the most critical financial invariant. A violation indicates
/// that tokens are being created or destroyed during stake/unstake operations.
pub struct TotalStakeConsistency;

impl Invariant for TotalStakeConsistency {
    fn name(&self) -> &str {
        "total_staked == sum(user_stakes)"
    }

    fn check(&self, snapshot: &StakingSnapshot) -> Result<(), String> {
        let sum = snapshot.sum_user_stakes();
        if snapshot.total_staked != sum {
            return Err(std::format!(
                "Total staked ({}) != sum of user stakes ({})",
                snapshot.total_staked,
                sum
            ));
        }
        Ok(())
    }
}

/// **Non-Negative Stakes**: All user stakes must be ≥ 0.
///
/// Prevents underflow from incorrect subtraction when unstaking.
pub struct NonNegativeStakes;

impl Invariant for NonNegativeStakes {
    fn name(&self) -> &str {
        "all user stakes >= 0"
    }

    fn check(&self, snapshot: &StakingSnapshot) -> Result<(), String> {
        for (addr, stake) in &snapshot.user_stakes {
            if *stake < 0 {
                return Err(std::format!(
                    "User {:?} has negative stake: {}",
                    addr, stake
                ));
            }
        }
        Ok(())
    }
}

/// **Non-Negative Rewards**: All pending rewards must be ≥ 0.
///
/// A negative reward indicates an arithmetic bug in the reward accumulator.
pub struct NonNegativeRewards;

impl Invariant for NonNegativeRewards {
    fn name(&self) -> &str {
        "all pending rewards >= 0"
    }

    fn check(&self, snapshot: &StakingSnapshot) -> Result<(), String> {
        for (addr, reward) in &snapshot.user_rewards {
            if *reward < 0 {
                return Err(std::format!(
                    "User {:?} has negative pending rewards: {}",
                    addr, reward
                ));
            }
        }
        Ok(())
    }
}

/// **Non-Negative Total**: `total_staked >= 0`.
///
/// The global total must never go negative.
pub struct NonNegativeTotal;

impl Invariant for NonNegativeTotal {
    fn name(&self) -> &str {
        "total_staked >= 0"
    }

    fn check(&self, snapshot: &StakingSnapshot) -> Result<(), String> {
        if snapshot.total_staked < 0 {
            return Err(std::format!(
                "Total staked is negative: {}",
                snapshot.total_staked
            ));
        }
        Ok(())
    }
}

/// **Non-Negative Reward Rate**: reward rate must be ≥ 0.
pub struct NonNegativeRewardRate;

impl Invariant for NonNegativeRewardRate {
    fn name(&self) -> &str {
        "reward_rate >= 0"
    }

    fn check(&self, snapshot: &StakingSnapshot) -> Result<(), String> {
        if snapshot.reward_rate < 0 {
            return Err(std::format!(
                "Reward rate is negative: {}",
                snapshot.reward_rate
            ));
        }
        Ok(())
    }
}

/// **Stake Upper Bound**: No user's stake exceeds the global total.
///
/// Validates that individual stakes cannot exceed the aggregate, which would
/// indicate a double-credit bug.
pub struct StakeUpperBound;

impl Invariant for StakeUpperBound {
    fn name(&self) -> &str {
        "user_stake <= total_staked for all users"
    }

    fn check(&self, snapshot: &StakingSnapshot) -> Result<(), String> {
        for (addr, stake) in &snapshot.user_stakes {
            if *stake > snapshot.total_staked {
                return Err(std::format!(
                    "User {:?} stake ({}) exceeds total staked ({})",
                    addr, stake, snapshot.total_staked
                ));
            }
        }
        Ok(())
    }
}

/// **Reward Conservation**: When reward_rate > 0 and total_staked > 0 and
/// time has advanced, at least one user should have pending rewards > 0.
///
/// Checks that the reward distribution mechanism is actually distributing.
pub struct RewardDistributionActive;

impl Invariant for RewardDistributionActive {
    fn name(&self) -> &str {
        "rewards are distributed when rate > 0 and stakers exist"
    }

    fn check(&self, snapshot: &StakingSnapshot) -> Result<(), String> {
        if snapshot.reward_rate <= 0 || snapshot.total_staked <= 0 || snapshot.timestamp == 0 {
            return Ok(()); // Preconditions not met; invariant is vacuously true.
        }

        let any_rewards = snapshot.user_rewards.iter().any(|(_, r)| *r > 0);
        if !any_rewards {
            return Err(std::format!(
                "No rewards distributed despite rate={} and total_staked={} at t={}",
                snapshot.reward_rate, snapshot.total_staked, snapshot.timestamp
            ));
        }
        Ok(())
    }
}

/// **Monotonic Time**: The timestamp in a snapshot must not decrease between 
/// consecutive checks. Used via `TransitionInvariant` with two snapshots.
pub struct MonotonicTime;

impl MonotonicTime {
    /// Check monotonicity between two snapshots.
    pub fn check_transition(
        before: &StakingSnapshot,
        after: &StakingSnapshot,
    ) -> Result<(), String> {
        if after.timestamp < before.timestamp {
            return Err(std::format!(
                "Time went backwards: {} -> {}",
                before.timestamp, after.timestamp
            ));
        }
        Ok(())
    }
}

// ── Invariant Set ────────────────────────────────────────────────────────────

/// A composable set of invariants that are checked together.
///
/// Provides a builder API for assembling the invariant suite to verify.
pub struct InvariantSet {
    invariants: Vec<Box<dyn Invariant>>,
}

impl InvariantSet {
    /// Create an empty invariant set.
    pub fn new() -> Self {
        Self {
            invariants: Vec::new(),
        }
    }

    /// Create a set pre-loaded with all built-in staking invariants.
    pub fn staking_defaults() -> Self {
        let mut set = Self::new();
        set.add(Box::new(TotalStakeConsistency));
        set.add(Box::new(NonNegativeStakes));
        set.add(Box::new(NonNegativeRewards));
        set.add(Box::new(NonNegativeTotal));
        set.add(Box::new(NonNegativeRewardRate));
        set.add(Box::new(StakeUpperBound));
        set
    }

    /// Add a custom invariant.
    pub fn add(&mut self, invariant: Box<dyn Invariant>) {
        self.invariants.push(invariant);
    }

    /// Verify all invariants against a snapshot.
    /// Returns a list of (invariant_name, violation_message) for all failures.
    pub fn check_all(&self, snapshot: &StakingSnapshot) -> Vec<(String, String)> {
        let mut violations = Vec::new();
        for inv in &self.invariants {
            if let Err(msg) = inv.check(snapshot) {
                violations.push((inv.name().to_string(), msg));
            }
        }
        violations
    }

    /// Assert all invariants hold, panicking with details on violation.
    pub fn assert_all(&self, snapshot: &StakingSnapshot) {
        let violations = self.check_all(snapshot);
        if !violations.is_empty() {
            let mut report = String::from("Invariant violations detected:\n");
            for (name, msg) in &violations {
                report.push_str(&std::format!("  ✗ [{}]: {}\n", name, msg));
            }
            panic!("{}", report);
        }
    }

    /// Number of invariants in the set.
    pub fn len(&self) -> usize {
        self.invariants.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.invariants.is_empty()
    }
}

impl Default for InvariantSet {
    fn default() -> Self {
        Self::new()
    }
}

// ── Transition Invariants ────────────────────────────────────────────────────

/// Invariants that verify the relationship between two consecutive snapshots
/// (before and after an action).
pub trait TransitionInvariant {
    fn name(&self) -> &str;
    fn check(&self, before: &StakingSnapshot, after: &StakingSnapshot) -> Result<(), String>;
}

/// **Stake Conservation**: On a stake action of amount `a`,
/// `after.total_staked == before.total_staked + a`.
pub struct StakeConservation {
    pub amount: i128,
}

impl TransitionInvariant for StakeConservation {
    fn name(&self) -> &str {
        "stake conservation: total increases by staked amount"
    }

    fn check(&self, before: &StakingSnapshot, after: &StakingSnapshot) -> Result<(), String> {
        let expected = before.total_staked.saturating_add(self.amount);
        if after.total_staked != expected {
            return Err(std::format!(
                "After staking {}: expected total={}, got total={}",
                self.amount, expected, after.total_staked
            ));
        }
        Ok(())
    }
}

/// **Unstake Conservation**: On an unstake of amount `a`,
/// `after.total_staked == before.total_staked - a`.
pub struct UnstakeConservation {
    pub amount: i128,
}

impl TransitionInvariant for UnstakeConservation {
    fn name(&self) -> &str {
        "unstake conservation: total decreases by unstaked amount"
    }

    fn check(&self, before: &StakingSnapshot, after: &StakingSnapshot) -> Result<(), String> {
        let expected = before.total_staked.saturating_sub(self.amount);
        if after.total_staked != expected {
            return Err(std::format!(
                "After unstaking {}: expected total={}, got total={}",
                self.amount, expected, after.total_staked
            ));
        }
        Ok(())
    }
}

/// **Reward Monotonicity**: Pending rewards never decrease from time advancement
/// alone (they can decrease when claimed).
pub struct RewardMonotonicity;

impl TransitionInvariant for RewardMonotonicity {
    fn name(&self) -> &str {
        "pending rewards do not decrease from time advancement"
    }

    fn check(&self, before: &StakingSnapshot, after: &StakingSnapshot) -> Result<(), String> {
        // Only check when time advanced and no claims occurred.
        if after.timestamp <= before.timestamp {
            return Ok(());
        }

        for (addr, after_reward) in &after.user_rewards {
            if let Some((_, before_reward)) = before.user_rewards.iter().find(|(a, _)| a == addr) {
                if *after_reward < *before_reward {
                    return Err(std::format!(
                        "Rewards decreased for {:?}: {} -> {} (without claim)",
                        addr, before_reward, after_reward
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Composite checker for transition invariants.
pub struct TransitionInvariantSet {
    invariants: Vec<Box<dyn TransitionInvariant>>,
}

impl TransitionInvariantSet {
    pub fn new() -> Self {
        Self {
            invariants: Vec::new(),
        }
    }

    pub fn add(&mut self, invariant: Box<dyn TransitionInvariant>) {
        self.invariants.push(invariant);
    }

    pub fn check_all(
        &self,
        before: &StakingSnapshot,
        after: &StakingSnapshot,
    ) -> Vec<(String, String)> {
        let mut violations = Vec::new();
        for inv in &self.invariants {
            if let Err(msg) = inv.check(before, after) {
                violations.push((inv.name().to_string(), msg));
            }
        }
        violations
    }

    pub fn assert_all(&self, before: &StakingSnapshot, after: &StakingSnapshot) {
        let violations = self.check_all(before, after);
        if !violations.is_empty() {
            let mut report = String::from("Transition invariant violations:\n");
            for (name, msg) in &violations {
                report.push_str(&std::format!("  ✗ [{}]: {}\n", name, msg));
            }
            panic!("{}", report);
        }
    }
}

impl Default for TransitionInvariantSet {
    fn default() -> Self {
        Self::new()
    }
}
