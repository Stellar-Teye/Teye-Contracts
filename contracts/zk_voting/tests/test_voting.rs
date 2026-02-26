#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Vec};
use zk_verifier::verifier::{G1Point, G2Point};
use zk_verifier::Proof;
use zk_voting::merkle::{make_leaf, MerkleTree};
use zk_voting::{ZkVoting, ZkVotingClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a valid Groth16 proof (matches Bn254Verifier mock rules:
/// a[0]==1, c[0]==1, public_inputs[0][0]==1).
fn valid_proof(env: &Env) -> (Proof, Vec<BytesN<32>>) {
    // a is a G1Point: x=[1,0..], y=[0..]
    let mut ax = [0u8; 32];
    ax[0] = 1;
    let ay = [0u8; 32];

    // b is a G2Point: x=(x0,x1), y=(y0,y1) — all zeros is fine for the mock
    let bx0 = [0u8; 32];
    let bx1 = [0u8; 32];
    let by0 = [0u8; 32];
    let by1 = [0u8; 32];

    // c is a G1Point: x=[1,0..], y=[0..]
    let mut cx = [0u8; 32];
    cx[0] = 1;
    let cy = [0u8; 32];

    let mut pi = [0u8; 32];
    pi[0] = 1;

    let proof = Proof {
        a: G1Point {
            x: BytesN::from_array(env, &ax),
            y: BytesN::from_array(env, &ay),
        },
        b: G2Point {
            x: (BytesN::from_array(env, &bx0), BytesN::from_array(env, &bx1)),
            y: (BytesN::from_array(env, &by0), BytesN::from_array(env, &by1)),
        },
        c: G1Point {
            x: BytesN::from_array(env, &cx),
            y: BytesN::from_array(env, &cy),
        },
    };

    let mut pi = [0u8; 32];
    pi[0] = 1;

    let mut inputs: Vec<BytesN<32>> = Vec::new(env);
    inputs.push_back(BytesN::from_array(env, &pi));
    (proof, inputs)
}

/// Build an invalid proof (a.x[0]==0 fails the mock verifier).
fn invalid_proof(env: &Env) -> (Proof, Vec<BytesN<32>>) {
    // All-zero G1/G2 points — mock verifier rejects because a.x[0] != 1
    let z32 = [0u8; 32];

    let proof = Proof {
        a: G1Point {
            x: BytesN::from_array(env, &z32),
            y: BytesN::from_array(env, &z32),
        },
        b: G2Point {
            x: (BytesN::from_array(env, &z32), BytesN::from_array(env, &z32)),
            y: (BytesN::from_array(env, &z32), BytesN::from_array(env, &z32)),
        },
        c: G1Point {
            x: BytesN::from_array(env, &z32),
            y: BytesN::from_array(env, &z32),
        },
    };

    let pi = [0u8; 32];

    let mut inputs: Vec<BytesN<32>> = Vec::new(env);
    inputs.push_back(BytesN::from_array(env, &z32));
    (proof, inputs)
}

/// Make a 32-byte nullifier from a seed byte.
fn nullifier(env: &Env, seed: u8) -> BytesN<32> {
    let mut raw = [0u8; 32];
    raw[0] = seed;
    BytesN::from_array(env, &raw)
}

/// Deploy and initialise a fresh ballot with 3 options + a Merkle root.
fn setup() -> (Env, Address, ZkVotingClient<'static>, BytesN<32>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ZkVoting, ());
    let client = ZkVotingClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &3u32);

    // Build a 4-leaf Merkle tree and set the root.
    let mut leaves: Vec<BytesN<32>> = Vec::new(&env);
    for i in 0u8..4 {
        leaves.push_back(make_leaf(&env, i));
    }
    let tree = MerkleTree::new(&env, leaves);
    let root = tree.root();

    client.set_merkle_root(&admin, &root);

    // Setup verification key
    let g1 = zk_verifier::vk::G1Point {
        x: BytesN::from_array(&env, &[0u8; 32]),
        y: BytesN::from_array(&env, &[0u8; 32]),
    };
    let g2 = zk_verifier::vk::G2Point {
        x: (
            BytesN::from_array(&env, &[0u8; 32]),
            BytesN::from_array(&env, &[0u8; 32]),
        ),
        y: (
            BytesN::from_array(&env, &[0u8; 32]),
            BytesN::from_array(&env, &[0u8; 32]),
        ),
    };
    let mut ic = Vec::new(&env);
    ic.push_back(g1.clone());
    let vk = zk_verifier::vk::VerificationKey {
        alpha_g1: g1.clone(),
        beta_g2: g2.clone(),
        gamma_g2: g2.clone(),
        delta_g2: g2.clone(),
        ic,
    };
    client.set_verification_key(&admin, &vk);

    (env, admin, client, root)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_valid_vote_increments_tally() {
    let (env, _admin, client, _root) = setup();
    let (proof, inputs) = valid_proof(&env);

    client.cast_vote(&nullifier(&env, 1), &0u32, &proof, &inputs);

    let results = client.get_results();
    assert_eq!(results.tallies.get(0).unwrap(), 1u64);
    assert_eq!(results.tallies.get(1).unwrap(), 0u64);
    assert_eq!(results.tallies.get(2).unwrap(), 0u64);
}

#[test]
fn test_double_vote_rejected() {
    let (env, _admin, client, _root) = setup();
    let (proof1, inputs1) = valid_proof(&env);
    let (proof2, inputs2) = valid_proof(&env);
    let n = nullifier(&env, 2);

    // First vote succeeds.
    client.cast_vote(&n, &0u32, &proof1, &inputs1);

    // Second vote with same nullifier must fail.
    let result = client.try_cast_vote(&n, &1u32, &proof2, &inputs2);
    assert!(result.is_err());

    // Tally unchanged after the rejected vote.
    let results = client.get_results();
    assert_eq!(results.tallies.get(0).unwrap(), 1u64);
    assert_eq!(results.tallies.get(1).unwrap(), 0u64);
}

#[test]
fn test_invalid_proof_rejected() {
    let (env, _admin, client, _root) = setup();
    let (proof, inputs) = invalid_proof(&env);

    let result = client.try_cast_vote(&nullifier(&env, 3), &0u32, &proof, &inputs);
    assert!(result.is_err());

    let results = client.get_results();
    assert_eq!(results.tallies.get(0).unwrap(), 0u64);
}

#[test]
fn test_invalid_option_rejected() {
    let (env, _admin, client, _root) = setup();
    let (proof, inputs) = valid_proof(&env);

    // Option 3 is out of range (valid: 0, 1, 2).
    let result = client.try_cast_vote(&nullifier(&env, 4), &3u32, &proof, &inputs);
    assert!(result.is_err());
}

#[test]
fn test_ballot_results_are_public() {
    let (env, _admin, client, _root) = setup();

    for (seed, option) in [(1u8, 0u32), (2, 0), (3, 1), (4, 2)] {
        let (proof, inputs) = valid_proof(&env);
        client.cast_vote(&nullifier(&env, seed), &option, &proof, &inputs);
    }

    let results = client.get_results();
    let total: u64 = results.tallies.iter().sum();
    assert_eq!(total, 4u64);
    assert_eq!(results.tallies.get(0).unwrap(), 2u64);
    assert_eq!(results.tallies.get(1).unwrap(), 1u64);
    assert_eq!(results.tallies.get(2).unwrap(), 1u64);
}

#[test]
fn test_voting_closed_rejects_votes() {
    let (env, admin, client, _root) = setup();

    client.close_ballot(&admin);

    let (proof, inputs) = valid_proof(&env);
    let result = client.try_cast_vote(&nullifier(&env, 5), &0u32, &proof, &inputs);
    assert!(result.is_err());

    let results = client.get_results();
    assert!(results.closed);
}

#[test]
fn test_nullifier_tracking() {
    let (env, _admin, client, _root) = setup();
    let n = nullifier(&env, 6);

    assert!(!client.is_nullifier_used(&n));

    let (proof, inputs) = valid_proof(&env);
    client.cast_vote(&n, &2u32, &proof, &inputs);

    assert!(client.is_nullifier_used(&n));
}

#[test]
fn test_merkle_proof_verification() {
    let env = Env::default();

    let mut leaves: Vec<BytesN<32>> = Vec::new(&env);
    for i in 0u8..4 {
        leaves.push_back(make_leaf(&env, i));
    }

    let tree = MerkleTree::new(&env, leaves);
    let root = tree.root();

    for idx in 0u32..4 {
        let leaf = tree.leaf(idx);
        let proof = tree.proof(&env, idx);
        assert!(
            MerkleTree::verify_proof(&env, &root, &leaf, idx, &proof),
            "proof failed for leaf {idx}"
        );
    }
}

#[test]
fn test_tampered_merkle_leaf_fails() {
    let env = Env::default();

    let mut leaves: Vec<BytesN<32>> = Vec::new(&env);
    for i in 0u8..4 {
        leaves.push_back(make_leaf(&env, i));
    }

    let tree = MerkleTree::new(&env, leaves);
    let root = tree.root();
    let bogus = make_leaf(&env, 0xFF);
    let proof = tree.proof(&env, 0);

    assert!(!MerkleTree::verify_proof(&env, &root, &bogus, 0, &proof));
}
