#![no_std]

pub mod circuit;

use circuit::{AccessWitness, ZkAccessCircuit};
use soroban_sdk::{Address, Env};
use zk_verifier::{AccessRequest, ZkAccessHelper};

/// Generates a mock `AccessRequest` compatible with the on-chain Verification logic.
///
/// In a real ZK application, this would invoke a prover backend (like SnarkJS or
/// an arkworks implementation) using the provided `witness` to compute Groth16 points
/// `a`, `b`, and `c`, as well as validating that the witness satisfies the `circuit`.
///
/// Because the target `ZkVerifierContract` currently verifies proofs by checking if
/// the first byte of `a.x` and `c.x` and the first public input is `1`, this Prover SDK
/// simply outputs bytes conforming to that logic depending on whether the `witness` is valid.
pub fn generate_proof(
    env: &Env,
    user: Address,
    resource_id: [u8; 32],
    witness: AccessWitness,
    public_inputs: &[&[u8; 32]],
) -> AccessRequest {
    // Check if the circuit logic dictates this is a valid proof formulation.
    let is_valid = ZkAccessCircuit::validate(&witness, public_inputs);

    // Default template (matching structural requirements for BN254 non-degenerate shapes).
    let mut proof_a = [0u8; 64];
    let mut proof_b = [0u8; 128];
    let mut proof_c = [0u8; 64];

    // Common non-degenerate bytes for B to avoid `MalformedG2Point` or `DegenerateProof`
    proof_b[0] = 1;
    proof_b[32] = 0x02;
    proof_b[64] = 0x03;
    proof_b[96] = 0x04;

    if is_valid {
        // Mock valid
        proof_a[0] = 1; // passes proof.a.x.get(0) == 1
        proof_a[32] = 0x02;

        proof_c[0] = 1; // passes proof.c.x.get(0) == 1
        proof_c[32] = 0x02;

        // Note: The verifier also expects public_inputs[0][0] == 1,
        // which must be provided by the caller in `public_inputs` explicitly
    } else {
        // Mock invalid (structurally okay, but pairing/validation logic will fail).
        proof_a[0] = 0xFF; // fails proof.a.x.get(0) == 1
        proof_a[32] = 0x02;

        proof_c[0] = 0xFF;
        proof_c[32] = 0x02;
    }

    ZkAccessHelper::create_request(
        env,
        user,
        resource_id,
        proof_a,
        proof_b,
        proof_c,
        public_inputs,
    )
}
