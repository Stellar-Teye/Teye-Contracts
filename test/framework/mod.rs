//! # Teye Contract Testing Framework
//!
//! A comprehensive, reusable testing harness for Soroban smart contracts
//! supporting property-based testing, invariant checking, state exploration,
//! and a declarative scenario DSL.
//!
//! ## Architecture
//!
//! ```text
//! test/framework/
//! ├── mod.rs             — Core TestEnv, re-exports
//! ├── generators.rs      — Property-based test value generators
//! ├── invariants.rs      — State invariant definitions & verification
//! ├── state_explorer.rs  — Systematic state-space exploration
//! └── scenario_dsl.rs    — Declarative test scenario builder
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use test_framework::{TestEnv, Invariant, scenario};
//!
//! let env = TestEnv::new();
//! let staking = env.deploy_staking(10, 86_400);
//!
//! scenario! {
//!     given staking.initialized(),
//!     when  staking.stake(&alice, 1_000),
//!     then  staking.total_staked() == 1_000,
//! }
//! ```

extern crate std;

pub mod generators;
pub mod invariants;
pub mod scenario_dsl;
pub mod state_explorer;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::StellarAssetClient,
    Address, Env,
};
use staking::{StakingContract, StakingContractClient};

// ── Core Test Environment ────────────────────────────────────────────────────

/// A high-level test environment that wraps the Soroban `Env` and provides
/// contract deployment, time control, and address management.
///
/// Designed for O(1) setup cost per test; token minting and contract
/// registration are amortised across the environment lifetime.
pub struct TestEnv {
    pub env: Env,
    generated_addresses: std::vec::Vec<Address>,
}

impl TestEnv {
    /// Create a new test environment with all auth mocked.
    pub fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        Self {
            env,
            generated_addresses: std::vec::Vec::new(),
        }
    }

    /// Generate a fresh Soroban address (cached for re-use).
    pub fn generate_address(&mut self) -> Address {
        let addr = Address::generate(&self.env);
        self.generated_addresses.push(addr.clone());
        addr
    }

    /// Generate `n` distinct addresses.
    pub fn generate_addresses(&mut self, n: usize) -> std::vec::Vec<Address> {
        (0..n).map(|_| self.generate_address()).collect()
    }

    /// Set the ledger timestamp.
    pub fn set_timestamp(&self, ts: u64) {
        self.env.ledger().set_timestamp(ts);
    }

    /// Advance the ledger timestamp by `delta` seconds.
    pub fn advance_time(&self, delta: u64) {
        let current = self.env.ledger().timestamp();
        self.env.ledger().set_timestamp(current.saturating_add(delta));
    }

    /// Current ledger timestamp.
    pub fn timestamp(&self) -> u64 {
        self.env.ledger().timestamp()
    }

    /// Deploy a pair of SAC token contracts and return their addresses.
    pub fn deploy_token_pair(&self) -> (Address, Address) {
        let token_a = self
            .env
            .register_stellar_asset_contract_v2(Address::generate(&self.env));
        let token_b = self
            .env
            .register_stellar_asset_contract_v2(Address::generate(&self.env));
        (token_a.address(), token_b.address())
    }

    /// Mint tokens from a SAC token to a recipient.
    pub fn mint_tokens(&self, token: &Address, recipient: &Address, amount: i128) {
        StellarAssetClient::new(&self.env, token).mint(recipient, &amount);
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

// ── Staking-Specific Harness ─────────────────────────────────────────────────

/// Pre-wired staking contract test fixture with token contracts deployed.
///
/// Provides a higher-level API that eliminates boilerplate in staking tests.
pub struct StakingTestHarness<'a> {
    pub env: &'a mut TestEnv,
    pub client: StakingContractClient<'static>,
    pub contract_id: Address,
    pub admin: Address,
    pub stake_token: Address,
    pub reward_token: Address,
}

impl<'a> StakingTestHarness<'a> {
    /// Deploy and initialize a staking contract with the given parameters.
    ///
    /// Pre-funds the contract with 1 billion reward tokens.
    pub fn new(env: &'a mut TestEnv, reward_rate: i128, lock_period: u64) -> Self {
        let (stake_token, reward_token) = env.deploy_token_pair();
        let contract_id = env.env.register(StakingContract, ());
        let client = StakingContractClient::new(&env.env, &contract_id);
        let admin = env.generate_address();

        client.initialize(&admin, &stake_token, &reward_token, &reward_rate, &lock_period);

        // Pre-fund with reward tokens.
        StellarAssetClient::new(&env.env, &reward_token)
            .mock_all_auths()
            .mint(&contract_id, &1_000_000_000i128);

        Self {
            env,
            client,
            contract_id,
            admin,
            stake_token,
            reward_token,
        }
    }

