#![no_std]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod events;
pub mod rewards;
pub mod timelock;

use common::admin_tiers::{self, AdminTier};
use common::multisig;
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, BytesN, Env, String, Symbol,
};

use timelock::{RateChangeProposal, UnstakeRequest};

// ── Storage key constants ────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const PENDING_ADMIN: Symbol = symbol_short!("PEND_ADM");
const INITIALIZED: Symbol = symbol_short!("INIT");
const STAKE_TOKEN: Symbol = symbol_short!("STK_TOK");
const REWARD_TOKEN: Symbol = symbol_short!("RWD_TOK");
const REWARD_RATE: Symbol = symbol_short!("RWD_RATE");
const TOTAL_STAKED: Symbol = symbol_short!("TOT_STK");
const REWARD_PER_TOKEN: Symbol = symbol_short!("RPT");
const LAST_UPDATE: Symbol = symbol_short!("LAST_UPD");
const LOCK_PERIOD: Symbol = symbol_short!("LOCK_PER");
const RATE_DELAY: Symbol = symbol_short!("RATE_DLY");

// Per-user persistent storage uses tuple keys:  (prefix, user_address)
const USER_STAKE: Symbol = symbol_short!("STK");
const USER_RPT_PAID: Symbol = symbol_short!("RPT_PAID");
const USER_EARNED: Symbol = symbol_short!("ERND");
// Records the ledger timestamp of a user's first-ever stake deposit.
// Used by the Governor DAO to compute the time-weighted loyalty multiplier.
const USER_SINCE: Symbol = symbol_short!("SINCE");

// ── Contract errors ──────────────────────────────────────────────────────────

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidInput = 4,
    InsufficientBalance = 5,
    TimelockNotExpired = 6,
    AlreadyWithdrawn = 7,
    RequestNotFound = 8,
    TokensIdentical = 9,
    RateChangeNotReady = 10,
    NoPendingRateChange = 11,
    MultisigRequired = 12,
    MultisigError = 13,
    Paused = 14,
}

// ── Public-facing types (re-exported for test consumers) ─────────────────────

/// Snapshot of a user's staking position returned by `get_staker_info`.
#[contracttype]
#[derive(Clone, Debug)]
pub struct StakerInfo {
    pub staked: i128,
    pub pending_rewards: i128,
}

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct StakingContract;

#[contractimpl]
impl StakingContract {
    fn emit_access_violation(env: &Env, caller: &Address, action: &str, required_permission: &str) {
        events::publish_access_violation(
            env,
            caller.clone(),
            String::from_str(env, action),
            String::from_str(env, required_permission),
        );
    }

    fn unauthorized<T>(
        env: &Env,
        caller: &Address,
        action: &str,
        required_permission: &str,
    ) -> Result<T, ContractError> {
        Self::emit_access_violation(env, caller, action, required_permission);
        Err(ContractError::Unauthorized)
    }

    // ── Initialisation ──────────────────────────────────────────────────────

    /// Bootstrap the contract.
    ///
    /// * `stake_token`  – SAC address of the token users stake.
    /// * `reward_token` – SAC address of the token distributed as rewards.
    /// * `reward_rate`  – tokens emitted **per second** across all stakers.
    /// * `lock_period`  – seconds a withdrawal must wait after `request_unstake`.
    pub fn initialize(
        env: Env,
        admin: Address,
        stake_token: Address,
        reward_token: Address,
        reward_rate: i128,
        lock_period: u64,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(ContractError::AlreadyInitialized);
        }
        if reward_rate < 0 {
            return Err(ContractError::InvalidInput);
        }
        if stake_token == reward_token {
            return Err(ContractError::TokensIdentical);
        }

