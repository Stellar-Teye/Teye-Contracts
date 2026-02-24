use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

// ── Storage Keys ─────────────────────────────────────────────────────────────

const ADMIN_TIER_PREFIX: Symbol = symbol_short!("ADM_TIER");
const SUPER_ADMIN: Symbol = symbol_short!("S_ADMIN");

const TTL_THRESHOLD: u32 = 5184000;
const TTL_EXTEND_TO: u32 = 10368000;

// ── Admin Tier Enum ──────────────────────────────────────────────────────────

/// Three-tier admin hierarchy with clear permission boundaries.
///
/// - `SuperAdmin`    – Can do everything: manage contracts, pause/unpause,
///                     and promote/demote other admins.
/// - `ContractAdmin` – Can manage a single contract's configuration
///                     (e.g. set reward rates, manage users) but cannot
///                     promote/demote admins.
/// - `OperatorAdmin` – Can only pause and unpause contract operations.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AdminTier {
    OperatorAdmin = 1,
    ContractAdmin = 2,
    SuperAdmin = 3,
}

impl AdminTier {
    /// Returns the numeric rank of this tier for comparison.
    pub fn rank(&self) -> u32 {
        match self {
            AdminTier::OperatorAdmin => 1,
            AdminTier::ContractAdmin => 2,
            AdminTier::SuperAdmin => 3,
        }
    }

    /// Returns true if this tier is at least as high as `min_tier`.
    pub fn has_at_least(&self, min_tier: &AdminTier) -> bool {
        self.rank() >= min_tier.rank()
    }
}

// ── Storage Helpers ──────────────────────────────────────────────────────────

fn admin_tier_key(admin: &Address) -> (Symbol, Address) {
    (ADMIN_TIER_PREFIX, admin.clone())
}

fn extend_ttl(env: &Env, key: &(Symbol, Address)) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ── Core Functions ───────────────────────────────────────────────────────────

/// Assigns an admin tier to the given address.
/// Only callable internally — callers must verify authorization beforehand.
pub fn set_admin_tier(env: &Env, admin: &Address, tier: AdminTier) {
    let key = admin_tier_key(admin);
    env.storage().persistent().set(&key, &tier);
    extend_ttl(env, &key);
}

/// Retrieves the admin tier of a given address, if any.
pub fn get_admin_tier(env: &Env, admin: &Address) -> Option<AdminTier> {
    let key = admin_tier_key(admin);
    let tier: Option<AdminTier> = env.storage().persistent().get(&key);
    if tier.is_some() {
        extend_ttl(env, &key);
    }
    tier
}

/// Removes the admin tier from a given address.
pub fn remove_admin_tier(env: &Env, admin: &Address) {
    let key = admin_tier_key(admin);
    env.storage().persistent().remove(&key);
}

/// Guard: reverts if the `caller` does not hold at least `min_tier`.
///
/// # Errors
/// Returns `false` if the caller has no admin tier or their tier is below
/// the required minimum.
pub fn require_tier(env: &Env, caller: &Address, min_tier: &AdminTier) -> bool {
    match get_admin_tier(env, caller) {
        Some(tier) => tier.has_at_least(min_tier),
        None => false,
    }
}

// ── SuperAdmin Registry ──────────────────────────────────────────────────────

/// Sets the initial super admin during contract initialization.
/// This also assigns them the SuperAdmin tier.
pub fn set_super_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&SUPER_ADMIN, admin);
    set_admin_tier(env, admin, AdminTier::SuperAdmin);
}

/// Returns the primary super admin address, if set.
pub fn get_super_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&SUPER_ADMIN)
}

// ── Promote / Demote ─────────────────────────────────────────────────────────

/// Promotes or assigns an admin to the specified tier.
///
/// Only a `SuperAdmin` may call this. The caller must have already been
/// authenticated via `require_auth()`.
///
/// Returns `true` on success, `false` if the caller is not a SuperAdmin.
pub fn promote_admin(env: &Env, caller: &Address, target: &Address, tier: AdminTier) -> bool {
    if !require_tier(env, caller, &AdminTier::SuperAdmin) {
        return false;
    }
    set_admin_tier(env, target, tier);
    true
}

/// Demotes (removes) an admin's tier entirely.
///
/// Only a `SuperAdmin` may call this. The caller must have already been
/// authenticated via `require_auth()`.
///
/// Returns `true` on success, `false` if the caller is not a SuperAdmin.
pub fn demote_admin(env: &Env, caller: &Address, target: &Address) -> bool {
    if !require_tier(env, caller, &AdminTier::SuperAdmin) {
        return false;
    }
    remove_admin_tier(env, target);
    true
}

/// Returns all admin addresses that have been assigned a tier.
/// This uses a tracked list stored alongside tier assignments.
pub fn list_admins(env: &Env) -> Vec<Address> {
    let list_key = symbol_short!("ADM_LIST");
    env.storage()
        .persistent()
        .get(&list_key)
        .unwrap_or(Vec::new(env))
}

/// Adds an address to the admin tracking list (avoids duplicates).
pub fn track_admin(env: &Env, admin: &Address) {
    let list_key = symbol_short!("ADM_LIST");
    let mut admins: Vec<Address> = env
        .storage()
        .persistent()
        .get(&list_key)
        .unwrap_or(Vec::new(env));

    let mut found = false;
    for i in 0..admins.len() {
        if admins.get(i) == Some(admin.clone()) {
            found = true;
            break;
        }
    }
    if !found {
        admins.push_back(admin.clone());
        env.storage().persistent().set(&list_key, &admins);
    }
}

/// Removes an address from the admin tracking list.
pub fn untrack_admin(env: &Env, admin: &Address) {
    let list_key = symbol_short!("ADM_LIST");
    let admins: Vec<Address> = env
        .storage()
        .persistent()
        .get(&list_key)
        .unwrap_or(Vec::new(env));

    let mut new_admins = Vec::new(env);
    for i in 0..admins.len() {
        if let Some(a) = admins.get(i) {
            if a != *admin {
                new_admins.push_back(a);
            }
        }
    }
    env.storage().persistent().set(&list_key, &new_admins);
}
