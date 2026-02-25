use soroban_sdk::{Env, contracttype};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaillierPublicKey {
    pub n: i128,  // n = p * q
    pub nn: i128, // n^2
    pub g: i128,  // g = n + 1
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaillierPrivateKey {
    pub lambda: i128, // phi(n) = (p-1)(q-1)
    pub mu: i128,     // L(g^lambda mod n^2)^-1 mod n
}

pub struct HomomorphicEngine;

impl HomomorphicEngine {
    /// Simplified encryption: c = (g^m * r^n) mod n^2
    /// For analytics, we often use r=1 or a fixed base if r is hard to generate no_std
    /// However, for full security r should be random.
    pub fn encrypt(_env: &Env, pub_key: &PaillierPublicKey, m: i128) -> i128 {
        let n = pub_key.n;
        let nn = pub_key.nn;
        let g = pub_key.g;
        
        // r = 17 (fixed for simplicity in this demo, in prod use env.prng())
        let r = 17i128; 
        
        // c = (g^m * r^n) mod n^2
        let gm = Self::pow_mod(g, m, nn);
        let rn = Self::pow_mod(r, n, nn);
        
        (gm * rn) % nn
    }

    pub fn decrypt(pub_key: &PaillierPublicKey, priv_key: &PaillierPrivateKey, c: i128) -> i128 {
        let n = pub_key.n;
        let nn = pub_key.nn;
        
        // L(u) = (u - 1) / n
        // m = L(c^lambda mod n^2) * mu mod n
        let u = Self::pow_mod(c, priv_key.lambda, nn);
        let l_u = (u - 1) / n;
        
        (l_u * priv_key.mu) % n
    }

    /// Additive property: E(m1 + m2) = E(m1) * E(m2) mod n^2
    pub fn add_ciphertexts(pub_key: &PaillierPublicKey, c1: i128, c2: i128) -> i128 {
        (c1 * c2) % pub_key.nn
    }

    fn pow_mod(mut base: i128, mut exp: i128, mod_val: i128) -> i128 {
        let mut res = 1;
        base %= mod_val;
        while exp > 0 {
            if exp % 2 == 1 {
                res = (res * base) % mod_val;
            }
            base = (base * base) % mod_val;
            exp /= 2;
        }
        res
    }
}
