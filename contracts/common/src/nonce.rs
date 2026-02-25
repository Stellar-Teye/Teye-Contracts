//! # Nonce-based Replay Protection
//!
//! Provides per-sender strictly-monotonic nonces for cross-contract calls.
//! Each (contract, sender) pair maintains an independent counter in persistent
//! storage.  A message is only accepted when its nonce equals the current
//! expected value; on success the counter is atomically incremented.
//!
//! ## Usage pattern
//!
//! **Sender side** — call `get_and_increment_nonce` *before* dispatching:
//! ```ignore
//! let nonce = nonce::get_and_increment_nonce(&env, &caller)?;
//! cross_contract_client.some_method(&nonce, ...);
//! ```
//!
//! **Recipient side** — call `validate_and_increment_nonce` *first*:
//! ```ignore
//! nonce::validate_and_increment_nonce(&env, &request.sender, request.nonce)?;
//! // ... rest of logic
//! ```

use soroban_sdk::{contracttype, Address, Env};

use crate::CommonError;

// ── Storage key ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum NonceKey {
    Nonce(Address),
}

// ── TTL constants (mirror common convention) ─────────────────────────────────

const TTL_THRESHOLD: u32 = 5_184_000;
const TTL_EXTEND_TO: u32 = 10_368_000;

// ── Internal helpers ─────────────────────────────────────────────────────────

fn nonce_key(sender: &Address) -> NonceKey {
    NonceKey::Nonce(sender.clone())
}

fn load_nonce(env: &Env, sender: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&nonce_key(sender))
        .unwrap_or(0u64)
}

fn store_nonce(env: &Env, sender: &Address, value: u64) {
    let key = nonce_key(sender);
    env.storage().persistent().set(&key, &value);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Return the current nonce for `sender` without modifying state.
///
/// New senders always start at `0`.
pub fn current_nonce(env: &Env, sender: &Address) -> u64 {
    load_nonce(env, sender)
}

/// **Sender side.** Read the current nonce, increment it atomically, and
/// return the value that was read (i.e. the nonce to embed in the outgoing
/// message).
///
/// Returns [`CommonError::NonceOverflow`] if the counter would exceed `u64::MAX`.
pub fn get_and_increment_nonce(env: &Env, sender: &Address) -> Result<u64, CommonError> {
    let current = load_nonce(env, sender);
    let next = current.checked_add(1).ok_or(CommonError::NonceOverflow)?;
    store_nonce(env, sender, next);
    Ok(current)
}

/// **Recipient side.** Validate that `provided` equals the expected nonce for
/// `sender`, then atomically increment the stored counter.
///
/// # Errors
/// - [`CommonError::InvalidNonce`] — `provided` does not match the expected value.
/// - [`CommonError::NonceOverflow`] — the counter is already at `u64::MAX`.
pub fn validate_and_increment_nonce(
    env: &Env,
    sender: &Address,
    provided: u64,
) -> Result<(), CommonError> {
    let expected = load_nonce(env, sender);
    if provided != expected {
        return Err(CommonError::InvalidNonce);
    }
    let next = expected.checked_add(1).ok_or(CommonError::NonceOverflow)?;
    store_nonce(env, sender, next);
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _, Env};

    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {}

    fn with_contract_env<F: FnOnce(&Env)>(f: F) {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            f(&env);
        });
    }

    #[test]
    fn new_sender_starts_at_zero() {
        with_contract_env(|env| {
            let sender = Address::generate(env);
            assert_eq!(current_nonce(env, &sender), 0);
        });
    }

    #[test]
    fn current_nonce_is_read_only() {
        with_contract_env(|env| {
            let sender = Address::generate(env);
            assert_eq!(current_nonce(env, &sender), 0);
            assert_eq!(current_nonce(env, &sender), 0);
        });
    }

    #[test]
    fn get_and_increment_returns_current_then_advances() {
        with_contract_env(|env| {
            let sender = Address::generate(env);
            assert_eq!(get_and_increment_nonce(env, &sender).unwrap(), 0);
            assert_eq!(get_and_increment_nonce(env, &sender).unwrap(), 1);
            assert_eq!(get_and_increment_nonce(env, &sender).unwrap(), 2);
            assert_eq!(current_nonce(env, &sender), 3);
        });
    }

    #[test]
    fn validate_and_increment_accepts_correct_nonce() {
        with_contract_env(|env| {
            let sender = Address::generate(env);
            validate_and_increment_nonce(env, &sender, 0).unwrap();
            assert_eq!(current_nonce(env, &sender), 1);
            validate_and_increment_nonce(env, &sender, 1).unwrap();
            assert_eq!(current_nonce(env, &sender), 2);
        });
    }

    #[test]
    fn replay_at_nonce_zero_is_rejected() {
        with_contract_env(|env| {
            let sender = Address::generate(env);
            validate_and_increment_nonce(env, &sender, 0).unwrap();
            let err = validate_and_increment_nonce(env, &sender, 0).unwrap_err();
            assert_eq!(err, CommonError::InvalidNonce);
            assert_eq!(err as u32, 31);
        });
    }

    #[test]
    fn stale_nonce_after_advances_is_rejected() {
        with_contract_env(|env| {
            let sender = Address::generate(env);
            validate_and_increment_nonce(env, &sender, 0).unwrap();
            validate_and_increment_nonce(env, &sender, 1).unwrap();
            let err = validate_and_increment_nonce(env, &sender, 0).unwrap_err();
            assert_eq!(err, CommonError::InvalidNonce);
        });
    }

    #[test]
    fn future_nonce_with_gap_is_rejected() {
        with_contract_env(|env| {
            let sender = Address::generate(env);
            let err = validate_and_increment_nonce(env, &sender, 5).unwrap_err();
            assert_eq!(err, CommonError::InvalidNonce);
            assert_eq!(current_nonce(env, &sender), 0);
        });
    }

    #[test]
    fn senders_have_independent_counters() {
        with_contract_env(|env| {
            let alice = Address::generate(env);
            let bob = Address::generate(env);
            validate_and_increment_nonce(env, &alice, 0).unwrap();
            validate_and_increment_nonce(env, &alice, 1).unwrap();
            assert_eq!(current_nonce(env, &alice), 2);
            assert_eq!(current_nonce(env, &bob), 0);
            validate_and_increment_nonce(env, &bob, 0).unwrap();
            assert_eq!(current_nonce(env, &bob), 1);
        });
    }

    #[test]
    fn error_codes_are_stable() {
        with_contract_env(|env| {
            assert_eq!(CommonError::InvalidNonce as u32, 31);
            assert_eq!(CommonError::NonceOverflow as u32, 32);
        });
    }
}