        let now = env.ledger().timestamp();

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);
        env.storage().instance().set(&STAKE_TOKEN, &stake_token);
        env.storage().instance().set(&REWARD_TOKEN, &reward_token);
        env.storage().instance().set(&REWARD_RATE, &reward_rate);
        env.storage().instance().set(&LAST_UPDATE, &now);
        env.storage().instance().set(&LOCK_PERIOD, &lock_period);
        // TOTAL_STAKED, REWARD_PER_TOKEN, and UNSTK_CTR start at zero;
        // unwrap_or(0) handles absent keys, so no explicit init needed.

        // Bootstrap the initializing admin as SuperAdmin in the tier system
        admin_tiers::set_super_admin(&env, &admin);
        admin_tiers::track_admin(&env, &admin);

        events::publish_initialized(
            &env,
            admin,
            stake_token,
            reward_token,
            reward_rate,
            lock_period,
        );

        Ok(())
    }

    // ── Staking ─────────────────────────────────────────────────────────────

    /// Deposit `amount` stake tokens.
    ///
    /// The global reward accumulator is updated first so the staker does not
    /// retroactively earn rewards on the newly deposited tokens.
    ///
    /// On a user's very first deposit the current timestamp is recorded under
    /// `USER_SINCE` so the Governor DAO can later compute their loyalty age.
    pub fn stake(env: Env, staker: Address, amount: i128) -> Result<(), ContractError> {
        let _guard = common::ReentrancyGuard::new(&env);
        Self::require_not_paused(&env)?;
        Self::require_initialized(&env)?;
        staker.require_auth();

        if amount <= 0 {
            return Err(ContractError::InvalidInput);
        }

        // 1. Flush global accumulator then snapshot for this user.
        Self::update_reward(&env, &staker);

        // 2. Pull tokens from the staker into the contract.
        let stake_token: Address = env
            .storage()
            .instance()
            .get(&STAKE_TOKEN)
            .ok_or(ContractError::NotInitialized)?;
        token::Client::new(&env, &stake_token).transfer(
            &staker,
            env.current_contract_address(),
            &amount,
        );

        // 3. Increase the user's staked balance and the global total.
        let user_stake_key = (USER_STAKE, staker.clone());
        let prev_stake: i128 = env
            .storage()
            .persistent()
            .get(&user_stake_key)
            .unwrap_or(0i128);
        let new_stake = prev_stake.saturating_add(amount);
        env.storage().persistent().set(&user_stake_key, &new_stake);

        let prev_total: i128 = env.storage().instance().get(&TOTAL_STAKED).unwrap_or(0);
        let new_total = prev_total.saturating_add(amount);
        env.storage().instance().set(&TOTAL_STAKED, &new_total);

        // 4. Record the first-stake timestamp for loyalty age tracking.
        //    Only written once; subsequent top-ups do not reset the clock.
        let since_key = (USER_SINCE, staker.clone());
        if !env.storage().persistent().has(&since_key) {
            let now = env.ledger().timestamp();
            env.storage().persistent().set(&since_key, &now);
        }

        events::publish_staked(&env, staker, amount, new_total);

        Ok(())
    }

    // ── Unstaking ───────────────────────────────────────────────────────────

    /// Queue `amount` tokens for withdrawal after the timelock.
    ///
    /// The staked balance is reduced immediately (preventing reward accrual
    /// on the queued amount) but tokens are only returned after the lock
    /// period via `withdraw`.
    pub fn request_unstake(env: Env, staker: Address, amount: i128) -> Result<u64, ContractError> {
        Self::require_not_paused(&env)?;
        Self::require_initialized(&env)?;
        staker.require_auth();

        if amount <= 0 {
            return Err(ContractError::InvalidInput);
        }

        // 1. Flush rewards before reducing stake.
        Self::update_reward(&env, &staker);

        // 2. Verify the user has enough staked.
        let user_stake_key = (USER_STAKE, staker.clone());
        let prev_stake: i128 = env.storage().persistent().get(&user_stake_key).unwrap_or(0);
        if prev_stake < amount {
            return Err(ContractError::InsufficientBalance);
        }

        // 3. Reduce staked balance and global total.
        let new_stake = prev_stake.saturating_sub(amount);
        env.storage().persistent().set(&user_stake_key, &new_stake);

        let prev_total: i128 = env.storage().instance().get(&TOTAL_STAKED).unwrap_or(0);
        let new_total = prev_total.saturating_sub(amount);
        env.storage().instance().set(&TOTAL_STAKED, &new_total);

        // 4. Create the timelock entry.
        let lock_period: u64 = env.storage().instance().get(&LOCK_PERIOD).unwrap_or(0);
        let now = env.ledger().timestamp();
        let unlock_at = now.saturating_add(lock_period);

        let request_id = timelock::next_request_id(&env);
        let request = UnstakeRequest {
            id: request_id,
            staker: staker.clone(),
            amount,
            unlock_at,
            withdrawn: false,
        };
        timelock::store_request(&env, &request);

        events::publish_unstake_requested(&env, request_id, staker, amount, unlock_at);

        Ok(request_id)
    }

    /// Withdraw tokens for a previously queued unstake request.
    ///
    /// Fails with `TimelockNotExpired` if called before `unlock_at`, and
    /// with `AlreadyWithdrawn` on duplicate calls.
    pub fn withdraw(env: Env, staker: Address, request_id: u64) -> Result<(), ContractError> {
        let _guard = common::ReentrancyGuard::new(&env);
        Self::require_initialized(&env)?;
        staker.require_auth();

        let mut request =
            timelock::get_request(&env, request_id).ok_or(ContractError::RequestNotFound)?;

        // Auth: only the original staker may withdraw.
        if request.staker != staker {
            return Self::unauthorized(&env, &staker, "withdraw", "request_owner");
        }
        if request.withdrawn {
            return Err(ContractError::AlreadyWithdrawn);
        }
        if env.ledger().timestamp() < request.unlock_at {
            return Err(ContractError::TimelockNotExpired);
        }

        // Mark as withdrawn before transfer (checks-effects-interactions).
        request.withdrawn = true;
        timelock::store_request(&env, &request);

        // Return tokens to staker.
        let stake_token: Address = env
            .storage()
            .instance()
            .get(&STAKE_TOKEN)
            .ok_or(ContractError::NotInitialized)?;
        token::Client::new(&env, &stake_token).transfer(
            &env.current_contract_address(),
            &staker,
            &request.amount,
        );

        events::publish_withdrawn(&env, request_id, staker, request.amount);

        Ok(())
    }

    // ── Rewards ─────────────────────────────────────────────────────────────

    /// Claim all accumulated rewards for `staker`.
    ///
    /// Rewards are transferred from the contract's reward-token balance.
    /// The contract must hold sufficient reward tokens (funded by the admin).
    pub fn claim_rewards(env: Env, staker: Address) -> Result<i128, ContractError> {
        let _guard = common::ReentrancyGuard::new(&env);
        Self::require_not_paused(&env)?;
        Self::require_initialized(&env)?;
        staker.require_auth();

        // 1. Sync the accumulator.
        Self::update_reward(&env, &staker);

        // 2. Read and reset the user's earned balance.
        let earned_key = (USER_EARNED, staker.clone());
        let earned: i128 = env.storage().persistent().get(&earned_key).unwrap_or(0);

        if earned <= 0 {
            // Nothing to claim — return without reverting.
            return Ok(0);
        }

        env.storage().persistent().set(&earned_key, &0i128);

        // 3. Transfer reward tokens to the staker.
        let reward_token: Address = env
            .storage()
            .instance()
            .get(&REWARD_TOKEN)
            .ok_or(ContractError::NotInitialized)?;
        token::Client::new(&env, &reward_token).transfer(
            &env.current_contract_address(),
            &staker,
            &earned,
        );

        events::publish_reward_claimed(&env, staker, earned);

        Ok(earned)
    }

    // ── View functions ───────────────────────────────────────────────────────

    /// Return the user's current staked balance.
    pub fn get_staked(env: Env, staker: Address) -> i128 {
        let key = (USER_STAKE, staker);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Return real-time pending rewards for a staker without mutating state.
    pub fn get_pending_rewards(env: Env, staker: Address) -> i128 {
        let total_staked: i128 = env.storage().instance().get(&TOTAL_STAKED).unwrap_or(0);
        let reward_rate: i128 = env.storage().instance().get(&REWARD_RATE).unwrap_or(0);
        let stored_rpt: i128 = env.storage().instance().get(&REWARD_PER_TOKEN).unwrap_or(0);
        let last_update: u64 = env.storage().instance().get(&LAST_UPDATE).unwrap_or(0);

        let now = env.ledger().timestamp();
        let elapsed = now.saturating_sub(last_update);
        let current_rpt =
            rewards::compute_reward_per_token(stored_rpt, reward_rate, elapsed, total_staked);

        let staked: i128 = env
            .storage()
            .persistent()
            .get(&(USER_STAKE, staker.clone()))
            .unwrap_or(0);
        let user_rpt_paid: i128 = env
            .storage()
            .persistent()
            .get(&(USER_RPT_PAID, staker.clone()))
            .unwrap_or(0);
        let user_earned: i128 = env
            .storage()
            .persistent()
            .get(&(USER_EARNED, staker))
            .unwrap_or(0);

        rewards::earned(staked, current_rpt, user_rpt_paid, user_earned)
    }

    /// Return the combined staking position for a user.
    ///
    /// Reads persistent storage once for each user key, avoiding the duplicate
    /// reads that calling `get_staked` + `get_pending_rewards` separately would incur.
    pub fn get_staker_info(env: Env, staker: Address) -> StakerInfo {
        let total_staked: i128 = env.storage().instance().get(&TOTAL_STAKED).unwrap_or(0);
        let reward_rate: i128 = env.storage().instance().get(&REWARD_RATE).unwrap_or(0);
        let stored_rpt: i128 = env.storage().instance().get(&REWARD_PER_TOKEN).unwrap_or(0);
        let last_update: u64 = env.storage().instance().get(&LAST_UPDATE).unwrap_or(0);

        let elapsed = env.ledger().timestamp().saturating_sub(last_update);
        let current_rpt =
            rewards::compute_reward_per_token(stored_rpt, reward_rate, elapsed, total_staked);

        let staked: i128 = env
            .storage()
            .persistent()
            .get(&(USER_STAKE, staker.clone()))
            .unwrap_or(0);
        let user_rpt_paid: i128 = env
            .storage()
            .persistent()
            .get(&(USER_RPT_PAID, staker.clone()))
            .unwrap_or(0);
        let user_earned: i128 = env
            .storage()
            .persistent()
            .get(&(USER_EARNED, staker))
            .unwrap_or(0);

        StakerInfo {
            staked,
            pending_rewards: rewards::earned(staked, current_rpt, user_rpt_paid, user_earned),
        }
    }

    /// Return the current global reward rate (tokens per second).
    pub fn get_reward_rate(env: Env) -> i128 {
        env.storage().instance().get(&REWARD_RATE).unwrap_or(0)
    }

    /// Return the sum of all currently staked tokens.
    pub fn get_total_staked(env: Env) -> i128 {
        env.storage().instance().get(&TOTAL_STAKED).unwrap_or(0)
    }

    /// Return the configured unstake lock period in seconds.
    pub fn get_lock_period(env: Env) -> u64 {
        env.storage().instance().get(&LOCK_PERIOD).unwrap_or(0)
    }

    /// Return the configured rate-change delay in seconds.
    pub fn get_rate_change_delay(env: Env) -> u64 {
        env.storage().instance().get(&RATE_DELAY).unwrap_or(0)
    }

    /// Return the pending rate-change proposal, if any.
    pub fn get_pending_rate_change(env: Env) -> Result<RateChangeProposal, ContractError> {
        timelock::get_rate_proposal(&env).ok_or(ContractError::NoPendingRateChange)
    }

    /// Return the details of a specific unstake request.
    pub fn get_unstake_request(env: Env, request_id: u64) -> Result<UnstakeRequest, ContractError> {
        timelock::get_request(&env, request_id).ok_or(ContractError::RequestNotFound)
    }

    /// Return the ledger timestamp when `staker` made their first deposit.
    ///
    /// Returns `0` if the address has never staked.
    pub fn get_stake_since(env: Env, staker: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&(USER_SINCE, staker))
            .unwrap_or(0u64)
    }

    /// Return how many seconds `staker` has been continuously staking.
    ///
    /// Used by the Governor DAO to compute the time-weighted loyalty multiplier:
    /// ```text
    /// loyalty_mult = 1.0 + min(stake_age_days / 365, 1.0)   // up to 2×
    /// ```
    /// Returns `0` if the address has never staked.
    pub fn get_stake_age(env: Env, staker: Address) -> u64 {
        let since: u64 = env
            .storage()
            .persistent()
            .get(&(USER_SINCE, staker))
            .unwrap_or(0u64);
        if since == 0 {
            return 0;
        }
        env.ledger().timestamp().saturating_sub(since)
    }

    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&INITIALIZED)
    }

    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)
    }

    // ── Admin transfer (two-step) ──────────────────────────────────────────

    /// Propose a new admin address. Only the current admin can call this.
    /// The new admin must call `accept_admin` to complete the transfer.
    pub fn propose_admin(
        env: Env,
        current_admin: Address,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        current_admin.require_auth();
        Self::require_admin(&env, &current_admin, "propose_admin")?;

        env.storage().instance().set(&PENDING_ADMIN, &new_admin);

        events::publish_admin_transfer_proposed(&env, current_admin, new_admin);

        Ok(())
    }

    /// Accept the pending admin transfer. Only the proposed new admin can call this.
    /// Completes the two-step admin transfer process.
    pub fn accept_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        new_admin.require_auth();

        let pending: Address = env
            .storage()
            .instance()
            .get(&PENDING_ADMIN)
            .ok_or(ContractError::InvalidInput)?;

        if new_admin != pending {
            return Self::unauthorized(&env, &new_admin, "accept_admin", "pending_admin");
        }

        let old_admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)?;

        env.storage().instance().set(&ADMIN, &new_admin);
        env.storage().instance().remove(&PENDING_ADMIN);

        events::publish_admin_transfer_accepted(&env, old_admin, new_admin);

        Ok(())
    }

    /// Cancel a pending admin transfer. Only the current admin can call this.
    pub fn cancel_admin_transfer(env: Env, current_admin: Address) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        current_admin.require_auth();
        Self::require_admin(&env, &current_admin, "cancel_admin_transfer")?;

        let pending: Address = env
            .storage()
            .instance()
            .get(&PENDING_ADMIN)
            .ok_or(ContractError::InvalidInput)?;

        env.storage().instance().remove(&PENDING_ADMIN);

        events::publish_admin_transfer_cancelled(&env, current_admin, pending);

        Ok(())
    }

    /// Get the pending admin address, if any.
    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&PENDING_ADMIN)
    }

    // ── Multisig management ──────────────────────────────────────────────────

    /// Configure M-of-N multisig for admin operations.
    ///
    /// Only the current admin can call this.  Once configured, critical
    /// admin operations (`set_reward_rate`, `set_lock_period`) require
    /// a fully-approved multisig proposal.
    pub fn configure_multisig(
        env: Env,
        caller: Address,
        signers: soroban_sdk::Vec<Address>,
        threshold: u32,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        Self::require_admin(&env, &caller, "configure_multisig")?;

        multisig::configure(&env, signers, threshold).map_err(|_| ContractError::InvalidInput)
    }

    /// Create a multisig proposal for an admin action.
    ///
    /// `action` is a short tag (e.g. `symbol_short!("SET_RATE")`).
    /// `data_hash` is a SHA-256 hash of the action parameters so
    /// approvers can verify intent.
    pub fn propose_admin_action(
        env: Env,
        proposer: Address,
        action: Symbol,
        data_hash: BytesN<32>,
    ) -> Result<u64, ContractError> {
        Self::require_initialized(&env)?;
        proposer.require_auth();

        multisig::propose(&env, &proposer, action, data_hash)
            .map_err(|_| ContractError::MultisigError)
    }

    /// Approve a pending multisig proposal.
    pub fn approve_admin_action(
        env: Env,
        approver: Address,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        approver.require_auth();

        multisig::approve(&env, &approver, proposal_id).map_err(|_| ContractError::MultisigError)
    }

    /// Return the current multisig configuration, if any.
    pub fn get_multisig_config(env: Env) -> Option<multisig::MultisigConfig> {
        multisig::get_config(&env)
    }

    /// Return a pending proposal by ID.
    pub fn get_proposal(env: Env, proposal_id: u64) -> Option<multisig::Proposal> {
        multisig::get_proposal(&env, proposal_id)
    }

    // ── Admin functions ──────────────────────────────────────────────────────

    /// Propose a reward-rate change.
    ///
    /// The global accumulator is flushed at the current rate *before* the
    /// rate changes, so existing stakers never lose or gain rewards
    /// retroactively.
    ///
    /// When multisig is configured, `proposal_id` must reference a fully
    /// approved proposal with action `"SET_RATE"`.  When multisig is not
    /// configured, pass `0` and the legacy single-admin path is used.
    pub fn set_reward_rate(
        env: Env,
        caller: Address,
        new_rate: i128,
        _proposal_id: u64,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        Self::require_admin_tier(&env, &caller, &AdminTier::Contract, "set_reward_rate")?;

        if new_rate < 0 {
            return Err(ContractError::InvalidInput);
        }

        // If multisig is configured, require an executable proposal and consume it.
        if !multisig::is_legacy_admin_allowed(&env) {
            if proposal_id == 0 {
                return Err(ContractError::MultisigRequired);
            }
            let proposal =
                multisig::get_proposal(&env, proposal_id).ok_or(ContractError::MultisigRequired)?;
            if proposal.action != symbol_short!("RWD_RATE")
                || !multisig::is_executable(&env, proposal_id)
            {
                return Err(ContractError::MultisigRequired);
            }
            multisig::mark_executed(&env, proposal_id).map_err(|_| ContractError::MultisigError)?;
        }

        let delay: u64 = env.storage().instance().get(&RATE_DELAY).unwrap_or(0);

        if delay == 0 {
            // No delay configured — apply immediately.
            Self::update_global_reward(&env);
            env.storage().instance().set(&REWARD_RATE, &new_rate);
            events::publish_reward_rate_set(&env, new_rate);
        } else {
            let effective_at = env.ledger().timestamp().saturating_add(delay);
            let proposal = RateChangeProposal {
                new_rate,
                effective_at,
            };
            timelock::store_rate_proposal(&env, &proposal);
            events::publish_reward_rate_proposed(&env, new_rate, effective_at);
        }

        Ok(())
    }

    /// Apply a previously proposed reward-rate change after the delay.
    pub fn apply_reward_rate(env: Env, caller: Address) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        Self::require_admin(&env, &caller, "apply_reward_rate")?;

        let proposal =
            timelock::get_rate_proposal(&env).ok_or(ContractError::NoPendingRateChange)?;

        if env.ledger().timestamp() < proposal.effective_at {
            return Err(ContractError::RateChangeNotReady);
        }

        Self::update_global_reward(&env);
        env.storage()
            .instance()
            .set(&REWARD_RATE, &proposal.new_rate);
        timelock::clear_rate_proposal(&env);

        events::publish_reward_rate_applied(&env, proposal.new_rate);

        Ok(())
    }

    /// Set the mandatory delay (in seconds) for reward-rate changes.
    pub fn set_rate_change_delay(
        env: Env,
        caller: Address,
        delay: u64,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        Self::require_admin(&env, &caller, "set_rate_change_delay")?;

        env.storage().instance().set(&RATE_DELAY, &delay);

        events::publish_rate_change_delay_set(&env, delay);

        Ok(())
    }

    /// Update the unstake lock period (affects only *future* requests).
    ///
    /// When multisig is configured, `proposal_id` must reference a fully
    /// approved proposal with action `"SET_LOCK"`.  When multisig is not
    /// configured, pass `0` and the legacy single-admin path is used.
    pub fn set_lock_period(
        env: Env,
        caller: Address,
        new_period: u64,
        _proposal_id: u64,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        Self::require_admin_tier(&env, &caller, &AdminTier::Contract, "set_lock_period")?;

        if !multisig::is_legacy_admin_allowed(&env) {
            if proposal_id == 0 {
                return Err(ContractError::MultisigRequired);
            }
            let proposal =
                multisig::get_proposal(&env, proposal_id).ok_or(ContractError::MultisigRequired)?;
            if proposal.action != symbol_short!("SET_LOCK")
                || !multisig::is_executable(&env, proposal_id)
            {
                return Err(ContractError::MultisigRequired);
            }
            multisig::mark_executed(&env, proposal_id).map_err(|_| ContractError::MultisigError)?;
        }

        env.storage().instance().set(&LOCK_PERIOD, &new_period);

        events::publish_lock_period_set(&env, new_period);

        Ok(())
    }

    // ── Admin tier management ────────────────────────────────────────────────

    /// Promotes or assigns a target address to the specified admin tier.
    ///
    /// Only a `SuperAdmin` may call this.
    pub fn promote_admin(
        env: Env,
        caller: Address,
        target: Address,
        tier: AdminTier,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        if !admin_tiers::promote_admin(&env, &caller, &target, tier) {
            return Self::unauthorized(&env, &caller, "promote_admin", "admin_tier:SuperAdmin");
        }
        admin_tiers::track_admin(&env, &target);
        Ok(())
    }

    /// Removes the admin tier from the target address entirely.
    ///
    /// Only a `SuperAdmin` may call this.
    pub fn demote_admin(env: Env, caller: Address, target: Address) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        if !admin_tiers::demote_admin(&env, &caller, &target) {
            return Self::unauthorized(&env, &caller, "demote_admin", "admin_tier:SuperAdmin");
        }
        admin_tiers::untrack_admin(&env, &target);
        Ok(())
    }

    /// Returns the admin tier of the given address, if any.
    pub fn get_admin_tier(env: Env, admin: Address) -> Option<AdminTier> {
        admin_tiers::get_admin_tier(&env, &admin)
    }

    // ── Pause management ──────────────────────────────────────────────────

    /// Pause all state-mutating operations.
    ///
    /// Requires at least `ContractAdmin` tier, or legacy admin.
    pub fn pause(env: Env, caller: Address) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        Self::require_admin_tier(&env, &caller, &AdminTier::ContractAdmin, "pause")?;
        common::pausable::pause(&env, &caller);
        Ok(())
    }

    /// Resume all state-mutating operations.
    ///
    /// Requires at least `ContractAdmin` tier, or legacy admin.
    pub fn unpause(env: Env, caller: Address) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        Self::require_admin_tier(&env, &caller, &AdminTier::ContractAdmin, "unpause")?;
        common::pausable::unpause(&env, &caller);
        Ok(())
    }

    /// Returns whether the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        common::pausable::is_paused(&env)
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    /// Guard: revert if the contract is paused.
    fn require_not_paused(env: &Env) -> Result<(), ContractError> {
        common::pausable::require_not_paused(env).map_err(|_| ContractError::Paused)
    }

    /// Guard: revert if the contract is not yet initialized.
    fn require_initialized(env: &Env) -> Result<(), ContractError> {
        if !env.storage().instance().has(&INITIALIZED) {
            return Err(ContractError::NotInitialized);
        }
        Ok(())
    }

    /// Guard: revert if `caller` is not the stored admin.
    /// Kept for backward compatibility.
    fn require_admin(env: &Env, caller: &Address, action: &str) -> Result<(), ContractError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)?;
        if *caller != admin {
            return Self::unauthorized(env, caller, action, "legacy_admin");
        }
        Ok(())
    }

    /// Guard: revert if `caller` does not hold at least `min_tier`.
    /// Falls back to the legacy ADMIN check for backward compatibility.
    fn require_admin_tier(
        env: &Env,
        caller: &Address,
        min_tier: &AdminTier,
        action: &str,
    ) -> Result<(), ContractError> {
        // First check the tiered system
        if admin_tiers::require_tier(env, caller, min_tier) {
            return Ok(());
        }
        // Fall back to legacy admin check
        Self::require_admin(env, caller, action)
    }

    /// Flush the global reward-per-token accumulator without touching any
    /// user-specific state.  Called at the start of every admin mutation that
    /// changes the emission rate.
    fn update_global_reward(env: &Env) {
        let total_staked: i128 = env.storage().instance().get(&TOTAL_STAKED).unwrap_or(0);
        let reward_rate: i128 = env.storage().instance().get(&REWARD_RATE).unwrap_or(0);
        let stored_rpt: i128 = env.storage().instance().get(&REWARD_PER_TOKEN).unwrap_or(0);
        let last_update: u64 = env.storage().instance().get(&LAST_UPDATE).unwrap_or(0);

        let now = env.ledger().timestamp();
        let elapsed = now.saturating_sub(last_update);

        let new_rpt =
            rewards::compute_reward_per_token(stored_rpt, reward_rate, elapsed, total_staked);

        env.storage().instance().set(&REWARD_PER_TOKEN, &new_rpt);
        env.storage().instance().set(&LAST_UPDATE, &now);
    }

    /// Full per-user reward flush.
    ///
    /// 1. Update the global RPT accumulator.
    /// 2. Compute everything the user has earned since their last snapshot.
    /// 3. Store the updated snapshot so the user's next interaction starts fresh.
    fn update_reward(env: &Env, user: &Address) {
        Self::update_global_reward(env);

        let current_rpt: i128 = env.storage().instance().get(&REWARD_PER_TOKEN).unwrap_or(0);

        let staked: i128 = env
            .storage()
            .persistent()
            .get(&(USER_STAKE, user.clone()))
            .unwrap_or(0);
        let user_rpt_paid: i128 = env
            .storage()
            .persistent()
            .get(&(USER_RPT_PAID, user.clone()))
            .unwrap_or(0);
        let user_earned: i128 = env
            .storage()
            .persistent()
            .get(&(USER_EARNED, user.clone()))
            .unwrap_or(0);

        let new_earned = rewards::earned(staked, current_rpt, user_rpt_paid, user_earned);

        env.storage()
            .persistent()
            .set(&(USER_EARNED, user.clone()), &new_earned);
        env.storage()
            .persistent()
            .set(&(USER_RPT_PAID, user.clone()), &current_rpt);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test;

#[cfg(test)]
mod test_admin_tiers;

#[cfg(test)]
mod test_multisig;
