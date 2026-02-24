use soroban_sdk::{symbol_short, Address, Bytes, Env, String};

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
