//! # State Space Explorer
//!
//! Systematically explores reachable contract states by executing action
//! sequences and verifying invariants after every transition.
//!
//! ## Design
//!
//! The explorer uses a bounded depth-first search over action sequences.
//! Each explored state is a `StakingSnapshot`; edges are `StakingAction`s.
//!
//! ## Complexity
//!
//! - Time: O(A^D × I × U) where A = actions per step, D = max depth,
//!   I = invariants, U = tracked users. In practice bounded by `max_steps`.
//! - Space: O(D × U) for the snapshot history (one snapshot per depth level).

extern crate std;

use soroban_sdk::Address;
use std::string::String;
use std::vec::Vec;

use super::generators::StakingAction;
use super::invariants::InvariantSet;
use super::{ActionOutcome, StakingTestHarness, TestRunSummary};

// ── Explorer Configuration ───────────────────────────────────────────────────

/// Configuration for state-space exploration.
#[derive(Debug, Clone)]
pub struct ExplorerConfig {
    /// Maximum number of actions to execute in a single exploration run.
    pub max_steps: usize,
    /// Whether to halt on the first invariant violation (fail-fast).
    pub fail_fast: bool,
    /// Whether to record snapshots for later analysis.
    pub record_snapshots: bool,
}

impl Default for ExplorerConfig {
    fn default() -> Self {
        Self {
            max_steps: 100,
            fail_fast: true,
            record_snapshots: false,
        }
    }
}

// ── Exploration Result ───────────────────────────────────────────────────────

/// Full result of an exploration run.
#[derive(Debug)]
pub struct ExplorationResult {
    pub summary: TestRunSummary,
    pub snapshots: Vec<super::StakingSnapshot>,
    pub action_log: Vec<(StakingAction, ActionOutcome)>,
}

impl ExplorationResult {
    pub fn passed(&self) -> bool {
        self.summary.passed()
    }
}

// ── State Space Explorer ─────────────────────────────────────────────────────

/// Executes action sequences against a staking contract, checking invariants
/// after every transition.
///
/// Tracks coverage metrics including entry points hit and transitions observed.
pub struct StateExplorer<'a> {
    harness: &'a StakingTestHarness<'a>,
    invariants: InvariantSet,
    config: ExplorerConfig,
    users: Vec<Address>,
    /// Tracks the next expected unstake request ID.
    next_request_id: u64,
}

impl<'a> StateExplorer<'a> {
    /// Create an explorer for the given harness and user pool.
    pub fn new(
        harness: &'a StakingTestHarness<'a>,
        invariants: InvariantSet,
        config: ExplorerConfig,
        users: Vec<Address>,
    ) -> Self {
        Self {
            harness,
            invariants,
            config,
            users,
            next_request_id: 1,
        }
    }

    /// Create an explorer with default configuration and built-in invariants.
    pub fn with_defaults(
        harness: &'a StakingTestHarness<'a>,
        users: Vec<Address>,
    ) -> Self {
        Self::new(
            harness,
            InvariantSet::staking_defaults(),
            ExplorerConfig::default(),
            users,
        )
    }

    /// Execute a sequence of actions, checking invariants after each.
    ///
    /// Returns an `ExplorationResult` with full coverage metrics.
    pub fn explore(&mut self, actions: &[StakingAction]) -> ExplorationResult {
        let mut summary = TestRunSummary::new();
        let mut snapshots = Vec::new();
        let mut action_log = Vec::new();

        // Take initial snapshot.
        let initial = self.harness.snapshot(&self.users);
        if self.config.record_snapshots {
            snapshots.push(initial);
        }

        let steps = actions.len().min(self.config.max_steps);

        for action in actions.iter().take(steps) {
            let outcome = self.execute_action(action);
            let entry_point = action_entry_point(action);
            summary.entry_points_hit.insert(entry_point);
            summary.actions_executed += 1;
            summary.transitions_observed += 1;

            action_log.push((action.clone(), outcome));

            // Check invariants after each action.
            let snapshot = self.harness.snapshot(&self.users);
            let violations = self.invariants.check_all(&snapshot);
            summary.invariant_checks += 1;

            for (name, msg) in violations {
                let violation = std::format!(
                    "After action #{} ({:?}): [{}] {}",
                    summary.actions_executed,
                    action,
                    name,
                    msg
                );
                summary.invariant_violations.push(violation);

                if self.config.fail_fast {
                    if self.config.record_snapshots {
                        snapshots.push(snapshot);
                    }
                    return ExplorationResult {
                        summary,
                        snapshots,
                        action_log,
                    };
                }
            }

            if self.config.record_snapshots {
                snapshots.push(snapshot);
            }
        }

        ExplorationResult {
            summary,
            snapshots,
            action_log,
        }
    }

