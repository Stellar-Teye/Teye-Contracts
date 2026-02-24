use crate::AccessRequest;
use soroban_sdk::{BytesN, Env, Vec};

/// Helper utility for creating ZK access requests.
pub struct ZkAccessHelper;

impl ZkAccessHelper {
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
                    x: BytesN::from_array(env, &proof_a[0..32].try_into().unwrap()),
                    y: BytesN::from_array(env, &proof_a[32..64].try_into().unwrap()),
                },
                b: G2Point {
                    x: (
                        BytesN::from_array(env, &proof_b[0..32].try_into().unwrap()),
                        BytesN::from_array(env, &proof_b[32..64].try_into().unwrap()),
                    ),
                    y: (
                        BytesN::from_array(env, &proof_b[64..96].try_into().unwrap()),
                        BytesN::from_array(env, &proof_b[96..128].try_into().unwrap()),
                    ),
                },
                c: G1Point {
                    x: BytesN::from_array(env, &proof_c[0..32].try_into().unwrap()),
                    y: BytesN::from_array(env, &proof_c[32..64].try_into().unwrap()),
                },
            },
            public_inputs: pi_vec,
        }
    }
}
