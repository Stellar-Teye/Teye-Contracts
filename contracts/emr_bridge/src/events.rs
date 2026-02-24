use soroban_sdk::{symbol_short, Address, Env, String};

pub fn publish_initialized(env: &Env, admin: Address) {
    env.events().publish((symbol_short!("EMR_INIT"),), admin);
}

pub fn publish_provider_registered(env: &Env, provider_id: String, registered_by: Address) {
    env.events()
        .publish((symbol_short!("PRV_REG"), provider_id), registered_by);
}

pub fn publish_provider_status_changed(env: &Env, provider_id: String, new_status: u32) {
    env.events()
        .publish((symbol_short!("PRV_STS"), provider_id), new_status);
}

pub fn publish_data_exchanged(env: &Env, exchange_id: String, provider_id: String) {
    env.events()
        .publish((symbol_short!("DATA_EX"), exchange_id, provider_id), ());
}

pub fn publish_mapping_created(env: &Env, mapping_id: String, provider_id: String) {
    env.events()
        .publish((symbol_short!("MAP_ADD"), mapping_id), provider_id);
}

pub fn publish_sync_verified(env: &Env, verification_id: String, is_consistent: bool) {
    env.events()
        .publish((symbol_short!("SYNC_VF"), verification_id), is_consistent);
}
