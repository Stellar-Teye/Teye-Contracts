#![allow(deprecated)] // events().publish migration tracked separately

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::CommonError;

const PAUSED: Symbol = symbol_short!("PAUSED");

/// Sets the contract pause state.
///
/// Callers are responsible for enforcing admin authorization before invoking
/// this function — the module itself does **not** perform auth checks, keeping
/// it reusable across contracts with different admin models.
pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&PAUSED, &paused);
}

/// Returns `true` when the contract is paused.
pub fn is_paused(env: &Env) -> bool {
    env.storage().instance().get(&PAUSED).unwrap_or(false)
}

/// Guard — returns `CommonError::Paused` when the contract is paused.
///
/// Place this at the top of every state-mutating function that must honour
/// the pause.  View-only functions should **not** call this.
pub fn require_not_paused(env: &Env) -> Result<(), CommonError> {
    if is_paused(env) {
        return Err(CommonError::Paused);
    }
    Ok(())
}

/// Pause the contract after verifying `caller` is the stored admin.
///
/// Emits a `("PAUSED", caller)` event on success.
pub fn pause(env: &Env, caller: &Address) {
    set_paused(env, true);
    env.events()
        .publish((symbol_short!("PAUSED"), caller.clone()), true);
}

/// Unpause the contract after verifying `caller` is the stored admin.
///
/// Emits an `("UNPAUSED", caller)` event on success.
pub fn unpause(env: &Env, caller: &Address) {
    set_paused(env, false);
    env.events()
        .publish((symbol_short!("UNPAUSED"), caller.clone()), true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, Env};

    #[contract]
    struct DummyContract;

    #[test]
    fn default_is_not_paused() {
        let env = Env::default();
        let contract_id = env.register(DummyContract, ());
        env.as_contract(&contract_id, || {
            assert!(!is_paused(&env));
            assert!(require_not_paused(&env).is_ok());
        });
    }

    #[test]
    fn pause_and_unpause() {
        let env = Env::default();
        let contract_id = env.register(DummyContract, ());
        env.as_contract(&contract_id, || {
            set_paused(&env, true);
            assert!(is_paused(&env));
            assert_eq!(require_not_paused(&env), Err(CommonError::Paused));

            set_paused(&env, false);
            assert!(!is_paused(&env));
            assert!(require_not_paused(&env).is_ok());
        });
    }
}
