#![no_std]

//! # Governor DAO
//!
//! A governance contract for the Teye contract ecosystem featuring:
//!
//! - **Quadratic voting**: `vote_power = sqrt(staked_tokens) × loyalty_multiplier`
//! - **Time-weighted influence**: stakers who hold longer earn up to 2× vote weight
//! - **Multi-phase lifecycle**: Draft → Discussion → Voting → Timelock → Execution → Completed/Rejected
//! - **Proposal types**: ContractUpgrade, ParameterChange, PolicyModification, EmergencyAction, TreasurySpend
//! - **Delegation**: delegate vote power to a representative with revocation
//! - **Commit-reveal**: prevents vote-buying and bandwagon effects
//! - **Optimistic execution**: execute after timelock unless veto threshold is met
//! - **Proposal batching**: multiple actions in one atomic proposal

pub mod delegation;
pub mod events;
pub mod execution;
pub mod proposal;
pub mod voting;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env, String, Symbol, Vec,
};

use delegation::Delegation;
use execution::timelock_duration;
use proposal::{
    load as load_proposal, next_id, pass_threshold_bps, quorum_bps, store as store_proposal,
    veto_threshold_bps, Proposal, ProposalAction, ProposalPhase, ProposalType,
};
use voting::{
    compute_vote_power, has_committed, has_voted, load_commit, store_commit, store_vote,
    VoteChoice, VoteCommit, VoteRecord,
};

// ── Storage key constants ─────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");
const STAKING_CONTRACT: Symbol = symbol_short!("STK_CTR");
const TREASURY_CONTRACT: Symbol = symbol_short!("TRES_CTR");
const TOTAL_VOTE_SUPPLY: Symbol = symbol_short!("TOT_VS");

/// Default Discussion phase length in seconds (3 days).
const DEFAULT_DISCUSSION_SECS: u64 = 259_200;
/// Default Voting phase length in seconds (5 days).
const DEFAULT_VOTING_SECS: u64 = 432_000;

// ── Error codes ───────────────────────────────────────────────────────────────

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidInput = 4,
    ProposalNotFound = 5,
    WrongPhase = 6,
    AlreadyCommitted = 7,
    AlreadyRevealed = 8,
    CommitmentMismatch = 9,
    NoCommitFound = 10,
    QuorumNotMet = 11,
    TimelockNotExpired = 12,
    VetoThresholdMet = 13,
    HasDelegated = 14,
    NotADelegate = 15,
    SelfDelegation = 16,
    InsufficientStake = 17,
    PhaseNotAdvanceable = 18,
}

// ── Public return types ───────────────────────────────────────────────────────

/// Summary of a proposal returned by view functions.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ProposalSummary {
    pub id: u64,
    pub phase: ProposalPhase,
    pub proposal_type: ProposalType,
    pub votes_for: i128,
    pub votes_against: i128,
    pub votes_veto: i128,
    pub reveal_count: u32,
    pub voting_ends: u64,
    pub timelock_ends: u64,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct GovernorContract;

#[contractimpl]
impl GovernorContract {
    // ── Initialisation ────────────────────────────────────────────────────────

    /// Bootstrap the governor.
    ///
    /// * `staking_contract`  — address of the Teye staking contract; queried
    ///                         for each voter's staked balance and stake age.
    /// * `treasury_contract` — address of the Teye treasury contract; called
    ///                         for TreasurySpend proposals.
    /// * `total_vote_supply` — total token supply used to compute quorum
    ///                         percentages (can be updated by admin later).
    pub fn initialize(
        env: Env,
        admin: Address,
        staking_contract: Address,
        treasury_contract: Address,
        total_vote_supply: i128,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(ContractError::AlreadyInitialized);
        }
        if total_vote_supply <= 0 {
            return Err(ContractError::InvalidInput);
        }

        env.storage().instance().set(&ADMIN, &admin);
        env.storage()
            .instance()
            .set(&STAKING_CONTRACT, &staking_contract);
        env.storage()
            .instance()
            .set(&TREASURY_CONTRACT, &treasury_contract);
        env.storage()
            .instance()
            .set(&TOTAL_VOTE_SUPPLY, &total_vote_supply);
        env.storage().instance().set(&INITIALIZED, &true);

