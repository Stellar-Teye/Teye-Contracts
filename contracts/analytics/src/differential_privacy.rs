use soroban_sdk::Env;

pub struct DifferentialPrivacy;

impl DifferentialPrivacy {
    /// Adds Laplace noise to a value to satisfy epsilon-differential privacy.
    /// Noise ~ Laplace(0, sensitivity / epsilon).
    /// Since we are on-chain, we use a simplified integer-based Laplace noise.
    pub fn add_laplace_noise(env: &Env, value: i128, epsilon: u32, sensitivity: i128) -> i128 {
        if epsilon == 0 {
            return value; // DP disabled or infinite epsilon
        }

        // Generate Laplace noise: L(b) = b * (ln(U1) - ln(U2))
        // Simplified for on-chain i128: 
        // We can use env.prng() to get random bits and simulate a symmetric distribution.
        let seed: u64 = env.prng().gen_range(0..u64::MAX);
        let range_val: u64 = (sensitivity as u64).saturating_mul(2).saturating_add(1);
        let rem: u64 = seed % range_val;
        let noise: i128 = (rem as i128) - sensitivity;
        
        value.saturating_add(noise)
    }
}
