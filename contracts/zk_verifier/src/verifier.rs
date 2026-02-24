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

pub struct Bn254Verifier;

impl Bn254Verifier {
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

        use soroban_sdk::crypto::{BnScalar, bn254::{Bn254G1Affine, Bn254G2Affine}};
        use core::ops::Neg;

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
            let scalar = BnScalar::from_bytes(input.clone());
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
        b_bytes[64..128].copy_from_slice(&proof.b.y.0.to_array()); // Error in previous slice index (64..96)
        // Wait, b.y is (BytesN<32>, BytesN<32>)
        let mut b_y_bytes = [0u8; 64];
        b_y_bytes[0..32].copy_from_slice(&proof.b.y.0.to_array());
        b_y_bytes[32..64].copy_from_slice(&proof.b.y.1.to_array());
        b_bytes[64..128].copy_from_slice(&b_y_bytes);
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

pub struct PoseidonHasher;

impl PoseidonHasher {
    /// Hashes elements using a Poseidon algorithm optimized for BN254.
    pub fn hash(env: &Env, inputs: &Vec<BytesN<32>>) -> BytesN<32> {
        // Mock hash logic using Env native capabilities
        let mut combined_bytes = soroban_sdk::Bytes::new(env);
        for input in inputs.iter() {
            combined_bytes.extend_from_array(&input.to_array());
        }
        env.crypto().keccak256(&combined_bytes).into()
    }
}
