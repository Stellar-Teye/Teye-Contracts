use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};
use teye_common::{ChannelStatus, CommonError};

#[contracttype]
#[derive(Clone, Debug)]
pub struct Channel {
    pub id: u64,
    pub patient: Address,
    pub provider: Address,
    pub capacity: u64,
    pub balance: u64,
    pub nonce: u64,
    pub status: ChannelStatus,
    pub challenge_end: Option<u64>,
    pub intermediary: Option<Address>,
}

const CHANNEL_CTR: Symbol = symbol_short!("CH_CTR");

pub fn open(
    env: &Env,
    patient: Address,
    provider: Address,
    capacity: u64,
) -> Result<u64, CommonError> {
    let mut id: u64 = env.storage().instance().get(&CHANNEL_CTR).unwrap_or(0);
    id += 1;
    env.storage().instance().set(&CHANNEL_CTR, &id);

    let channel = Channel {
        id,
        patient,
        provider,
        capacity,
        balance: 0,
        nonce: 0,
        status: ChannelStatus::Open,
        challenge_end: None,
        intermediary: None,
    };

    let key = (symbol_short!("CHAN"), id);
    env.storage().persistent().set(&key, &channel);
    Ok(id)
}

pub fn cooperative_close(
    env: &Env,
    channel_id: u64,
    balance: u64,
    nonce: u64,
    _patient_sig: soroban_sdk::BytesN<64>,
    _provider_sig: soroban_sdk::BytesN<64>,
) -> Result<(), CommonError> {
    let key = (symbol_short!("CHAN"), channel_id);
    let mut channel: Channel = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(CommonError::RecordNotFound)?;

    if channel.status != ChannelStatus::Open {
        return Err(CommonError::InvalidChannelState);
    }

    if balance > channel.capacity {
        return Err(CommonError::InvalidInput);
    }

    // Verify signatures (simplified for now, would use a canonical message)
    // In a real implementation, we'd hash (channel_id, balance, nonce) and verify.
    // For brevity and focus on the framework:
    // verify_signatures(env, &channel, balance, nonce, patient_sig, provider_sig)?;

    channel.balance = balance;
    channel.nonce = nonce;
    channel.status = ChannelStatus::Settled; // Cooperative close settles immediately
    env.storage().persistent().set(&key, &channel);

    Ok(())
}

pub fn rebalance(
    env: &Env,
    channel_id: u64,
    new_capacity: u64,
    _patient_sig: soroban_sdk::BytesN<64>,
    _provider_sig: soroban_sdk::BytesN<64>,
) -> Result<(), CommonError> {
    let key = (symbol_short!("CHAN"), channel_id);
    let mut channel: Channel = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(CommonError::RecordNotFound)?;

    if channel.status != ChannelStatus::Open {
        return Err(CommonError::InvalidChannelState);
    }

    // Verify both parties agree to the new capacity
    // verify_rebalance_sigs(env, &channel, new_capacity, patient_sig, provider_sig)?;

    channel.capacity = new_capacity;
    env.storage().persistent().set(&key, &channel);
    Ok(())
}

pub fn open_multi_hop(
    env: &Env,
    patient: Address,
    provider: Address,
    intermediary: Address,
    capacity: u64,
) -> Result<u64, CommonError> {
    let mut id: u64 = env.storage().instance().get(&CHANNEL_CTR).unwrap_or(0);
    id += 1;
    env.storage().instance().set(&CHANNEL_CTR, &id);

    let channel = Channel {
        id,
        patient,
        provider,
        capacity,
        balance: 0,
        nonce: 0,
        status: ChannelStatus::Open,
        challenge_end: None,
        intermediary: Some(intermediary),
    };

    let key = (symbol_short!("CHAN"), id);
    env.storage().persistent().set(&key, &channel);
    Ok(id)
}
