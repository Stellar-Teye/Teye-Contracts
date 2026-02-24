use crate::{
    events,
    rbac::{self, Permission},
    ContractError,
};
use common::admin_tiers::{self, AdminTier};
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

// ── Types ─────────────────────────────────────────────────────

/// Defines the scope of the pause mechanism
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PauseScope {
    /// Halts writing state to the entire contract
    Global,
    /// Halts writing state targeted to a specific function execution
    Function(Symbol),
}

// ── Storage Keys ─────────────────────────────────────────────

pub fn global_pause_key() -> Symbol {
    symbol_short!("P_GLOB")
}

pub fn function_pause_key(func: &Symbol) -> (Symbol, Symbol) {
    (symbol_short!("P_FUNC"), func.clone())
}

// ── Core Logistics ───────────────────────────────────────────

/// Asserts the specified scope is currently active and not halted. Automatically evaluates Global halts simultaneously.
pub fn require_not_paused(env: &Env, scope: &PauseScope) -> Result<(), ContractError> {
    // 1. Check Global
    if env
        .storage()
        .instance()
        .get(&global_pause_key())
        .unwrap_or(false)
    {
        return Err(ContractError::Paused);
    }

    // 2. Check Specific Scope
    if let PauseScope::Function(func_name) = scope {
        if env
            .storage()
            .instance()
            .get(&function_pause_key(func_name))
            .unwrap_or(false)
        {
            return Err(ContractError::Paused);
        }
    }

    Ok(())
}

/// Engages a circuit breaker for the specified scope.
/// Requires at least `OperatorAdmin` tier, or the existing SystemAdmin RBAC permission.
pub fn pause_contract(env: &Env, caller: &Address, scope: PauseScope) -> Result<(), ContractError> {
    let has_tier = admin_tiers::require_tier(env, caller, &AdminTier::OperatorAdmin);
    let has_rbac = rbac::has_permission(env, caller, &Permission::SystemAdmin);
    if !has_tier && !has_rbac {
        return Err(ContractError::Unauthorized);
    }

    match &scope {
        PauseScope::Global => {
            env.storage().instance().set(&global_pause_key(), &true);
        }
        PauseScope::Function(func_name) => {
            env.storage()
                .instance()
                .set(&function_pause_key(func_name), &true);
        }
    }

    events::publish_contract_paused(env, caller.clone(), scope);

    Ok(())
}

/// Resumes operation of a circuit breaker for the specified scope.
/// Requires at least `OperatorAdmin` tier, or the existing SystemAdmin RBAC permission.
pub fn resume_contract(
    env: &Env,
    caller: &Address,
    scope: PauseScope,
) -> Result<(), ContractError> {
    let has_tier = admin_tiers::require_tier(env, caller, &AdminTier::OperatorAdmin);
    let has_rbac = rbac::has_permission(env, caller, &Permission::SystemAdmin);
    if !has_tier && !has_rbac {
        return Err(ContractError::Unauthorized);
    }

    match &scope {
        PauseScope::Global => {
            env.storage().instance().set(&global_pause_key(), &false);
        }
        PauseScope::Function(func_name) => {
            env.storage()
                .instance()
                .set(&function_pause_key(func_name), &false);
        }
    }

    events::publish_contract_resumed(env, caller.clone(), scope);

    Ok(())
}
