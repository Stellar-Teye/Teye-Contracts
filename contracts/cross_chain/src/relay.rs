use soroban_sdk::{contracttype, symbol_short, BytesN, Env, Symbol};

use crate::events;

const TTL_THRESHOLD: u32 = 17_280;
const TTL_EXTEND_TO: u32 = 518_400;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateRootAnchor {
    pub root: BytesN<32>,
    pub ledger_sequence: u32,
    pub chain_id: Symbol,
    pub anchored_at: u64,
}

fn relay_root_key(chain_id: Symbol, ledger_sequence: u32) -> (Symbol, Symbol, u32) {
    (symbol_short!("RELAYROOT"), chain_id, ledger_sequence)
}

fn relay_latest_key(chain_id: Symbol) -> (Symbol, Symbol) {
    (symbol_short!("RELAYLST"), chain_id)
}

pub fn anchor_state_root(env: &Env, root: [u8; 32], chain_id: Symbol) {
    let ledger_sequence = env.ledger().sequence();
    let anchor = StateRootAnchor {
        root: BytesN::from_array(env, &root),
        ledger_sequence,
        chain_id,
        anchored_at: env.ledger().timestamp(),
    };

    let root_key = relay_root_key(anchor.chain_id.clone(), anchor.ledger_sequence);
    env.storage().persistent().set(&root_key, &anchor);
    env.storage()
        .persistent()
        .extend_ttl(&root_key, TTL_THRESHOLD, TTL_EXTEND_TO);

    let latest_key = relay_latest_key(anchor.chain_id.clone());
    env.storage()
        .persistent()
        .set(&latest_key, &anchor.ledger_sequence);
    env.storage()
        .persistent()
        .extend_ttl(&latest_key, TTL_THRESHOLD, TTL_EXTEND_TO);

    events::emit_state_root_anchored(
        env,
        anchor.chain_id.clone(),
        anchor.root.clone(),
        anchor.ledger_sequence,
    );
}

pub fn get_anchored_root(
    env: &Env,
    chain_id: Symbol,
    ledger_sequence: u32,
) -> Option<StateRootAnchor> {
    let key = relay_root_key(chain_id, ledger_sequence);
    let anchor: Option<StateRootAnchor> = env.storage().persistent().get(&key);
    if anchor.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
    anchor
}

pub fn get_latest_root(env: &Env, chain_id: Symbol) -> Option<StateRootAnchor> {
    let latest_key = relay_latest_key(chain_id.clone());
    let latest_seq: Option<u32> = env.storage().persistent().get(&latest_key);
    if latest_seq.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&latest_key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }

    match latest_seq {
        Some(seq) => get_anchored_root(env, chain_id, seq),
        None => None,
    }
}

pub fn sync_roots(env: &Env, roots: soroban_sdk::Vec<StateRootAnchor>) {
    env.current_contract_address().require_auth();

    for anchor in roots.iter() {
        let root_key = relay_root_key(anchor.chain_id.clone(), anchor.ledger_sequence);
        env.storage().persistent().set(&root_key, &anchor);
        env.storage()
            .persistent()
            .extend_ttl(&root_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        let latest_key = relay_latest_key(anchor.chain_id.clone());
        let current_latest: Option<u32> = env.storage().persistent().get(&latest_key);
        if current_latest
            .map(|s| anchor.ledger_sequence >= s)
            .unwrap_or(true)
        {
            env.storage()
                .persistent()
                .set(&latest_key, &anchor.ledger_sequence);
            env.storage()
                .persistent()
                .extend_ttl(&latest_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        }

        events::emit_state_root_anchored(
            env,
            anchor.chain_id.clone(),
            anchor.root.clone(),
            anchor.ledger_sequence,
        );
    }
}

pub fn is_finalized(env: &Env, anchor: &StateRootAnchor, finality_depth: u32) -> bool {
    let current = env.ledger().sequence();
    current.saturating_sub(anchor.ledger_sequence) >= finality_depth
}
