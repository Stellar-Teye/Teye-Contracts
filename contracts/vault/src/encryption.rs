#![allow(clippy::arithmetic_side_effects)]

use soroban_sdk::{BytesN, Env, String};

pub fn seal_reference(env: &Env, data_ref_hash: &String, key_commitment: &BytesN<32>) -> BytesN<32> {
    let mut payload = soroban_sdk::Bytes::new(env);
    payload.append(&data_ref_hash.to_bytes());
    payload.append(&soroban_sdk::Bytes::from_slice(env, &key_commitment.to_array()));
    env.crypto().sha256(&payload).into()
}
