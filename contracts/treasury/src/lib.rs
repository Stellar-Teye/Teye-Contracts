#![no_std]

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, String, Symbol, Vec,
};

// ── Storage keys ────────────────────────────────────────────────────────────────

const CONFIG: Symbol = symbol_short!("CONFIG");
const PROPOSAL_CTR: Symbol = symbol_short!("PR_CTR");
const PROPOSAL: Symbol = symbol_short!("PROPOSAL");
const ALLOCATION: Symbol = symbol_short!("ALLOC");

// ── Types ──────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreasuryConfig {
    /// Address that may update configuration and sign proposals.
    pub admin: Address,
    /// ERC-20–like token contract address that represents treasury funds.
    pub token: Address,
    /// Set of signer addresses authorised to create/approve/execute proposals.
    pub signers: Vec<Address>,
    /// Number of distinct signer approvals required to execute a proposal.
    pub threshold: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    Pending,
    Executed,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub to: Address,
    pub amount: i128,
    pub category: Symbol,
    pub description: String,
    pub approvals: Vec<Address>,
    pub status: ProposalStatus,
    pub created_at: u64,
    pub expires_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationSummary {
    pub category: Symbol,
    pub total_spent: i128,
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn is_signer(env: &Env, who: &Address) -> bool {
    let cfg: TreasuryConfig = env
        .storage()
        .instance()
        .get(&CONFIG)
        .expect("config not set");
    cfg.signers.iter().any(|s| s == *who)
}

fn load_config(env: &Env) -> TreasuryConfig {
    env.storage()
        .instance()
        .get(&CONFIG)
        .expect("config not set")
}

fn next_proposal_id(env: &Env) -> u64 {
    let current: u64 = env.storage().instance().get(&PROPOSAL_CTR).unwrap_or(0);
    let next = current.saturating_add(1);
    env.storage().instance().set(&PROPOSAL_CTR, &next);
    next
}

fn proposal_key(id: u64) -> (Symbol, u64) {
    (PROPOSAL, id)
}

fn allocation_key(category: &Symbol) -> (Symbol, Symbol) {
    (ALLOCATION, category.clone())
}

fn has_approval(_env: &Env, proposal: &Proposal, signer: &Address) -> bool {
    proposal.approvals.iter().any(|s| s == *signer)
}

fn count_approvals(proposal: &Proposal) -> u32 {
    proposal.approvals.len()
}

// ── Contract ───────────────────────────────────────────────────────────────────

#[contract]
pub struct TreasuryContract;

#[contractimpl]
impl TreasuryContract {
    // ── Configuration ─────────────────────────────────────────────────────────

    /// Initialise the treasury with an admin, token, signers, and threshold.
    ///
    /// All signers, including the admin, participate in the multi-sig scheme.
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        signers: Vec<Address>,
        threshold: u32,
    ) {
        if env.storage().instance().has(&CONFIG) {
            panic!("already initialized");
        }
        if signers.is_empty() {
            panic!("no signers");
        }
        if threshold == 0 || threshold > signers.len() {
            panic!("invalid threshold");
        }

        let cfg = TreasuryConfig {
            admin,
            token,
            signers,
            threshold,
        };

        env.storage().instance().set(&CONFIG, &cfg);
    }

    pub fn get_config(env: Env) -> TreasuryConfig {
        load_config(&env)
    }

    // ── Proposal lifecycle ────────────────────────────────────────────────────

    /// Create a new spending proposal. Only authorised signers may create.
    pub fn create_proposal(
        env: Env,
        proposer: Address,
        to: Address,
        amount: i128,
        category: Symbol,
        description: String,
        expires_at: u64,
    ) -> Proposal {
        proposer.require_auth();

        if amount <= 0 {
            panic!("amount must be positive");
        }

        if !is_signer(&env, &proposer) {
            panic!("unauthorised proposer");
        }

        let now = env.ledger().timestamp();
        if expires_at <= now {
            panic!("expiry must be in the future");
        }

        let id = next_proposal_id(&env);

        let approvals = {
            let mut v = Vec::new(&env);
            // Optional: auto-approve by proposer to reduce friction.
            v.push_back(proposer.clone());
            v
        };

        let proposal = Proposal {
            id,
            proposer,
            to,
            amount,
            category,
            description,
            approvals,
            status: ProposalStatus::Pending,
            created_at: now,
            expires_at,
        };

        env.storage().persistent().set(&proposal_key(id), &proposal);
        proposal
    }

    pub fn get_proposal(env: Env, id: u64) -> Option<Proposal> {
        env.storage().persistent().get(&proposal_key(id))
    }

    /// Approve a proposal. Duplicate approvals are ignored.
    pub fn approve_proposal(env: Env, signer: Address, id: u64) {
        signer.require_auth();

        if !is_signer(&env, &signer) {
            panic!("unauthorised signer");
        }

        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&proposal_key(id))
            .expect("proposal not found");

        if !matches!(proposal.status, ProposalStatus::Pending) {
            panic!("proposal not pending");
        }

        let now = env.ledger().timestamp();
        if now >= proposal.expires_at {
            proposal.status = ProposalStatus::Expired;
            env.storage().persistent().set(&proposal_key(id), &proposal);
            panic!("proposal expired");
        }

        if has_approval(&env, &proposal, &signer) {
            // No-op if already approved.
            return;
        }

        proposal.approvals.push_back(signer);
        env.storage().persistent().set(&proposal_key(id), &proposal);
    }

    /// Execute an approved proposal, transferring funds from the treasury to
    /// the destination address and recording allocation statistics.
    pub fn execute_proposal(env: Env, signer: Address, id: u64) {
        signer.require_auth();

        if !is_signer(&env, &signer) {
            panic!("unauthorised signer");
        }

        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&proposal_key(id))
            .expect("proposal not found");

        if !matches!(proposal.status, ProposalStatus::Pending) {
            panic!("proposal not pending");
        }

        let now = env.ledger().timestamp();
        if now >= proposal.expires_at {
            proposal.status = ProposalStatus::Expired;
            env.storage().persistent().set(&proposal_key(id), &proposal);
            panic!("proposal expired");
        }

        let cfg = load_config(&env);
        let approvals = count_approvals(&proposal);
        if approvals < cfg.threshold {
            panic!("insufficient approvals");
        }

        // Perform the token transfer.
        let token_client = token::Client::new(&env, &cfg.token);
        token_client.transfer(
            &env.current_contract_address(),
            &proposal.to,
            &proposal.amount,
        );

        // Mark as executed.
        proposal.status = ProposalStatus::Executed;
        env.storage().persistent().set(&proposal_key(id), &proposal);

        // Update allocation tracking.
        let key = allocation_key(&proposal.category);
        let mut spent: i128 = env.storage().instance().get(&key).unwrap_or(0);
        spent = spent.saturating_add(proposal.amount);
        env.storage().instance().set(&key, &spent);
    }

    // ── Reporting helpers ─────────────────────────────────────────────────────

    /// Returns how much has been spent for a given category across all
    /// executed proposals.
    pub fn get_allocation_for_category(env: Env, category: Symbol) -> AllocationSummary {
        let key = allocation_key(&category);
        let spent: i128 = env.storage().instance().get(&key).unwrap_or(0);
        AllocationSummary {
            category,
            total_spent: spent,
        }
    }
}
