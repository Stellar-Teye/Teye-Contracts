use crate::verifier::{G1Point, G2Point};
use crate::{AccessRequest, Proof};
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
    pub fn create_request(
        env: &Env,
        user: soroban_sdk::Address,
        resource_id: [u8; 32],
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        public_inputs: &[&[u8; 32]],
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
            nonce: 0, // Default nonce; caller should set appropriately for replay protection
        }
    }
}
