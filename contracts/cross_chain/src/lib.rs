#![no_std]

pub mod events;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Bytes, Env, String, Symbol,
};

/// Storage keys
const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");

/// Represents a validated message from a foreign chain
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossChainMessage {
    pub source_chain: String,
    pub source_address: String,
    pub target_action: Symbol, // e.g., symbol_short!("GRANT")
    pub payload: Bytes,        // serialized action data
}

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CrossChainError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    AlreadyProcessed = 4,
    UnknownIdentity = 5,
    UnsupportedAction = 6,
}

#[contract]
pub struct CrossChainContract;

#[contractimpl]
impl CrossChainContract {
    /// Initialize the bridge with an administrator
    pub fn initialize(env: Env, admin: Address) -> Result<(), CrossChainError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(CrossChainError::AlreadyInitialized);
        }

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);

        events::publish_initialized(&env, admin);

        Ok(())
    }

    /// Add a trusted relayer allowed to submit cross-chain messages
    pub fn add_relayer(env: Env, caller: Address, relayer: Address) -> Result<(), CrossChainError> {
        caller.require_auth();
        let admin: Address = env.storage().instance().get(&ADMIN).ok_or(CrossChainError::NotInitialized)?;
        if caller != admin {
            return Err(CrossChainError::Unauthorized);
        }

        let key = (symbol_short!("RELAYER"), relayer.clone());
        env.storage().persistent().set(&key, &true);

        events::publish_relayer_added(&env, relayer);

        Ok(())
    }

    /// Check if an address is a trusted relayer
    pub fn is_relayer(env: Env, address: Address) -> bool {
        let key = (symbol_short!("RELAYER"), address);
        env.storage().persistent().get(&key).unwrap_or(false)
    }

    /// Map a foreign identity to a local Soroban address
    pub fn map_identity(
        env: Env,
        caller: Address,
        foreign_chain: String,
        foreign_address: String,
        local_address: Address,
    ) -> Result<(), CrossChainError> {
        caller.require_auth();
        let admin: Address = env.storage().instance().get(&ADMIN).ok_or(CrossChainError::NotInitialized)?;
        if caller != admin {
            return Err(CrossChainError::Unauthorized); // Only admin can map identities for now
        }

        let key = (
            symbol_short!("ID_MAP"),
            foreign_chain.clone(),
            foreign_address.clone(),
        );
        env.storage().persistent().set(&key, &local_address);

        events::publish_identity_mapped(&env, foreign_chain, foreign_address, local_address);

        Ok(())
    }

    /// Get local address mapped to foreign identity
    pub fn get_local_address(
        env: Env,
        foreign_chain: String,
        foreign_address: String,
    ) -> Option<Address> {
        let key = (symbol_short!("ID_MAP"), foreign_chain, foreign_address);
        env.storage().persistent().get(&key)
    }

    /// Process a cross-chain message
    pub fn process_message(
        env: Env,
        caller: Address,
        message_id: Bytes,
        message: CrossChainMessage,
        _vision_contract: Address,
    ) -> Result<(), CrossChainError> {
        caller.require_auth();

        if !Self::is_relayer(env.clone(), caller) {
            return Err(CrossChainError::Unauthorized);
        }

        // Prevent replay attacks
        let processed_key = (symbol_short!("PROC_MSG"), message_id.clone());
        if env
            .storage()
            .persistent()
            .get::<_, bool>(&processed_key)
            .unwrap_or(false)
        {
            return Err(CrossChainError::AlreadyProcessed); // Already processed
        }

        // Mark as processed
        env.storage().persistent().set(&processed_key, &true);

        // Map foreign address to local address
        let local_patient = Self::get_local_address(
            env.clone(),
            message.source_chain.clone(),
            message.source_address.clone(),
        );

        if local_patient.is_none() {
            return Err(CrossChainError::UnknownIdentity); // Unknown foreign identity
        }

        let _patient_addr = local_patient.ok_or(CrossChainError::UnknownIdentity)?;

        // Handle the message based on target action
        if message.target_action == symbol_short!("GRANT") {
            // Unpack payload: expected grantee (Address), level (u32/AccessLevel), duration (u64)
            // Due to limitations in basic Bytes payload unpacking in this simplified example,
            // we will expect the payload to be properly formatted or use a standard structure.
            // For now, let's assume the bridge acts ALONGSIDE the user.

            // To properly grant access, the CrossChain contract must be an Admin or
            // delegated by the user on the VisionRecords contract.

            // For demonstration, let's just emit the event and assume the cross-contract
            // call is handled properly if this contract has permissions:
            // let client = VisionRecordsContractClient::new(&env, &vision_contract);
            // client.grant_access(&env.current_contract_address(), &patient_addr, &grantee, &level, &duration);

            events::publish_message_processed(&env, message_id, message.source_chain, true);
            Ok(())
        } else {
            Err(CrossChainError::UnsupportedAction) // Unsupported action
        }
    }
}

#[cfg(test)]
mod test;
