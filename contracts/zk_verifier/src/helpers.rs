use crate::verifier::Proof;
use crate::AccessRequest;
use soroban_sdk::{BytesN, Env, Vec};

pub struct ZkAccessHelper;

impl ZkAccessHelper {
    /// Helper to format raw byte arrays into the contract's standard `AccessRequest`.
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
            proof: crate::verifier::Proof {
                a: crate::verifier::G1Point {
                    x: BytesN::from_array(env, &proof_a[0..32].try_into().unwrap()),
                    y: BytesN::from_array(env, &proof_a[32..64].try_into().unwrap()),
                },
                b: crate::verifier::G2Point {
                    x: (
                        BytesN::from_array(env, &proof_b[0..32].try_into().unwrap()),
                        BytesN::from_array(env, &proof_b[32..64].try_into().unwrap()),
                    ),
                    y: (
                        BytesN::from_array(env, &proof_b[64..96].try_into().unwrap()),
                        BytesN::from_array(env, &proof_b[96..128].try_into().unwrap()),
                    ),
                },
                c: crate::verifier::G1Point {
                    x: BytesN::from_array(env, &proof_c[0..32].try_into().unwrap()),
                    y: BytesN::from_array(env, &proof_c[32..64].try_into().unwrap()),
                },
            },
            public_inputs: pi_vec,
        }
    }
}
