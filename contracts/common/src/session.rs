//! Session state for progressive authorization with privilege decay.

use crate::progressive_auth::AuthLevel;
use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol, Vec};

const SESSION_KEY: Symbol = symbol_short!("SESS");
const OVERRIDE_KEY: Symbol = symbol_short!("E_OVR");
const OVERRIDE_CNT: Symbol = symbol_short!("E_OVR_C");

const TTL_THRESHOLD: u32 = 5_184_000;
const TTL_EXTEND_TO: u32 = 10_368_000;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthSession {
    pub user: Address,
    pub issued_at: u64,
    pub last_active_at: u64,
    pub expires_at: u64,
    pub max_level: AuthLevel,
    pub decay_interval_seconds: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyOverrideLog {
    pub id: u64,
    pub user: Address,
    pub reason: String,
    pub requested_level: AuthLevel,
    pub created_at: u64,
    pub requires_post_hoc_review: bool,
    pub reviewed: bool,
    pub reviewer: Option<Address>,
    pub review_notes: Option<String>,
}

fn session_storage_key(user: &Address) -> (Symbol, Address) {
    (SESSION_KEY, user.clone())
}

fn emergency_key(id: u64) -> (Symbol, u64) {
    (OVERRIDE_KEY, id)
}

fn extend_session_ttl(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn extend_override_ttl(env: &Env, key: &(Symbol, u64)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Create or refresh an auth session.
pub fn start_or_refresh_session(
    env: &Env,
    user: &Address,
    max_level: AuthLevel,
    ttl_seconds: u64,
    decay_interval_seconds: u64,
) -> AuthSession {
    let now = env.ledger().timestamp();
    let key = session_storage_key(user);

    let mut session: AuthSession = env.storage().persistent().get(&key).unwrap_or(AuthSession {
        user: user.clone(),
        issued_at: now,
        last_active_at: now,
        expires_at: now.saturating_add(ttl_seconds),
        max_level: max_level.clone(),
        decay_interval_seconds,
    });

    session.last_active_at = now;
    session.expires_at = now.saturating_add(ttl_seconds);
    session.max_level = max_level;
    session.decay_interval_seconds = decay_interval_seconds;

    env.storage().persistent().set(&key, &session);
    extend_session_ttl(env, &key);
    session
}

pub fn get_session(env: &Env, user: &Address) -> Option<AuthSession> {
    let key = session_storage_key(user);
    let session: Option<AuthSession> = env.storage().persistent().get(&key);
    if session.is_some() {
        extend_session_ttl(env, &key);
    }
    session
}

/// Compute effective level after privilege decay.
pub fn effective_level(env: &Env, session: &AuthSession) -> Option<AuthLevel> {
    let now = env.ledger().timestamp();
    if now > session.expires_at {
        return None;
    }

    if session.decay_interval_seconds == 0 {
        return Some(session.max_level.clone());
    }

    let elapsed = now.saturating_sub(session.issued_at);
    let drops = elapsed / session.decay_interval_seconds;

    let level = match (session.max_level.clone(), drops) {
        (AuthLevel::Level4, 0) => AuthLevel::Level4,
        (AuthLevel::Level4, 1) => AuthLevel::Level3,
        (AuthLevel::Level4, 2) => AuthLevel::Level2,
        (AuthLevel::Level4, _) => AuthLevel::Level1,
        (AuthLevel::Level3, 0) => AuthLevel::Level3,
        (AuthLevel::Level3, 1) => AuthLevel::Level2,
        (AuthLevel::Level3, _) => AuthLevel::Level1,
        (AuthLevel::Level2, 0) => AuthLevel::Level2,
        (AuthLevel::Level2, _) => AuthLevel::Level1,
        (AuthLevel::Level1, _) => AuthLevel::Level1,
    };

    Some(level)
}

/// Validate an active session meets required level.
pub fn validate_session_level(env: &Env, user: &Address, required: AuthLevel) -> bool {
    let session = match get_session(env, user) {
        Some(s) => s,
        None => return false,
    };

    let effective = match effective_level(env, &session) {
        Some(level) => level,
        None => return false,
    };

    (effective as u32) >= (required as u32)
}

/// Emergency override: elevated action with mandatory post-hoc review tracking.
pub fn start_emergency_override(
    env: &Env,
    user: &Address,
    reason: String,
    requested_level: AuthLevel,
) -> EmergencyOverrideLog {
    let now = env.ledger().timestamp();
    let id: u64 = env.storage().instance().get(&OVERRIDE_CNT).unwrap_or(0) + 1;
    env.storage().instance().set(&OVERRIDE_CNT, &id);

    let record = EmergencyOverrideLog {
        id,
        user: user.clone(),
        reason,
        requested_level,
        created_at: now,
        requires_post_hoc_review: true,
        reviewed: false,
        reviewer: None,
        review_notes: None,
    };

    let key = emergency_key(id);
    env.storage().persistent().set(&key, &record);
    extend_override_ttl(env, &key);

    record
}

pub fn review_emergency_override(
    env: &Env,
    override_id: u64,
    reviewer: &Address,
    review_notes: String,
) -> Option<EmergencyOverrideLog> {
    let key = emergency_key(override_id);
    let mut record: EmergencyOverrideLog = env.storage().persistent().get(&key)?;

    record.reviewed = true;
    record.reviewer = Some(reviewer.clone());
    record.review_notes = Some(review_notes);

    env.storage().persistent().set(&key, &record);
    extend_override_ttl(env, &key);
    Some(record)
}

pub fn get_emergency_override(env: &Env, override_id: u64) -> Option<EmergencyOverrideLog> {
    let key = emergency_key(override_id);
    let record: Option<EmergencyOverrideLog> = env.storage().persistent().get(&key);
    if record.is_some() {
        extend_override_ttl(env, &key);
    }
    record
}

pub fn list_pending_emergency_overrides(env: &Env) -> Vec<u64> {
    let total: u64 = env.storage().instance().get(&OVERRIDE_CNT).unwrap_or(0);
    let mut pending = Vec::new(env);
    for id in 1..=total {
        if let Some(record) = get_emergency_override(env, id) {
            if !record.reviewed {
                pending.push_back(id);
            }
        }
    }
    pending
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        contract, contractimpl,
        testutils::{Address as _, Ledger},
        Env,
    };

    #[contract]
    struct TestContract;

    #[contractimpl]
    impl TestContract {
        pub fn noop(_env: Env) {}
    }

    #[test]
    fn session_privileges_decay_over_time() {
        let env = Env::default();
        let contract_id = env.register(TestContract, ());
        let user = Address::generate(&env);

        let session = env.as_contract(&contract_id, || {
            start_or_refresh_session(&env, &user, AuthLevel::Level4, 3_600, 300)
        });
        assert_eq!(effective_level(&env, &session), Some(AuthLevel::Level4));

        env.ledger().set_timestamp(env.ledger().timestamp() + 301);
        let updated = env
            .as_contract(&contract_id, || get_session(&env, &user))
            .expect("session should exist");
        assert_eq!(effective_level(&env, &updated), Some(AuthLevel::Level3));

        env.ledger().set_timestamp(env.ledger().timestamp() + 901);
        let updated = env
            .as_contract(&contract_id, || get_session(&env, &user))
            .expect("session should exist");
        assert_eq!(effective_level(&env, &updated), Some(AuthLevel::Level1));
    }

    #[test]
    fn emergency_override_requires_review_tracking() {
        let env = Env::default();
        let contract_id = env.register(TestContract, ());
        let user = Address::generate(&env);
        let reviewer = Address::generate(&env);

        let record = env.as_contract(&contract_id, || {
            start_emergency_override(
                &env,
                &user,
                String::from_str(&env, "patient critical event"),
                AuthLevel::Level4,
            )
        });

        assert!(record.requires_post_hoc_review);
        assert!(!record.reviewed);

        let reviewed = env
            .as_contract(&contract_id, || {
                review_emergency_override(
                    &env,
                    record.id,
                    &reviewer,
                    String::from_str(&env, "approved and documented"),
                )
            })
            .expect("record should exist");

        assert!(reviewed.reviewed);
        assert_eq!(reviewed.reviewer, Some(reviewer));
    }
}
