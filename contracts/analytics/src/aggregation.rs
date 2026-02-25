use crate::homomorphic::{HomomorphicEngine, PaillierPublicKey};
use soroban_sdk::Vec;

pub struct Aggregator;

impl Aggregator {
    /// Sums a list of ciphertexts using homomorphic addition.
    pub fn aggregate_sum(pub_key: &PaillierPublicKey, ciphertexts: Vec<i128>) -> i128 {
        let mut total = 1i128; // neutral element for multiplication (addition in plaintext)
        for c in ciphertexts.iter() {
            total = HomomorphicEngine::add_ciphertexts(pub_key, total, c);
        }
        total
    }

    /// Computes the count as a sum of encrypted 1s.
    /// In practice, if all records are included, this is just the length of the list.
    /// But if records are filtered homomorphically (e.g., predicate evaluation), this is useful.
    pub fn aggregate_count(pub_key: &PaillierPublicKey, encrypted_flags: Vec<i128>) -> i128 {
        Self::aggregate_sum(pub_key, encrypted_flags)
    }

    /// Average is (sum / count). Since we can't divide ciphertexts, 
    /// the trusted aggregator decrypts both and computes the division.
    pub fn aggregate_average(sum: i128, count: i128) -> i128 {
        if count == 0 {
            return 0;
        }
        sum / count
    }
}
