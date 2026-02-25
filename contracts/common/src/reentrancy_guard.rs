use soroban_sdk::{contracterror, symbol_short, Env, Symbol};

const REENTRANCY_LOCK: Symbol = symbol_short!("REN_LOCK");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ReentrancyError {
    ReentrantCall = 99,
}

/// A scope guard that sets a reentrancy lock in instance storage on creation
/// and unsets it on drop.
pub struct ReentrancyGuard<'a> {
    env: &'a Env,
}

impl<'a> ReentrancyGuard<'a> {
    /// Creates a new reentrancy guard.
    ///
    /// # Panics
    ///
    /// Panics with `ReentrancyError::ReentrantCall` if the lock is already set.
    pub fn new(env: &'a Env) -> Self {
        let is_locked = env
            .storage()
            .instance()
            .get::<_, bool>(&REENTRANCY_LOCK)
            .unwrap_or(false);

        if is_locked {
            env.panic_with_error(ReentrancyError::ReentrantCall);
        }

        env.storage().instance().set(&REENTRANCY_LOCK, &true);

        Self { env }
    }
}

impl<'a> Drop for ReentrancyGuard<'a> {
    fn drop(&mut self) {
        self.env.storage().instance().remove(&REENTRANCY_LOCK);
    }
}
