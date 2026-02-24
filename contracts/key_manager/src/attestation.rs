use soroban_sdk::{Bytes, BytesN, Env};

use crate::{KeyRecord, KeyStatus};

pub fn attest_record(env: &Env, record: &KeyRecord) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.extend_from_array(&record.id.to_array());
    data.extend_from_array(&[record.key_type.clone() as u8]);
    data.extend_from_array(&[record.level.clone() as u8]);
    data.extend_from_array(&record.chain_code.to_array());
    data.extend_from_array(&record.current_version.to_be_bytes());
    data.extend_from_array(&record.created_at.to_be_bytes());
    data.extend_from_array(&record.last_rotated.to_be_bytes());
    data.extend_from_array(&record.rotation_interval.to_be_bytes());
    data.extend_from_array(&record.uses.to_be_bytes());
    data.extend_from_array(&[match record.status {
        KeyStatus::Active => 1,
        KeyStatus::Revoked => 2,
    }]);
    data.extend_from_array(&record.policy.max_uses.to_be_bytes());
    data.extend_from_array(&record.policy.not_before.to_be_bytes());
    data.extend_from_array(&record.policy.not_after.to_be_bytes());
    data.extend_from_array(&record.policy.allowed_ops.len().to_be_bytes());
    env.crypto().sha256(&data).into()
}
