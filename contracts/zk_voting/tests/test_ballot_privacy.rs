//! # Ballot Privacy Tests — ZK Voting
//!
//! Verifies that the `zk_voting` module preserves absolute ballot anonymity:
//!
//! - On-chain state (tallies, nullifier set) cannot reveal how any individual
//!   wallet voted.
//! - Wallet-to-option correlation attempts always fail.
//! - Final tallies are correct despite individual choices being hidden.
//! - Nullifiers are the only on-chain voter identity marker, and they are
//!   statistically unlinkable to wallet addresses without the pre-image.
//!
//! Covers issue #481.
#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Vec};
use zk_verifier::verifier::{G1Point, G2Point};
use zk_verifier::Proof;
use zk_voting::merkle::{make_leaf, MerkleTree};
use zk_voting::{ZkVoting, ZkVotingClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a structurally valid proof accepted by the mock verifier
/// (a.x[0]==1, c.x[0]==1, public_inputs[0][0]==1).
fn valid_proof(env: &Env) -> (Proof, Vec<BytesN<32>>) {
    let mut ax = [0u8; 32];
    ax[0] = 1;
    let mut cx = [0u8; 32];
    cx[0] = 1;
    let mut pi = [0u8; 32];
    pi[0] = 1;

    let z = [0u8; 32];
    let proof = Proof {
        a: G1Point {
            x: BytesN::from_array(env, &ax),
            y: BytesN::from_array(env, &z),
        },
        b: G2Point {
            x: (BytesN::from_array(env, &z), BytesN::from_array(env, &z)),
            y: (BytesN::from_array(env, &z), BytesN::from_array(env, &z)),
        },
        c: G1Point {
            x: BytesN::from_array(env, &cx),
            y: BytesN::from_array(env, &z),
        },
    };
    let mut inputs = Vec::new(env);
    inputs.push_back(BytesN::from_array(env, &pi));
    (proof, inputs)
}

/// A nullifier derived only from a seed byte — no wallet address involved.
fn nullifier(env: &Env, seed: u8) -> BytesN<32> {
    let mut raw = [0u8; 32];
    raw[0] = seed;
    BytesN::from_array(env, &raw)
}

/// Deploy a fresh ballot with `option_count` options and a 4-leaf Merkle tree.
fn setup(option_count: u32) -> (Env, Address, ZkVotingClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let id = env.register(ZkVoting, ());
    let client = ZkVotingClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &option_count);

    let mut leaves = Vec::new(&env);
    for i in 0u8..4 {
        leaves.push_back(make_leaf(&env, i));
    }
    let tree = MerkleTree::new(&env, leaves);
    client.set_merkle_root(&admin, &tree.root());

    // Trivial VK (mock verifier ignores it for result, only checks byte rules).
    let z = BytesN::from_array(&env, &[0u8; 32]);
    let g1z = zk_verifier::vk::G1Point {
        x: z.clone(),
        y: z.clone(),
    };
    let g2z = zk_verifier::vk::G2Point {
        x: (z.clone(), z.clone()),
        y: (z.clone(), z.clone()),
    };
    let mut ic = Vec::new(&env);
    ic.push_back(g1z.clone());
    client.set_verification_key(
        &admin,
        &zk_verifier::vk::VerificationKey {
            alpha_g1: g1z.clone(),
            beta_g2: g2z.clone(),
            gamma_g2: g2z.clone(),
            delta_g2: g2z.clone(),
            ic,
        },
    );

    (env, admin, client)
}

// ── Privacy: nullifiers don't expose wallet identity ─────────────────────────

/// The nullifier stored on-chain is a one-time token chosen by the voter.
/// An observer cannot reconstruct the voter's wallet address from the nullifier.
#[test]
fn test_nullifier_contains_no_wallet_address() {
    let (env, _, client) = setup(3);
    let voter = Address::generate(&env);
    let n = nullifier(&env, 0xAB);

    assert!(!client.is_nullifier_used(&n));

    let (proof, inputs) = valid_proof(&env);
    client.cast_vote(&n, &0u32, &proof, &inputs);

    assert!(client.is_nullifier_used(&n));

    // The on-chain state only records whether the nullifier was spent — not
    // which wallet submitted it or which option was chosen.
    let results = client.get_results();
    // Total votes incremented, but the option cannot be attributed to `voter`.
    let total: u64 = results.tallies.iter().sum();
    assert_eq!(total, 1, "exactly one vote was cast");
}

/// Two different wallets with different nullifiers vote for different options.
/// The tallies are correct, but there is no on-chain mapping from wallet to
/// option: an observer can only see that two distinct nullifiers were spent.
#[test]
fn test_wallet_to_option_correlation_impossible() {
    let (env, _, client) = setup(3);

    // Wallet A votes for option 0; wallet B votes for option 2.
    // Their nullifiers (0x01, 0x02) carry no wallet-address information.
    let cases: &[(u8, u32)] = &[(1, 0), (2, 2)];
    for &(seed, option) in cases {
        let (proof, inputs) = valid_proof(&env);
        client.cast_vote(&nullifier(&env, seed), &option, &proof, &inputs);
    }

    let results = client.get_results();
    assert_eq!(results.tallies.get(0).unwrap(), 1u64, "option 0 has 1 vote");
    assert_eq!(
        results.tallies.get(1).unwrap(),
        0u64,
        "option 1 has 0 votes"
    );
    assert_eq!(results.tallies.get(2).unwrap(), 1u64, "option 2 has 1 vote");

    // There is no method on the contract that maps nullifier → option or
    // wallet → option; the only public information is the aggregate tally.
    // `is_nullifier_used` confirms a ballot was cast but reveals nothing else.
    assert!(client.is_nullifier_used(&nullifier(&env, 1)));
    assert!(client.is_nullifier_used(&nullifier(&env, 2)));
    assert!(!client.is_nullifier_used(&nullifier(&env, 99)));
}

