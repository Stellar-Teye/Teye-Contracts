use soroban_sdk::{Bytes, BytesN, Env};

fn hash_bytes(env: &Env, bytes: &Bytes) -> BytesN<32> {
    env.crypto().sha256(bytes).into()
}

pub fn derive_child_key(
    env: &Env,
    parent_key: &BytesN<32>,
    parent_chain: &BytesN<32>,
    index: u32,
    hardened: bool,
) -> (BytesN<32>, BytesN<32>) {
    let mut data = Bytes::new(env);
    data.extend_from_array(&parent_key.to_array());
    data.extend_from_array(&parent_chain.to_array());
    data.extend_from_array(&index.to_be_bytes());
    data.extend_from_array(&[if hardened { 1 } else { 0 }]);

    let child_key = hash_bytes(env, &data);

    let mut chain_data = Bytes::new(env);
    chain_data.extend_from_array(&parent_chain.to_array());
    chain_data.extend_from_array(&index.to_be_bytes());
    chain_data.extend_from_array(&[if hardened { 1 } else { 0 }]);
    chain_data.extend_from_array(b"chain");

    let child_chain = hash_bytes(env, &chain_data);

    (child_key, child_chain)
}

pub fn derive_record_key(env: &Env, key_bytes: &BytesN<32>, record_id: u64) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.extend_from_array(&key_bytes.to_array());
    data.extend_from_array(&record_id.to_be_bytes());
    data.extend_from_array(b"record");
    hash_bytes(env, &data)
}
