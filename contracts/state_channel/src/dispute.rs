use soroban_sdk::{Env, Address, symbol_short};
use teye_common::{CommonError, ChannelStatus};
use crate::channel::Channel;

const CHALLENGE_PERIOD: u64 = 86400; // 1 day in seconds

pub fn unilateral_close(env: &Env, channel_id: u64, closer: Address) -> Result<(), CommonError> {
    let key = (symbol_short!("CHAN"), channel_id);
    let mut channel: Channel = env.storage().persistent().get(&key).ok_or(CommonError::RecordNotFound)?;

    if channel.status != ChannelStatus::Open {
        return Err(CommonError::InvalidChannelState);
    }

    if closer != channel.patient && closer != channel.provider {
        return Err(CommonError::AccessDenied);
    }

    channel.status = ChannelStatus::Disputed;
    channel.challenge_end = Some(env.ledger().timestamp() + CHALLENGE_PERIOD);
    env.storage().persistent().set(&key, &channel);

    Ok(())
}

pub fn submit_fraud_proof(
    env: &Env,
    channel_id: u64,
    nonce: u64,
    balance: u64,
    _sig: soroban_sdk::BytesN<64>,
) -> Result<(), CommonError> {
    let key = (symbol_short!("CHAN"), channel_id);
    let mut channel: Channel = env.storage().persistent().get(&key).ok_or(CommonError::RecordNotFound)?;

    if channel.status != ChannelStatus::Disputed {
        return Err(CommonError::InvalidChannelState);
    }

    if let Some(end) = channel.challenge_end {
        if env.ledger().timestamp() > end {
            return Err(CommonError::InvalidChannelState);
        }
    }

    // Verify signature of the state transition
    // verify_state_sig(env, &channel, nonce, balance, sig)?;

    if nonce > channel.nonce {
        channel.nonce = nonce;
        channel.balance = balance;
        // Optionally reward the fraud prover or just update to the latest valid state
        env.storage().persistent().set(&key, &channel);
    }

    Ok(())
}
