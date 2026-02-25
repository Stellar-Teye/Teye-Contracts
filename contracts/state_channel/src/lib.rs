#![no_std]

use common::CommonError;
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};
use teye_common as common;

pub mod channel;
pub mod dispute;
pub mod settlement;

#[cfg(test)]
mod test;

const VISION_RECORDS: Symbol = symbol_short!("V_REC");
const ADMIN: Symbol = symbol_short!("ADMIN");

#[contract]
pub struct StateChannelContract;

#[contractimpl]
impl StateChannelContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        vision_records: Address,
    ) -> Result<(), CommonError> {
        if env.storage().instance().has(&ADMIN) {
            return Err(CommonError::AlreadyInitialized);
        }
        env.storage().instance().set(&ADMIN, &admin);
        env.storage()
            .instance()
            .set(&VISION_RECORDS, &vision_records);
        Ok(())
    }

    pub fn open_channel(
        env: Env,
        patient: Address,
        provider: Address,
        capacity: u64,
    ) -> Result<u64, CommonError> {
        patient.require_auth();
        channel::open(&env, patient, provider, capacity)
    }

    pub fn cooperative_close(
        env: Env,
        channel_id: u64,
        latest_balance: u64,
        latest_nonce: u64,
        patient_sig: soroban_sdk::BytesN<64>,
        provider_sig: soroban_sdk::BytesN<64>,
    ) -> Result<(), CommonError> {
        channel::cooperative_close(
            &env,
            channel_id,
            latest_balance,
            latest_nonce,
            patient_sig,
            provider_sig,
        )
    }

    pub fn unilateral_close(env: Env, channel_id: u64, closer: Address) -> Result<(), CommonError> {
        closer.require_auth();
        dispute::unilateral_close(&env, channel_id, closer)
    }

    pub fn submit_fraud_proof(
        env: Env,
        channel_id: u64,
        invalid_state_nonce: u64,
        invalid_state_balance: u64,
        signature: soroban_sdk::BytesN<64>,
    ) -> Result<(), CommonError> {
        dispute::submit_fraud_proof(
            &env,
            channel_id,
            invalid_state_nonce,
            invalid_state_balance,
            signature,
        )
    }

    pub fn settle(env: Env, channel_id: u64) -> Result<(), CommonError> {
        settlement::settle(&env, channel_id)
    }

    pub fn rebalance(
        env: Env,
        channel_id: u64,
        new_capacity: u64,
        patient_sig: soroban_sdk::BytesN<64>,
        provider_sig: soroban_sdk::BytesN<64>,
    ) -> Result<(), CommonError> {
        channel::rebalance(&env, channel_id, new_capacity, patient_sig, provider_sig)
    }

    pub fn open_multi_hop(
        env: Env,
        patient: Address,
        provider: Address,
        intermediary: Address,
        capacity: u64,
    ) -> Result<u64, CommonError> {
        patient.require_auth();
        channel::open_multi_hop(&env, patient, provider, intermediary, capacity)
    }
}
