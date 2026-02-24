#![no_std]

pub mod events;
pub mod rewards;
pub mod timelock;

use common::admin_tiers::{self, AdminTier};
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol,
};

use timelock::UnstakeRequest;

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

// Per-user persistent storage uses tuple keys:  (prefix, user_address)
const USER_STAKE: Symbol = symbol_short!("STK");
const USER_RPT_PAID: Symbol = symbol_short!("RPT_PAID");
const USER_EARNED: Symbol = symbol_short!("ERND");

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
    pub fn stake(env: Env, staker: Address, amount: i128) -> Result<(), ContractError> {
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
            &env.current_contract_address(),
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
        Self::require_initialized(&env)?;
        staker.require_auth();

        let mut request =
            timelock::get_request(&env, request_id).ok_or(ContractError::RequestNotFound)?;

        // Auth: only the original staker may withdraw.
        if request.staker != staker {
            return Err(ContractError::Unauthorized);
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

    /// Return the details of a specific unstake request.
    pub fn get_unstake_request(env: Env, request_id: u64) -> Result<UnstakeRequest, ContractError> {
        timelock::get_request(&env, request_id).ok_or(ContractError::RequestNotFound)
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
        Self::require_admin(&env, &current_admin)?;

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
            return Err(ContractError::Unauthorized);
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
    pub fn cancel_admin_transfer(
        env: Env,
        current_admin: Address,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        current_admin.require_auth();
        Self::require_admin(&env, &current_admin)?;

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

    // ── Admin functions ──────────────────────────────────────────────────────

    /// Update the reward emission rate.
    ///
    /// The global accumulator is flushed at the current rate *before* the
    /// rate changes, so existing stakers never lose or gain rewards
    /// retroactively.
    ///
    /// Requires at least `ContractAdmin` tier.
    pub fn set_reward_rate(env: Env, caller: Address, new_rate: i128) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        Self::require_admin_tier(&env, &caller, &AdminTier::ContractAdmin)?;

        if new_rate < 0 {
            return Err(ContractError::InvalidInput);
        }

        // Flush accumulator at old rate before changing.
        Self::update_global_reward(&env);

        env.storage().instance().set(&REWARD_RATE, &new_rate);

        events::publish_reward_rate_set(&env, new_rate);

        Ok(())
    }

    /// Update the unstake lock period (affects only *future* requests).
    ///
    /// Requires at least `ContractAdmin` tier.
    pub fn set_lock_period(
        env: Env,
        caller: Address,
        new_period: u64,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        Self::require_admin_tier(&env, &caller, &AdminTier::ContractAdmin)?;

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
            return Err(ContractError::Unauthorized);
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
            return Err(ContractError::Unauthorized);
        }
        admin_tiers::untrack_admin(&env, &target);
        Ok(())
    }

    /// Returns the admin tier of the given address, if any.
    pub fn get_admin_tier(env: Env, admin: Address) -> Option<AdminTier> {
        admin_tiers::get_admin_tier(&env, &admin)
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    /// Guard: revert if the contract is not yet initialized.
    fn require_initialized(env: &Env) -> Result<(), ContractError> {
        if !env.storage().instance().has(&INITIALIZED) {
            return Err(ContractError::NotInitialized);
        }
        Ok(())
    }

    /// Guard: revert if `caller` is not the stored admin.
    /// Kept for backward compatibility.
    fn require_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)?;
        if *caller != admin {
            return Err(ContractError::Unauthorized);
        }
        Ok(())
    }

    /// Guard: revert if `caller` does not hold at least `min_tier`.
    /// Falls back to the legacy ADMIN check for backward compatibility.
    fn require_admin_tier(
        env: &Env,
        caller: &Address,
        min_tier: &AdminTier,
    ) -> Result<(), ContractError> {
        // First check the tiered system
        if admin_tiers::require_tier(env, caller, min_tier) {
            return Ok(());
        }
        // Fall back to legacy admin check
        Self::require_admin(env, caller)
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
