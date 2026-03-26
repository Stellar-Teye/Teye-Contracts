#![allow(clippy::arithmetic_side_effects)]

extern crate alloc;
use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Share {
    pub x: u8,
    pub y: [u8; 32],
}

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    let mut i = 0;
    while i < 8 {
        if (b & 1) != 0 {
            p ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
        i += 1;
    }
    p
}

fn gf_pow(mut a: u8, mut e: u8) -> u8 {
    let mut out = 1u8;
    while e > 0 {
        if (e & 1) == 1 {
            out = gf_mul(out, a);
        }
        a = gf_mul(a, a);
        e >>= 1;
    }
    out
}

fn gf_inv(a: u8) -> u8 {
    if a == 0 {
        0
    } else {
        gf_pow(a, 254)
    }
}

fn gf_div(a: u8, b: u8) -> u8 {
    if b == 0 {
        0
    } else {
        gf_mul(a, gf_inv(b))
    }
}

pub fn split(secret: [u8; 32], threshold: u8, n: u8, seed: [u8; 32]) -> Vec<Share> {
    let mut coeffs = [[0u8; 32]; 8];
    let mut d = 0u8;
    while d < threshold.saturating_sub(1) {
        let mut i = 0usize;
        while i < 32 {
            coeffs[d as usize][i] =
                seed[i] ^ (d.wrapping_add(1)).wrapping_mul((i as u8).wrapping_add(17));
            i += 1;
        }
        d += 1;
    }

    let mut shares = Vec::new();
    let mut x = 1u8;
    while x <= n {
        let mut y = [0u8; 32];
        let mut i = 0usize;
        while i < 32 {
            let mut acc = secret[i];
            let mut pow = x;
            let mut k = 0u8;
            while k < threshold.saturating_sub(1) {
                acc ^= gf_mul(coeffs[k as usize][i], pow);
                pow = gf_mul(pow, x);
                k += 1;
            }
            y[i] = acc;
            i += 1;
        }
        shares.push(Share { x, y });
        x = x.saturating_add(1);
    }
    shares
}

pub fn reconstruct(shares: &[Share], threshold: u8) -> Option<[u8; 32]> {
    if shares.len() < threshold as usize || threshold == 0 {
        return None;
    }

    let mut out = [0u8; 32];
    let mut i = 0usize;
    while i < 32 {
        let mut secret_byte = 0u8;
        let mut j = 0usize;
        while j < threshold as usize {
            let xj = shares[j].x;
            let yj = shares[j].y[i];

            let mut num = 1u8;
            let mut den = 1u8;
            let mut m = 0usize;
            while m < threshold as usize {
                if m != j {
                    let xm = shares[m].x;
                    num = gf_mul(num, xm);
                    den = gf_mul(den, xm ^ xj);
                }
                m += 1;
            }

            let lj = gf_div(num, den);
            secret_byte ^= gf_mul(yj, lj);
            j += 1;
        }
        out[i] = secret_byte;
        i += 1;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn reconstruct_with_threshold_shares() {
        let secret = [7u8; 32];
        let seed = [11u8; 32];
        let shares = split(secret, 3, 5, seed);
        let subset = vec![shares[0].clone(), shares[2].clone(), shares[4].clone()];
        let recovered = reconstruct(&subset, 3).unwrap();
        assert_eq!(recovered, secret);
    }
}