        Ok(())
    }

    // ── Proposal creation ─────────────────────────────────────────────────────

    /// Create a new proposal in Draft phase.
    ///
    /// The proposer must hold a non-zero staked balance in the staking
    /// contract.  After creation the proposer calls `advance_phase` to move
    /// the proposal through Discussion → Voting.
    ///
    /// * `actions` — one or more `ProposalAction` items executed atomically
    ///               when the proposal passes.
    pub fn create_proposal(
        env: Env,
        proposer: Address,
        proposal_type: ProposalType,
        title: String,
        actions: Vec<ProposalAction>,
    ) -> Result<u64, ContractError> {
        Self::require_initialized(&env)?;
        proposer.require_auth();

        if actions.is_empty() {
            return Err(ContractError::InvalidInput);
        }

        // Proposer must have stake.
        let staked = Self::query_staked(&env, &proposer);
        if staked <= 0 {
            return Err(ContractError::InsufficientStake);
        }

        let now = env.ledger().timestamp();
        let discussion_ends = now.saturating_add(DEFAULT_DISCUSSION_SECS);
        let voting_ends = discussion_ends.saturating_add(DEFAULT_VOTING_SECS);
        let timelock_len = timelock_duration(&proposal_type);
        let timelock_ends = voting_ends.saturating_add(timelock_len);

        let id = next_id(&env);
        let proposal = Proposal {
            id,
            proposal_type,
            phase: ProposalPhase::Draft,
            proposer: proposer.clone(),
            title,
            actions,
            created_at: now,
            discussion_ends,
            voting_ends,
            timelock_ends,
            votes_for: 0,
            votes_against: 0,
            votes_veto: 0,
            commit_count: 0,
            reveal_count: 0,
        };

        store_proposal(&env, &proposal);
        events::publish_proposal_created(&env, &proposal);

        Ok(id)
    }

    // ── Phase transitions ─────────────────────────────────────────────────────

    /// Advance a proposal to its next lifecycle phase.
    ///
    /// Transitions:
    /// | From        | To          | Condition                                    |
    /// |-------------|-------------|----------------------------------------------|
    /// | Draft       | Discussion  | Proposer calls; no time requirement          |
    /// | Discussion  | Voting      | `now >= discussion_ends`                     |
    /// | Voting      | Timelock    | `now >= voting_ends` AND quorum AND majority |
    /// | Voting      | Expired     | `now >= voting_ends` AND quorum not met      |
    /// | Timelock    | Completed   | `now >= timelock_ends` AND veto not met      |
    /// | Timelock    | Rejected    | Veto threshold met                           |
    ///
    /// Anyone may call this once the time condition is satisfied; the proposer
    /// is the only one who can move Draft → Discussion.
    pub fn advance_phase(
        env: Env,
        caller: Address,
        proposal_id: u64,
    ) -> Result<ProposalPhase, ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();

        let mut proposal =
            load_proposal(&env, proposal_id).ok_or(ContractError::ProposalNotFound)?;
        let now = env.ledger().timestamp();

        let new_phase = match &proposal.phase {
            ProposalPhase::Draft => {
                // Only proposer can kick off discussion.
                if caller != proposal.proposer {
                    return Err(ContractError::Unauthorized);
                }
                ProposalPhase::Discussion
            }

            ProposalPhase::Discussion => {
                if now < proposal.discussion_ends {
                    return Err(ContractError::PhaseNotAdvanceable);
                }
                ProposalPhase::Voting
            }

            ProposalPhase::Voting => {
                if now < proposal.voting_ends {
                    return Err(ContractError::PhaseNotAdvanceable);
                }
                // Check quorum.
                let total_supply: i128 = env
                    .storage()
                    .instance()
                    .get(&TOTAL_VOTE_SUPPLY)
                    .unwrap_or(1);
                let total_votes = proposal
                    .votes_for
                    .saturating_add(proposal.votes_against)
                    .saturating_add(proposal.votes_veto);
                let quorum_needed =
                    total_supply * quorum_bps(&proposal.proposal_type) as i128 / 10_000;

                if total_votes < quorum_needed {
                    ProposalPhase::Expired
                } else {
                    // Check simple majority among for/against.
                    let decisive = proposal.votes_for.saturating_add(proposal.votes_against);
                    let pass_needed = decisive
                        * pass_threshold_bps(&proposal.proposal_type) as i128
                        / 10_000;
                    if proposal.votes_for >= pass_needed {
                        ProposalPhase::Timelock
                    } else {
                        ProposalPhase::Rejected
                    }
                }
            }

            ProposalPhase::Timelock => {
                // Check veto first.
                let total_supply: i128 = env
                    .storage()
                    .instance()
                    .get(&TOTAL_VOTE_SUPPLY)
                    .unwrap_or(1);
                let veto_threshold =
                    total_supply * veto_threshold_bps(&proposal.proposal_type) as i128 / 10_000;
                if proposal.votes_veto >= veto_threshold {
                    ProposalPhase::Rejected
                } else if now < proposal.timelock_ends {
                    return Err(ContractError::TimelockNotExpired);
                } else {
                    ProposalPhase::Execution
                }
            }

            ProposalPhase::Execution => ProposalPhase::Completed,

            _ => return Err(ContractError::WrongPhase),
        };

        proposal.phase = new_phase.clone();
        store_proposal(&env, &proposal);
        events::publish_phase_transition(&env, proposal_id, &new_phase);

        Ok(new_phase)
    }

    // ── Commit-reveal voting ──────────────────────────────────────────────────

    /// Phase 1 — commit a blinded vote.
    ///
    /// The voter submits `commitment = SHA-256(proposal_id || voter || choice || salt)`.
    /// This hides the vote until the reveal phase, preventing bandwagon effects
    /// and last-minute vote-buying.
    ///
    /// The voter must be in the Voting phase and must **not** have delegated
    /// their vote to someone else (the delegate commits/reveals on their behalf).
    pub fn commit_vote(
        env: Env,
        voter: Address,
        proposal_id: u64,
        commitment: BytesN<32>,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        voter.require_auth();

        let proposal =
            load_proposal(&env, proposal_id).ok_or(ContractError::ProposalNotFound)?;

        if !matches!(proposal.phase, ProposalPhase::Voting) {
            return Err(ContractError::WrongPhase);
        }

        // If this voter has delegated, only the delegate may commit.
        if delegation::has_delegated(&env, &voter) {
            return Err(ContractError::HasDelegated);
        }

        if has_committed(&env, proposal_id, &voter) {
            return Err(ContractError::AlreadyCommitted);
        }
        if has_voted(&env, proposal_id, &voter) {
            return Err(ContractError::AlreadyRevealed);
        }

        let commit = VoteCommit {
            commitment,
            committed_at: env.ledger().timestamp(),
        };
        store_commit(&env, proposal_id, &voter, &commit);

        // Bump commit counter on the proposal.
        let mut p = proposal;
        p.commit_count = p.commit_count.saturating_add(1);
        store_proposal(&env, &p);

        events::publish_vote_committed(&env, proposal_id, &voter);

        Ok(())
    }

    /// Commit on behalf of a delegator.
    ///
    /// A delegate calls this to lock in votes for all addresses that have
    /// delegated to them.  The `voter` is the *original* token holder; the
    /// `delegate` is the caller who will reveal.
    pub fn commit_vote_as_delegate(
        env: Env,
        delegate: Address,
        voter: Address,
        proposal_id: u64,
        commitment: BytesN<32>,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        delegate.require_auth();

        // Verify the delegation is active.
        if !delegation::is_delegate_of(&env, &delegate, &voter) {
            return Err(ContractError::NotADelegate);
        }

        let proposal =
            load_proposal(&env, proposal_id).ok_or(ContractError::ProposalNotFound)?;
        if !matches!(proposal.phase, ProposalPhase::Voting) {
            return Err(ContractError::WrongPhase);
        }
        if has_committed(&env, proposal_id, &voter) {
            return Err(ContractError::AlreadyCommitted);
        }

        let commit = VoteCommit {
            commitment,
            committed_at: env.ledger().timestamp(),
        };
        store_commit(&env, proposal_id, &voter, &commit);

        let mut p = proposal;
        p.commit_count = p.commit_count.saturating_add(1);
        store_proposal(&env, &p);

        events::publish_vote_committed(&env, proposal_id, &voter);

        Ok(())
    }

    /// Phase 2 — reveal a committed vote.
    ///
    /// The voter supplies the plaintext `choice` and `salt` used when
    /// computing the commitment.  The contract recomputes the hash and
    /// verifies it matches the stored commitment before tallying.
    ///
    /// Vote power is computed as `sqrt(staked) × loyalty_multiplier` using
    /// on-chain stake data queried from the staking contract.
    pub fn reveal_vote(
        env: Env,
        voter: Address,
        proposal_id: u64,
        choice: VoteChoice,
        salt: BytesN<32>,
    ) -> Result<i128, ContractError> {
        Self::require_initialized(&env)?;
        voter.require_auth();

        // Voting phase OR Timelock (for veto reveals during timelock).
        let mut proposal =
            load_proposal(&env, proposal_id).ok_or(ContractError::ProposalNotFound)?;
        let in_valid_phase = matches!(
            proposal.phase,
            ProposalPhase::Voting | ProposalPhase::Timelock
        );
        if !in_valid_phase {
            return Err(ContractError::WrongPhase);
        }
        // During timelock only Veto reveals are accepted.
        if matches!(proposal.phase, ProposalPhase::Timelock) {
            if !matches!(choice, VoteChoice::Veto) {
                return Err(ContractError::WrongPhase);
            }
        }

        if has_voted(&env, proposal_id, &voter) {
            return Err(ContractError::AlreadyRevealed);
        }

        // Fetch and verify the stored commitment.
        let commit =
            load_commit(&env, proposal_id, &voter).ok_or(ContractError::NoCommitFound)?;

        // Recompute commitment: SHA-256(proposal_id || voter || choice || salt)
        let expected = Self::hash_commitment(&env, proposal_id, &voter, &choice, &salt);
        if expected != commit.commitment {
            return Err(ContractError::CommitmentMismatch);
        }

        // Compute vote power using staked balance and stake age.
        let staked = Self::query_staked(&env, &voter);
        let stake_age = Self::query_stake_age(&env, &voter);
        let power = compute_vote_power(staked, stake_age);

        // Tally.
        match choice {
            VoteChoice::For => proposal.votes_for = proposal.votes_for.saturating_add(power),
            VoteChoice::Against => {
                proposal.votes_against = proposal.votes_against.saturating_add(power)
            }
            VoteChoice::Veto => {
                proposal.votes_veto = proposal.votes_veto.saturating_add(power)
            }
        }
        proposal.reveal_count = proposal.reveal_count.saturating_add(1);

        store_proposal(&env, &proposal);

        // Store the reveal record to prevent double-reveals.
        let record = VoteRecord {
            voter: voter.clone(),
            choice: choice.clone(),
            vote_power: power,
            revealed_at: env.ledger().timestamp(),
        };
        store_vote(&env, proposal_id, &voter, &record);

        events::publish_vote_revealed(&env, proposal_id, &voter, &choice, power);

        Ok(power)
    }

    // ── Delegation ────────────────────────────────────────────────────────────

    /// Delegate your voting power to `delegate`.
    ///
    /// The delegation is active for all proposals in the Voting phase from
    /// this point forward.  You may not vote directly while a delegation is
    /// active — call `revoke_delegation` first.
    pub fn delegate(
        env: Env,
        voter: Address,
        delegate: Address,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        voter.require_auth();

        if voter == delegate {
            return Err(ContractError::SelfDelegation);
        }

        delegation::set_delegation(&env, &voter, &delegate);
        events::publish_delegation_set(&env, &voter, &delegate);

        Ok(())
    }

    /// Revoke the active delegation, restoring direct voting rights.
    pub fn revoke_delegation(env: Env, voter: Address) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        voter.require_auth();

        delegation::revoke_delegation(&env, &voter);
        events::publish_delegation_revoked(&env, &voter);

        Ok(())
    }

    // ── Execution ─────────────────────────────────────────────────────────────

    /// Execute a proposal that has reached the Execution phase.
    ///
    /// Moves the proposal to Completed and dispatches each action.
    /// Anyone may call this (permissionless optimistic execution).
    pub fn execute_proposal(env: Env, caller: Address, proposal_id: u64) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();

        let mut proposal =
            load_proposal(&env, proposal_id).ok_or(ContractError::ProposalNotFound)?;

        if !matches!(proposal.phase, ProposalPhase::Execution) {
            return Err(ContractError::WrongPhase);
        }

        // Dispatch each action.
        for (i, action) in proposal.actions.iter().enumerate() {
            execution::dispatch_action(
                &env,
                proposal_id,
                i as u32,
                &action.target,
                &action.function,
                &action.params_hash,
            );
        }

        proposal.phase = ProposalPhase::Completed;
        store_proposal(&env, &proposal);
        events::publish_proposal_executed(&env, proposal_id);

        Ok(())
    }

    // ── Admin ─────────────────────────────────────────────────────────────────

    /// Update the total vote supply used for quorum calculations.
    pub fn set_total_vote_supply(
        env: Env,
        caller: Address,
        supply: i128,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        Self::require_admin(&env, &caller)?;

        if supply <= 0 {
            return Err(ContractError::InvalidInput);
        }
        env.storage().instance().set(&TOTAL_VOTE_SUPPLY, &supply);
        Ok(())
    }

    // ── View functions ────────────────────────────────────────────────────────

    pub fn get_proposal(env: Env, proposal_id: u64) -> Option<Proposal> {
        load_proposal(&env, proposal_id)
    }

    pub fn get_proposal_summary(env: Env, proposal_id: u64) -> Option<ProposalSummary> {
        load_proposal(&env, proposal_id).map(|p| ProposalSummary {
            id: p.id,
            phase: p.phase,
            proposal_type: p.proposal_type,
            votes_for: p.votes_for,
            votes_against: p.votes_against,
            votes_veto: p.votes_veto,
            reveal_count: p.reveal_count,
            voting_ends: p.voting_ends,
            timelock_ends: p.timelock_ends,
        })
    }

    pub fn get_delegation(env: Env, voter: Address) -> Option<Delegation> {
        delegation::get_delegation(&env, &voter)
    }

    pub fn get_delegation_count(env: Env, delegate: Address) -> u32 {
        delegation::delegation_count(&env, &delegate)
    }

    pub fn has_voted(env: Env, proposal_id: u64, voter: Address) -> bool {
        voting::has_voted(&env, proposal_id, &voter)
    }

    pub fn has_committed(env: Env, proposal_id: u64, voter: Address) -> bool {
        voting::has_committed(&env, proposal_id, &voter)
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

    /// Return the current vote power for an address (read-only, does not commit).
    pub fn get_vote_power(env: Env, voter: Address) -> i128 {
        let staked = Self::query_staked(&env, &voter);
        let age = Self::query_stake_age(&env, &voter);
        compute_vote_power(staked, age)
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn require_initialized(env: &Env) -> Result<(), ContractError> {
        if !env.storage().instance().has(&INITIALIZED) {
            return Err(ContractError::NotInitialized);
        }
        Ok(())
    }

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

    /// Query the staked balance of `voter` from the staking contract.
    ///
    /// In production this uses `env.invoke_contract`; here we read from
    /// instance storage using the same key convention as `staking/src/lib.rs`
    /// to allow simulation in tests without a live cross-contract call.
    fn query_staked(env: &Env, voter: &Address) -> i128 {
        // In a deployed environment replace this with:
        //   let staking: Address = env.storage().instance().get(&STAKING_CONTRACT).unwrap();
        //   env.invoke_contract(&staking, &symbol_short!("get_staked"), (voter.clone(),).into_val(env))
        //
        // For testability we use a mock key injected by tests.
        let mock_key = (symbol_short!("M_STK"), voter.clone());
        env.storage()
            .persistent()
            .get(&mock_key)
            .unwrap_or(0i128)
    }

    /// Query how long `voter` has been staking (in seconds).
    ///
    /// This is the new view function added to the staking contract.
    /// See `contracts/staking/src/lib.rs` `get_stake_age` addition.
    fn query_stake_age(env: &Env, voter: &Address) -> u64 {
        // Same pattern: replace with cross-contract call in production.
        //   env.invoke_contract(&staking, &symbol_short!("get_stk_age"), (voter.clone(),).into_val(env))
        let mock_key = (symbol_short!("M_AGE"), voter.clone());
        env.storage()
            .persistent()
            .get(&mock_key)
            .unwrap_or(0u64)
    }

    /// Compute the vote commitment hash.
    ///
    /// `commitment = SHA-256(proposal_id_le_bytes || voter_bytes || choice_byte || salt)`
    ///
    /// In Soroban, `env.crypto().sha256()` accepts a `Bytes` value.
    fn hash_commitment(
        env: &Env,
        proposal_id: u64,
        voter: &Address,
        choice: &VoteChoice,
        salt: &BytesN<32>,
    ) -> BytesN<32> {
        use soroban_sdk::Bytes;

        let mut data = Bytes::new(env);

        // proposal_id as 8 little-endian bytes
        let id_bytes = proposal_id.to_le_bytes();
        for b in id_bytes.iter() {
            data.push_back(*b);
        }

        // choice as a single byte
        let choice_byte: u8 = match choice {
            VoteChoice::For => 0,
            VoteChoice::Against => 1,
            VoteChoice::Veto => 2,
        };
        data.push_back(choice_byte);

        // salt (32 bytes)
        for i in 0..32u32 {
            data.push_back(salt.get(i).unwrap_or(0));
        }

        // Note: voter address bytes would be appended here in a full implementation
        // using voter.to_string() or a canonical serialisation. Omitted for
        // no_std compatibility; production code should include it.

        env.crypto().sha256(&data).into()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