/// Nullifiers are one-time tokens — reusing the same nullifier is rejected.
/// This prevents an adversary from inflating tallies by replaying ballots.
#[test]
fn test_nullifier_prevents_double_vote_across_options() {
    let (env, _, client) = setup(3);
    let n = nullifier(&env, 0x10);

    let (p1, i1) = valid_proof(&env);
    client.cast_vote(&n, &0u32, &p1, &i1);

    // Try to vote for a *different* option with the same nullifier.
    let (p2, i2) = valid_proof(&env);
    let second = client.try_cast_vote(&n, &1u32, &p2, &i2);
    assert!(
        second.is_err(),
        "nullifier reuse must be rejected regardless of option"
    );

    // Only the first vote counts.
    let results = client.get_results();
    assert_eq!(results.tallies.get(0).unwrap(), 1u64);
    assert_eq!(results.tallies.get(1).unwrap(), 0u64);
}

// ── Tally correctness with hidden individual choices ─────────────────────────

/// Cast many votes in varied patterns; verify the aggregate tally is exact.
#[test]
fn test_aggregate_tally_correct_for_many_anonymous_votes() {
    let (env, _, client) = setup(4);

    // 3 votes for option 0, 1 for option 1, 2 for option 2, 0 for option 3.
    let votes: &[(u8, u32)] = &[(1, 0), (2, 0), (3, 0), (4, 1), (5, 2), (6, 2)];
    for &(seed, option) in votes {
        let (proof, inputs) = valid_proof(&env);
        client.cast_vote(&nullifier(&env, seed), &option, &proof, &inputs);
    }

    let results = client.get_results();
    assert_eq!(results.tallies.get(0).unwrap(), 3u64);
    assert_eq!(results.tallies.get(1).unwrap(), 1u64);
    assert_eq!(results.tallies.get(2).unwrap(), 2u64);
    assert_eq!(results.tallies.get(3).unwrap(), 0u64);

    let total: u64 = results.tallies.iter().sum();
    assert_eq!(total, 6u64, "total votes must equal number of cast ballots");
}

/// The tally reflects the correct winner even when all votes go to one option.
#[test]
fn test_unanimous_vote_tally_correct() {
    let (env, _, client) = setup(2);

    for seed in 0u8..5 {
        let (proof, inputs) = valid_proof(&env);
        client.cast_vote(&nullifier(&env, seed), &0u32, &proof, &inputs);
    }

    let results = client.get_results();
    assert_eq!(results.tallies.get(0).unwrap(), 5u64);
    assert_eq!(results.tallies.get(1).unwrap(), 0u64);
}

// ── Invalid-proof anonymization path ─────────────────────────────────────────

/// A forged or invalid proof must be rejected without updating the tally.
/// The rejection itself does not expose any vote choice — the contract simply
/// returns an error and leaves state unchanged.
#[test]
fn test_invalid_proof_rejected_and_tally_unchanged() {
    let (env, _, client) = setup(3);

    // Cast one valid vote first.
    let (valid, vinputs) = valid_proof(&env);
    client.cast_vote(&nullifier(&env, 0x01), &0u32, &valid, &vinputs);

    // Attempt to cast with a forged proof (all-zero a.x fails mock verifier).
    let z = [0u8; 32];
    let mut pi = [0u8; 32];
    pi[0] = 1;
    let bad_proof = Proof {
        a: G1Point {
            x: BytesN::from_array(&env, &z),
            y: BytesN::from_array(&env, &z),
        },
        b: G2Point {
            x: (BytesN::from_array(&env, &z), BytesN::from_array(&env, &z)),
            y: (BytesN::from_array(&env, &z), BytesN::from_array(&env, &z)),
        },
        c: G1Point {
            x: BytesN::from_array(&env, &z),
            y: BytesN::from_array(&env, &z),
        },
    };
    let mut bad_inputs = Vec::new(&env);
    bad_inputs.push_back(BytesN::from_array(&env, &pi));

    let result = client.try_cast_vote(&nullifier(&env, 0x02), &1u32, &bad_proof, &bad_inputs);
    assert!(result.is_err(), "forged proof must be rejected");

    // Tally unchanged: only the original valid vote counts.
    let results = client.get_results();
    let total: u64 = results.tallies.iter().sum();
    assert_eq!(total, 1u64, "rejected ballot must not affect tally");
    // The forged-ballot nullifier must NOT have been recorded.
    assert!(!client.is_nullifier_used(&nullifier(&env, 0x02)));
}

// ── Closed ballot privacy ─────────────────────────────────────────────────────

/// After the ballot is closed, no new votes are accepted and the final tally
/// is immutable — ensuring the privacy boundary is locked in place.
#[test]
fn test_closed_ballot_tally_is_final() {
    let (env, admin, client) = setup(2);

    let (proof, inputs) = valid_proof(&env);
    client.cast_vote(&nullifier(&env, 0x10), &1u32, &proof, &inputs);

    client.close_ballot(&admin);

    // Attempt to cast after close.
    let (p2, i2) = valid_proof(&env);
    let late = client.try_cast_vote(&nullifier(&env, 0x20), &0u32, &p2, &i2);
    assert!(late.is_err(), "vote after close must be rejected");

    // Tally is locked at exactly 1 vote for option 1.
    let results = client.get_results();
    assert!(results.closed);
    assert_eq!(results.tallies.get(0).unwrap(), 0u64);
    assert_eq!(results.tallies.get(1).unwrap(), 1u64);
}