    /// Execute a single action against the harness, returning the outcome.
    fn execute_action(&mut self, action: &StakingAction) -> ActionOutcome {
        match action {
            StakingAction::Stake { user_index, amount } => {
                let user = &self.users[*user_index % self.users.len()];
                // Ensure user has enough tokens.
                self.harness
                    .env
                    .mint_tokens(&self.harness.stake_token, user, *amount);
                match self.harness.client.try_stake(user, amount) {
                    Ok(_) => ActionOutcome::Ok,
                    Err(Ok(e)) => ActionOutcome::ExpectedError(e as u32),
                    Err(Err(e)) => {
                        ActionOutcome::UnexpectedError(std::format!("{:?}", e))
                    }
                }
            }
            StakingAction::RequestUnstake { user_index, amount } => {
                let user = &self.users[*user_index % self.users.len()];
                match self.harness.client.try_request_unstake(user, amount) {
                    Ok(id) => {
                        self.next_request_id = id.unwrap_or(0) + 1;
                        ActionOutcome::Ok
                    }
                    Err(Ok(e)) => ActionOutcome::ExpectedError(e as u32),
                    Err(Err(e)) => {
                        ActionOutcome::UnexpectedError(std::format!("{:?}", e))
                    }
                }
            }
            StakingAction::Withdraw {
                user_index,
                request_id,
            } => {
                let user = &self.users[*user_index % self.users.len()];
                match self.harness.client.try_withdraw(user, request_id) {
                    Ok(_) => ActionOutcome::Ok,
                    Err(Ok(e)) => ActionOutcome::ExpectedError(e as u32),
                    Err(Err(e)) => {
                        ActionOutcome::UnexpectedError(std::format!("{:?}", e))
                    }
                }
            }
            StakingAction::ClaimRewards { user_index } => {
                let user = &self.users[*user_index % self.users.len()];
                match self.harness.client.try_claim_rewards(user) {
                    Ok(_) => ActionOutcome::Ok,
                    Err(Ok(e)) => ActionOutcome::ExpectedError(e as u32),
                    Err(Err(e)) => {
                        ActionOutcome::UnexpectedError(std::format!("{:?}", e))
                    }
                }
            }
            StakingAction::AdvanceTime { delta } => {
                self.harness.env.advance_time(*delta);
                ActionOutcome::Ok
            }
            StakingAction::SetRewardRate { new_rate } => {
                match self.harness.client.try_set_reward_rate(
                    &self.harness.admin,
                    new_rate,
                    &0u64,
                ) {
                    Ok(_) => ActionOutcome::Ok,
                    Err(Ok(e)) => ActionOutcome::ExpectedError(e as u32),
                    Err(Err(e)) => {
                        ActionOutcome::UnexpectedError(std::format!("{:?}", e))
                    }
                }
            }
            StakingAction::SetLockPeriod { new_period } => {
                match self.harness.client.try_set_lock_period(
                    &self.harness.admin,
                    new_period,
                    &0u64,
                ) {
                    Ok(_) => ActionOutcome::Ok,
                    Err(Ok(e)) => ActionOutcome::ExpectedError(e as u32),
                    Err(Err(e)) => {
                        ActionOutcome::UnexpectedError(std::format!("{:?}", e))
                    }
                }
            }
            StakingAction::Pause => {
                match self.harness.client.try_pause(&self.harness.admin) {
                    Ok(_) => ActionOutcome::Ok,
                    Err(Ok(e)) => ActionOutcome::ExpectedError(e as u32),
                    Err(Err(e)) => {
                        ActionOutcome::UnexpectedError(std::format!("{:?}", e))
                    }
                }
            }
            StakingAction::Unpause => {
                match self.harness.client.try_unpause(&self.harness.admin) {
                    Ok(_) => ActionOutcome::Ok,
                    Err(Ok(e)) => ActionOutcome::ExpectedError(e as u32),
                    Err(Err(e)) => {
                        ActionOutcome::UnexpectedError(std::format!("{:?}", e))
                    }
                }
            }
        }
    }
}

/// Map a staking action to its entry point name for coverage tracking.
fn action_entry_point(action: &StakingAction) -> String {
    match action {
        StakingAction::Stake { .. } => "stake".into(),
        StakingAction::RequestUnstake { .. } => "request_unstake".into(),
        StakingAction::Withdraw { .. } => "withdraw".into(),
        StakingAction::ClaimRewards { .. } => "claim_rewards".into(),
        StakingAction::AdvanceTime { .. } => "advance_time".into(),
        StakingAction::SetRewardRate { .. } => "set_reward_rate".into(),
        StakingAction::SetLockPeriod { .. } => "set_lock_period".into(),
        StakingAction::Pause => "pause".into(),
        StakingAction::Unpause => "unpause".into(),
    }
}

/// The complete set of staking contract entry points, for coverage calculation.
pub const STAKING_ENTRY_POINTS: &[&str] = &[
    "stake",
    "request_unstake",
    "withdraw",
    "claim_rewards",
    "set_reward_rate",
    "set_lock_period",
    "pause",
    "unpause",
];
