<<<<<<< HEAD
use crate::{
    verifier::{G1Point, G2Point, Proof, PoseidonHasher},
    AccessRequest,
};
=======
use crate::verifier::{G1Point, G2Point};
use crate::{AccessRequest, Proof};
>>>>>>> 8ac60fcc51b5991fb5c3c3a879dcb5daa5df7d74
use soroban_sdk::{BytesN, Env, Vec};

/// Helper utility for creating ZK access requests.
pub struct ZkAccessHelper;

impl ZkAccessHelper {
    fn to_bytesn32(env: &Env, bytes: &[u8]) -> BytesN<32> {
        let mut buf = [0u8; 32];
        if bytes.len() == 32 {
            buf.copy_from_slice(bytes);
        }
        BytesN::from_array(env, &buf)
    }

    /// Formats raw cryptographic proof points and public inputs into a standard `AccessRequest`.
    ///
    /// This helper is intended for use in tests and off-chain tools to ensure consistent
    /// formatting of the `AccessRequest` structure submitted to the `ZkVerifierContract`.
    #[allow(clippy::too_many_arguments)]
    pub fn create_request(
        env: &Env,
        user: soroban_sdk::Address,
        resource_id: [u8; 32],
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        public_inputs: &[&[u8; 32]],
        expires_at: u64,
    ) -> AccessRequest {
        let mut pi_vec = Vec::new(env);
        for &pi in public_inputs {
            pi_vec.push_back(BytesN::from_array(env, pi));
        }

        AccessRequest {
            user,
            resource_id: BytesN::from_array(env, &resource_id),
            proof: Proof {
                a: G1Point {
                    x: Self::to_bytesn32(env, &proof_a[0..32]),
                    y: Self::to_bytesn32(env, &proof_a[32..64]),
                },
                b: G2Point {
                    x: (
                        Self::to_bytesn32(env, &proof_b[0..32]),
                        Self::to_bytesn32(env, &proof_b[32..64]),
                    ),
                    y: (
                        Self::to_bytesn32(env, &proof_b[64..96]),
                        Self::to_bytesn32(env, &proof_b[96..128]),
                    ),
                },
                c: G1Point {
                    x: Self::to_bytesn32(env, &proof_c[0..32]),
                    y: Self::to_bytesn32(env, &proof_c[32..64]),
                },
            },
            public_inputs: pi_vec,
            expires_at,
            nonce: 0, // Default nonce; caller should set appropriately for replay protection
        }
    }
}
/// Merkle tree proof verification utilities.
pub struct MerkleVerifier;

impl MerkleVerifier {
    /// Verifies a Merkle proof that a leaf exists in a tree with the given root.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `root` - The expected Merkle tree root hash
    /// * `leaf` - The leaf data to verify
    /// * `proof_path` - Vector of (sibling_hash, is_left) tuples
    ///   - `sibling_hash`: The hash of the sibling node at this level
    ///   - `is_left`: If true, sibling is on the left; if false, sibling is on the right
    ///
    /// # Returns
    /// `true` if the proof is valid and the leaf exists in the tree, `false` otherwise
    ///
    /// # Example
    /// ```ignore
    /// // For a tree:
    /// //       root
    /// //      /    \
    /// //     h1     h2
    /// //    / \    / \
    /// //   L0 L1  L2 L3
    /// //
    /// // To prove L0 exists:
    /// // proof_path = [(hash(L1), false), (hash(h2), false)]
    /// // First step: hash(L0, L1) = h1
    /// // Second step: hash(h1, h2) = root
    /// ```
    pub fn verify_merkle_proof(
        env: &Env,
        root: &BytesN<32>,
        leaf: &BytesN<32>,
        proof_path: &Vec<(BytesN<32>, bool)>,
    ) -> bool {
        // Maximum tree depth to prevent excessive gas consumption
        const MAX_DEPTH: u32 = 32;

        if proof_path.len() > MAX_DEPTH {
            return false;
        }

        // Start with the leaf hash
        let mut current_hash = leaf.clone();

        // Traverse the proof path from leaf to root
        for i in 0..proof_path.len() {
            let (sibling_hash, is_left) = proof_path.get_unchecked(i);

            // Create a vector with both hashes in the correct order
            let mut hashes = Vec::new(env);
            
            if is_left {
                // Sibling is on the left, current is on the right
                hashes.push_back(sibling_hash);
                hashes.push_back(current_hash.clone());
            } else {
                // Current is on the left, sibling is on the right
                hashes.push_back(current_hash.clone());
                hashes.push_back(sibling_hash);
            }

            // Hash the pair to get the parent node
            current_hash = PoseidonHasher::hash(env, &hashes);
        }

        // The final computed hash should match the root
        current_hash == *root
    }

    /// Computes a Merkle root from a list of leaves.
    /// This is a helper function primarily for testing.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `leaves` - Vector of leaf hashes
    ///
    /// # Returns
    /// The Merkle root hash
    pub fn compute_merkle_root(env: &Env, leaves: &Vec<BytesN<32>>) -> BytesN<32> {
        if leaves.is_empty() {
            return BytesN::from_array(env, &[0u8; 32]);
        }

        if leaves.len() == 1 {
            return leaves.get_unchecked(0);
        }

        let mut current_level = leaves.clone();

        while current_level.len() > 1 {
            let mut next_level = Vec::new(env);
            let mut i = 0;

            while i < current_level.len() {
                let left = current_level.get_unchecked(i);
                
                // If odd number of nodes, duplicate the last one
                let right = if i + 1 < current_level.len() {
                    current_level.get_unchecked(i + 1)
                } else {
                    left.clone()
                };

                let mut pair = Vec::new(env);
                pair.push_back(left);
                pair.push_back(right);
                
                let parent = PoseidonHasher::hash(env, &pair);
                next_level.push_back(parent);

                i += 2;
            }

            current_level = next_level;
        }

        current_level.get_unchecked(0)
    }
}