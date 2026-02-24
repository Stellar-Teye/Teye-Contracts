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
            proof: Proof {
                a: BytesN::from_array(env, &proof_a),
                b: BytesN::from_array(env, &proof_b),
                c: BytesN::from_array(env, &proof_c),
            },
            public_inputs: pi_vec,
        }
    }
}
