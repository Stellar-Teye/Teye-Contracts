use soroban_sdk::{Bytes, BytesN, Env};

/// Builds a canonical message for a grant access meta-transaction.
///
/// Message format: "grant_access" || patient_pubkey(32) || grantee_id(32)
///                 || level(4 BE) || expires_at(8 BE) || nonce(8 BE)
pub fn build_grant_message(
    env: &Env,
    patient_pubkey: &BytesN<32>,
    grantee_id: &BytesN<32>,
    level: u32,
    expires_at: u64,
    nonce: u64,
) -> Bytes {
    let mut msg = Bytes::new(env);
    msg.append(&Bytes::from_slice(env, b"grant_access"));
    msg.append(&Bytes::from_slice(env, &patient_pubkey.to_array()));
    msg.append(&Bytes::from_slice(env, &grantee_id.to_array()));
    msg.append(&Bytes::from_slice(env, &level.to_be_bytes()));
    msg.append(&Bytes::from_slice(env, &expires_at.to_be_bytes()));
    msg.append(&Bytes::from_slice(env, &nonce.to_be_bytes()));
    msg
}

/// Verifies an ed25519 signature over a message.
///
/// Panics if the signature is invalid (Soroban host behavior).
pub fn verify_meta_signature(
    env: &Env,
    public_key: &BytesN<32>,
    message: &Bytes,
    signature: &BytesN<64>,
) {
    env.crypto()
        .ed25519_verify(public_key, message, signature);
}
