#![no_std]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]

pub mod ballot;
pub mod merkle;

use ballot::{DataKey, OptionIndex, VoteError};
use soroban_sdk::{
    contract, contractimpl, contracttype, panic_with_error, Address, BytesN, Env, Vec,
};
use zk_verifier::{Bn254Verifier, Proof};

#[contracttype]
#[derive(Clone, Debug)]
pub struct BallotResults {
    pub option_count: u32,
    pub tallies: Vec<u64>,
    pub closed: bool,
}

#[contract]
pub struct ZkVoting;

#[contractimpl]
impl ZkVoting {
    /// Initialise the ballot.
    /// - `admin`        : address that manages the ballot
    /// - `option_count` : number of vote options (>= 2)
    pub fn initialize(env: Env, admin: Address, option_count: u32) {
        assert!(
            !env.storage().instance().has(&DataKey::Admin),
            "already initialised"
        );
        assert!(option_count >= 2, "need at least 2 options");

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::OptionCount, &option_count);
        env.storage().instance().set(&DataKey::Closed, &false);

        for i in 0..option_count {
            env.storage().persistent().set(&DataKey::Tally(i), &0u64);
        }
    }

    /// Set the Merkle root that defines eligible voters. Admin only.
    pub fn set_merkle_root(env: Env, caller: Address, root: BytesN<32>) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        Self::require_open(&env);
        env.storage().persistent().set(&DataKey::MerkleRoot, &root);
    }

    /// Set the Verification key for ZK proof validation. Admin only.
    pub fn set_verification_key(env: Env, caller: Address, vk: zk_verifier::vk::VerificationKey) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        env.storage().instance().set(&DataKey::VerificationKey, &vk);
    }

    /// Return the current Verification key.
    pub fn get_verification_key(env: Env) -> Option<zk_verifier::vk::VerificationKey> {
        env.storage().instance().get(&DataKey::VerificationKey)
    }

    /// Close the ballot. No more votes accepted after this.
    pub fn close_ballot(env: Env, caller: Address) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        env.storage().instance().set(&DataKey::Closed, &true);
    }

    /// Cast an anonymous vote.
    /// - `nullifier`    : 32-byte one-time tag to prevent double-voting
    /// - `option_index` : which option to vote for (0-based)
    /// - `proof`        : Groth16 ZK proof of Merkle membership
    /// - `public_inputs`: public signals (first element must encode the root)
    pub fn cast_vote(
        env: Env,
        nullifier: BytesN<32>,
        option_index: OptionIndex,
        proof: Proof,
        public_inputs: Vec<BytesN<32>>,
    ) -> Result<(), VoteError> {
        // 1. Ballot must be open
        Self::require_open(&env);

        // 2. Option must be valid
        let option_count: u32 = env.storage().instance().get(&DataKey::OptionCount).unwrap();
        if option_index >= option_count {
            return Err(VoteError::InvalidOption);
        }

        // 3. Nullifier must be fresh
        if env
            .storage()
            .persistent()
            .has(&DataKey::Nullifier(nullifier.clone()))
        {
            return Err(VoteError::NullifierAlreadyUsed);
        }

        // 4. Merkle root must be set
        let _root: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::MerkleRoot)
            .ok_or(VoteError::MerkleRootNotSet)?;

        // 5. Verify the ZK proof
        let vk_opt: Option<zk_verifier::vk::VerificationKey> =
            env.storage().instance().get(&DataKey::VerificationKey);
        let vk = vk_opt.ok_or(VoteError::InvalidProof)?;
        if !Bn254Verifier::verify_proof(&env, &vk, &proof, &public_inputs) {
            return Err(VoteError::InvalidProof);
        }

        // 6. Spend the nullifier
        env.storage()
            .persistent()
            .set(&DataKey::Nullifier(nullifier), &true);

        // 7. Increment tally
        let current: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Tally(option_index))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Tally(option_index), &(current + 1));

        Ok(())
    }

    /// Return tallies for all options. Publicly verifiable.
    pub fn get_results(env: Env) -> BallotResults {
        let option_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::OptionCount)
            .unwrap_or(0);
        let closed: bool = env
            .storage()
            .instance()
            .get(&DataKey::Closed)
            .unwrap_or(false);

        let mut tallies: Vec<u64> = Vec::new(&env);
        for i in 0..option_count {
            let t: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::Tally(i))
                .unwrap_or(0);
            tallies.push_back(t);
        }
        BallotResults {
            option_count,
            tallies,
            closed,
        }
    }

    /// Check if a nullifier has been spent.
    pub fn is_nullifier_used(env: Env, nullifier: BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Nullifier(nullifier))
    }

    /// Return the current Merkle root.
    pub fn get_merkle_root(env: Env) -> Option<BytesN<32>> {
        env.storage().persistent().get(&DataKey::MerkleRoot)
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    fn require_admin(env: &Env, caller: &Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if caller != &admin {
            panic_with_error!(env, VoteError::Unauthorized);
        }
    }

    fn require_open(env: &Env) {
        let closed: bool = env
            .storage()
            .instance()
            .get(&DataKey::Closed)
            .unwrap_or(false);
        if closed {
            panic_with_error!(env, VoteError::BallotNotOpen);
        }
    }
}