    /// Create a funded staker with `amount` stake tokens.
    pub fn create_staker(&self, amount: i128) -> Address {
        let staker = Address::generate(&self.env.env);
        self.env.mint_tokens(&self.stake_token, &staker, amount);
        staker
    }

    /// Stake tokens for a user.
    pub fn stake(&self, staker: &Address, amount: i128) {
        self.client.stake(staker, &amount);
    }

    /// Request unstake and return the request ID.
    pub fn request_unstake(&self, staker: &Address, amount: i128) -> u64 {
        self.client.request_unstake(staker, &amount)
    }

    /// Withdraw from an unstake request.
    pub fn withdraw(&self, staker: &Address, request_id: u64) {
        self.client.withdraw(staker, &request_id);
    }

    /// Claim rewards for a staker.
    pub fn claim_rewards(&self, staker: &Address) -> i128 {
        self.client.claim_rewards(staker)
    }

    /// Read total staked.
    pub fn total_staked(&self) -> i128 {
        self.client.get_total_staked()
    }

    /// Read a user's staked balance.
    pub fn user_staked(&self, staker: &Address) -> i128 {
        self.client.get_staked(staker)
    }

    /// Read a user's pending rewards.
    pub fn pending_rewards(&self, staker: &Address) -> i128 {
        self.client.get_pending_rewards(staker)
    }

    /// Snapshot of all observable staking state for invariant checking.
    pub fn snapshot(&self, stakers: &[Address]) -> StakingSnapshot {
        let total_staked = self.total_staked();
        let reward_rate = self.client.get_reward_rate();
        let lock_period = self.client.get_lock_period();

        let user_stakes: std::vec::Vec<(Address, i128)> = stakers
            .iter()
            .map(|s| (s.clone(), self.user_staked(s)))
            .collect();

        let user_rewards: std::vec::Vec<(Address, i128)> = stakers
            .iter()
            .map(|s| (s.clone(), self.pending_rewards(s)))
            .collect();

        StakingSnapshot {
            timestamp: self.env.timestamp(),
            total_staked,
            reward_rate,
            lock_period,
            user_stakes,
            user_rewards,
        }
    }
}

/// Immutable snapshot of staking contract state at a point in time.
///
/// Used by invariant checkers and the state explorer for O(1) state comparisons.
#[derive(Debug, Clone)]
pub struct StakingSnapshot {
    pub timestamp: u64,
    pub total_staked: i128,
    pub reward_rate: i128,
    pub lock_period: u64,
    pub user_stakes: std::vec::Vec<(Address, i128)>,
    pub user_rewards: std::vec::Vec<(Address, i128)>,
}

impl StakingSnapshot {
    /// Sum of all individual user stakes.
    pub fn sum_user_stakes(&self) -> i128 {
        self.user_stakes.iter().map(|(_, s)| s).sum()
    }
}

// ── Test Outcome Tracking ────────────────────────────────────────────────────

/// Result of a single test action, used by the state explorer and scenario DSL.
#[derive(Debug, Clone)]
pub enum ActionOutcome {
    /// The action succeeded.
    Ok,
    /// The action failed with the expected error.
    ExpectedError(u32),
    /// The action failed unexpectedly.
    UnexpectedError(std::string::String),
}

/// Summary of a test run with coverage metrics.
#[derive(Debug, Clone)]
pub struct TestRunSummary {
    pub actions_executed: usize,
    pub invariant_checks: usize,
    pub invariant_violations: std::vec::Vec<std::string::String>,
    pub entry_points_hit: std::collections::HashSet<std::string::String>,
    pub transitions_observed: usize,
}

impl TestRunSummary {
    pub fn new() -> Self {
        Self {
            actions_executed: 0,
            invariant_checks: 0,
            invariant_violations: std::vec::Vec::new(),
            entry_points_hit: std::collections::HashSet::new(),
            transitions_observed: 0,
        }
    }

    /// True when no invariant violations were detected.
    pub fn passed(&self) -> bool {
        self.invariant_violations.is_empty()
    }

    /// Coverage ratio: entry points hit / total known entry points.
    pub fn entry_point_coverage(&self, total_entry_points: usize) -> f64 {
        if total_entry_points == 0 {
            return 0.0;
        }
        self.entry_points_hit.len() as f64 / total_entry_points as f64
    }
}

impl Default for TestRunSummary {
    fn default() -> Self {
        Self::new()
    }
}
