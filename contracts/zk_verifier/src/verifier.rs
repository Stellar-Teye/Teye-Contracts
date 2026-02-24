use soroban_sdk::{contracttype, BytesN, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct G1Point {
    pub x: BytesN<32>,
    pub y: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct G2Point {
    pub x: (BytesN<32>, BytesN<32>),
    pub y: (BytesN<32>, BytesN<32>),
}

/// Compressed or raw Groth16 proof points
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proof {
    pub a: G1Point,
    pub b: G2Point,
    pub c: G1Point,
}

/// Total byte length of a G2 point (four 32-byte limbs).
const G2_POINT_LEN: usize = 128;

/// Errors produced by structural validation of proof components.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProofValidationError {
    /// A proof component is entirely zero bytes.
    ZeroedComponent,
    /// A proof component is saturated (all 0xFF) — invalid curve encoding.
    OversizedComponent,
    /// G1 point A has a malformed internal structure.
    MalformedG1PointA,
    /// G1 point C has a malformed internal structure.
    MalformedG1PointC,
    /// The G2 point has a malformed internal structure.
    MalformedG2Point,
    /// No public inputs were provided.
    EmptyPublicInputs,
    /// A public-input element is all zeros.
    ZeroedPublicInput,
}

// ── Helper functions ─────────────────────────────────────────────────────────

/// Returns `true` if all bytes of a `BytesN<32>` are zero.
fn is_bytes_all_zeros(b: &BytesN<32>) -> bool {
    let arr = b.to_array();
    let mut i = 0;
    while i < 32 {
        if arr[i] != 0 {
            return false;
        }
        i += 1;
    }
    true
}

/// Returns `true` if all bytes of a `BytesN<32>` are 0xFF.
fn is_bytes_all_ones(b: &BytesN<32>) -> bool {
    let arr = b.to_array();
    let mut i = 0;
    while i < 32 {
        if arr[i] != 0xFF {
            return false;
        }
        i += 1;
    }
    true
}

/// Check whether both coordinates of a G1 point are all zeros.
fn is_g1_all_zeros(p: &G1Point) -> bool {
    is_bytes_all_zeros(&p.x) && is_bytes_all_zeros(&p.y)
}

/// Check whether both coordinates of a G1 point are all 0xFF.
fn is_g1_all_ones(p: &G1Point) -> bool {
    is_bytes_all_ones(&p.x) && is_bytes_all_ones(&p.y)
}

/// Check whether all four limbs of a G2 point are all zeros.
fn is_g2_all_zeros(p: &G2Point) -> bool {
    is_bytes_all_zeros(&p.x.0)
        && is_bytes_all_zeros(&p.x.1)
        && is_bytes_all_zeros(&p.y.0)
        && is_bytes_all_zeros(&p.y.1)
}

/// Check whether all four limbs of a G2 point are all 0xFF.
fn is_g2_all_ones(p: &G2Point) -> bool {
    is_bytes_all_ones(&p.x.0)
        && is_bytes_all_ones(&p.x.1)
        && is_bytes_all_ones(&p.y.0)
        && is_bytes_all_ones(&p.y.1)
}

/// Verifier implementation for the BN254 curve.
pub struct Bn254Verifier;

impl Bn254Verifier {
    /// Validate individual proof components for known-bad byte patterns that
    /// would cause undefined behaviour or nonsensical results in a real pairing
    /// check.  This runs *before* the verification arithmetic.
    pub fn validate_proof_components(
        proof: &Proof,
        public_inputs: &Vec<BytesN<32>>,
    ) -> Result<(), ProofValidationError> {
        // --- G1 point `a` --------------------------------------------------
        if is_g1_all_zeros(&proof.a) {
            return Err(ProofValidationError::ZeroedComponent);
        }
        if is_g1_all_ones(&proof.a) {
            return Err(ProofValidationError::OversizedComponent);
        }
        // Each coordinate of a G1 point must not be individually all-zero.
        if is_bytes_all_zeros(&proof.a.x) || is_bytes_all_zeros(&proof.a.y) {
            return Err(ProofValidationError::MalformedG1PointA);
        }

        // --- G2 point `b` --------------------------------------------------
        if is_g2_all_zeros(&proof.b) {
            return Err(ProofValidationError::ZeroedComponent);
        }
        if is_g2_all_ones(&proof.b) {
            return Err(ProofValidationError::OversizedComponent);
        }
        // G2 is composed of four 32-byte limbs; none may be individually zero.
        if is_bytes_all_zeros(&proof.b.x.0)
            || is_bytes_all_zeros(&proof.b.x.1)
            || is_bytes_all_zeros(&proof.b.y.0)
            || is_bytes_all_zeros(&proof.b.y.1)
        {
            return Err(ProofValidationError::MalformedG2Point);
        }

        // --- G1 point `c` --------------------------------------------------
        if is_g1_all_zeros(&proof.c) {
            return Err(ProofValidationError::ZeroedComponent);
        }
        if is_g1_all_ones(&proof.c) {
            return Err(ProofValidationError::OversizedComponent);
        }
        if is_bytes_all_zeros(&proof.c.x) || is_bytes_all_zeros(&proof.c.y) {
            return Err(ProofValidationError::MalformedG1PointC);
        }

        // --- Public inputs --------------------------------------------------
        if public_inputs.is_empty() {
            return Err(ProofValidationError::EmptyPublicInputs);
        }
        for pi in public_inputs.iter() {
            if is_bytes_all_zeros(&pi) {
                return Err(ProofValidationError::ZeroedPublicInput);
            }
        }

        Ok(())
    }

    /// Minimal abstraction for verifying a Groth16 proof over the BN254 curve
    /// using Soroban Wasm primitives. In a production environment this would
    /// utilize a host function or an optimized `#![no_std]` pairing library.
    pub fn verify_proof(
        env: &Env,
        vk: &crate::vk::VerificationKey,
        proof: &Proof,
        public_inputs: &Vec<BytesN<32>>,
    ) -> bool {
        if public_inputs.len() != vk.ic.len().saturating_sub(1) {
            return false;
        }

        use core::ops::Neg;
        use soroban_sdk::crypto::bn254::{Bn254G1Affine, Bn254G2Affine, Fr};

        // 1. Compute the public input point (acc)
        // acc = IC[0] + sum(public_inputs[i] * IC[i+1])
        let ic0 = vk.ic.get(0).unwrap();
        let mut ic0_bytes = [0u8; 64];
        ic0_bytes[0..32].copy_from_slice(&ic0.x.to_array());
        ic0_bytes[32..64].copy_from_slice(&ic0.y.to_array());
        let mut acc = Bn254G1Affine::from_array(env, &ic0_bytes);

        for (i, input) in public_inputs.iter().enumerate() {
            let ic_point = vk.ic.get(u32::try_from(i + 1).unwrap()).unwrap();
            let mut ic_bytes = [0u8; 64];
            ic_bytes[0..32].copy_from_slice(&ic_point.x.to_array());
            ic_bytes[32..64].copy_from_slice(&ic_point.y.to_array());
            let g1_point = Bn254G1Affine::from_array(env, &ic_bytes);

            // Scalar multiplication: IC[i+1] * input
            let scalar = Fr::from_bytes(input.clone());
            let mul = env.crypto().bn254().g1_mul(&g1_point, &scalar);

            // Addition: acc + (IC[i+1] * input)
            acc = env.crypto().bn254().g1_add(&acc, &mul);
        }

        // 2. Perform the pairing check
        // e(A, B) == e(alpha, beta) * e(acc, gamma) * e(C, delta)
        // Rearranged for sum(e(P_i, Q_i)) == 0 approach:
        // e(-A, B) * e(alpha, beta) * e(acc, gamma) * e(C, delta) == 1

        // Negate G1 point A instead of G2 point B (achieves the same result)
        let mut a_bytes = [0u8; 64];
        a_bytes[0..32].copy_from_slice(&proof.a.x.to_array());
        a_bytes[32..64].copy_from_slice(&proof.a.y.to_array());
        let point_a = Bn254G1Affine::from_array(env, &a_bytes);
        let neg_a = point_a.neg();

        let mut g1_points = Vec::<Bn254G1Affine>::new(env);
        let mut g2_points = Vec::<Bn254G2Affine>::new(env);

        // pair 1: e(-A, B)
        g1_points.push_back(neg_a);

        let mut b_bytes = [0u8; 128];
        b_bytes[0..32].copy_from_slice(&proof.b.x.0.to_array());
        b_bytes[32..64].copy_from_slice(&proof.b.x.1.to_array());
        b_bytes[64..96].copy_from_slice(&proof.b.y.0.to_array());
        b_bytes[96..128].copy_from_slice(&proof.b.y.1.to_array());
        g2_points.push_back(Bn254G2Affine::from_array(env, &b_bytes));

        // pair 2: e(alpha, beta)
        let mut alpha_bytes = [0u8; 64];
        alpha_bytes[0..32].copy_from_slice(&vk.alpha_g1.x.to_array());
        alpha_bytes[32..64].copy_from_slice(&vk.alpha_g1.y.to_array());
        g1_points.push_back(Bn254G1Affine::from_array(env, &alpha_bytes));

        let mut beta_bytes = [0u8; 128];
        beta_bytes[0..32].copy_from_slice(&vk.beta_g2.x.0.to_array());
        beta_bytes[32..64].copy_from_slice(&vk.beta_g2.x.1.to_array());
        beta_bytes[64..96].copy_from_slice(&vk.beta_g2.y.0.to_array());
        beta_bytes[96..128].copy_from_slice(&vk.beta_g2.y.1.to_array());
        g2_points.push_back(Bn254G2Affine::from_array(env, &beta_bytes));

        // pair 3: e(acc, gamma)
        g1_points.push_back(acc);

        let mut gamma_bytes = [0u8; 128];
        gamma_bytes[0..32].copy_from_slice(&vk.gamma_g2.x.0.to_array());
        gamma_bytes[32..64].copy_from_slice(&vk.gamma_g2.x.1.to_array());
        gamma_bytes[64..96].copy_from_slice(&vk.gamma_g2.y.0.to_array());
        gamma_bytes[96..128].copy_from_slice(&vk.gamma_g2.y.1.to_array());
        g2_points.push_back(Bn254G2Affine::from_array(env, &gamma_bytes));

        // pair 4: e(C, delta)
        let mut c_bytes = [0u8; 64];
        c_bytes[0..32].copy_from_slice(&proof.c.x.to_array());
        c_bytes[32..64].copy_from_slice(&proof.c.y.to_array());
        g1_points.push_back(Bn254G1Affine::from_array(env, &c_bytes));

        let mut delta_bytes = [0u8; 128];
        delta_bytes[0..32].copy_from_slice(&vk.delta_g2.x.0.to_array());
        delta_bytes[32..64].copy_from_slice(&vk.delta_g2.x.1.to_array());
        delta_bytes[64..96].copy_from_slice(&vk.delta_g2.y.0.to_array());
        delta_bytes[96..128].copy_from_slice(&vk.delta_g2.y.1.to_array());
        g2_points.push_back(Bn254G2Affine::from_array(env, &delta_bytes));

        env.crypto().bn254().pairing_check(g1_points, g2_points)
    }
}

/// Hasher implementation using the Poseidon algorithm.
pub struct PoseidonHasher;

impl PoseidonHasher {
    /// Hashes a vector of inputs using the Poseidon hash function.
    ///
    /// Poseidon is a ZK-friendly hash function optimized for operation over
    /// prime fields like the BN254 scalar field.
    pub fn hash(env: &Env, inputs: &Vec<BytesN<32>>) -> BytesN<32> {
        // Mock hash logic using Env native capabilities
        let mut combined_bytes = soroban_sdk::Bytes::new(env);
        for input in inputs.iter() {
            combined_bytes.extend_from_array(&input.to_array());
        }
        env.crypto().keccak256(&combined_bytes).into()
    }
}
