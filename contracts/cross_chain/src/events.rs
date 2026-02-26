#![allow(deprecated)] // events().publish migration tracked separately

<<<<<<< HEAD
use soroban_sdk::{symbol_short, Address, Bytes, Env, String};
=======
use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, Env, String, Symbol};
>>>>>>> upstream/master

pub fn publish_initialized(env: &Env, admin: Address) {
    env.events().publish((symbol_short!("INIT"),), admin);
}

pub fn publish_relayer_added(env: &Env, relayer: Address) {
    env.events().publish((symbol_short!("RELAYER"),), relayer);
}

pub fn publish_identity_mapped(
    env: &Env,
    chain: String,
    foreign_addr: String,
    local_addr: Address,
) {
    env.events()
        .publish((symbol_short!("ID_MAP"), chain, foreign_addr), local_addr);
}

pub fn publish_message_processed(env: &Env, chain: String, message_id: Bytes, success: bool) {
    env.events()
        .publish((symbol_short!("PROC_MSG"), chain, message_id), success);
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordExported {
    pub record_id: BytesN<32>,
    pub state_root: BytesN<32>,
    pub selective: bool,
    pub timestamp: u64,
}

pub fn emit_record_exported(
    env: &Env,
    record_id: BytesN<32>,
    state_root: BytesN<32>,
    selective: bool,
    timestamp: u64,
) {
    let event = RecordExported {
        record_id,
        state_root,
        selective,
        timestamp,
    };
    env.events().publish((symbol_short!("REC_EXP"),), event);
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordImported {
    pub record_id: BytesN<32>,
    pub source_chain: Symbol,
    pub verified: bool,
}

pub fn emit_record_imported(
    env: &Env,
    record_id: BytesN<32>,
    source_chain: Symbol,
    verified: bool,
) {
    let event = RecordImported {
        record_id,
        source_chain,
        verified,
    };
    env.events().publish((symbol_short!("REC_IMP"),), event);
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateRootAnchored {
    pub chain_id: Symbol,
    pub root: BytesN<32>,
    pub ledger_sequence: u32,
}

pub fn emit_state_root_anchored(
    env: &Env,
    chain_id: Symbol,
    root: BytesN<32>,
    ledger_sequence: u32,
) {
    let event = StateRootAnchored {
        chain_id,
        root,
        ledger_sequence,
    };
    env.events().publish((symbol_short!("ROOT_ANC"),), event);
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainReorgDetected {
    pub chain_id: Symbol,
    pub suspect_root: BytesN<32>,
    pub finality_depth: u32,
}

pub fn emit_chain_reorg_detected(
    env: &Env,
    chain_id: Symbol,
    suspect_root: BytesN<32>,
    finality_depth: u32,
) {
    let event = ChainReorgDetected {
        chain_id,
        suspect_root,
        finality_depth,
    };
    env.events().publish((symbol_short!("REORG"),), event);
}
