//! Progressive authorization policy and enforcement.

use crate::multisig;
use soroban_sdk::{contracttype, Address, Env};

/// Progressive auth level.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AuthLevel {
    Level1 = 1,
    Level2 = 2,
    Level3 = 3,
    Level4 = 4,
}

/// Level requirements resolved from policy.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthRequirements {
    pub level: AuthLevel,
    pub min_delay_seconds: u64,
    pub requires_multisig: bool,
    pub requires_zk_proof: bool,
}

/// Risk threshold policy mapping score -> level.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressiveAuthPolicy {
    pub level2_min_score: u32,
    pub level3_min_score: u32,
    pub level4_min_score: u32,
    pub level2_delay_seconds: u64,
    pub level3_delay_seconds: u64,
    pub level4_delay_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgressiveAuthError {
    DelayNotSatisfied,
    MissingMultisigApproval,
    MissingZkProof,
}

impl AuthLevel {
    pub fn rank(&self) -> u32 {
        self.clone() as u32
    }
}

pub fn default_policy() -> ProgressiveAuthPolicy {
    ProgressiveAuthPolicy {
        level2_min_score: 25,
        level3_min_score: 50,
        level4_min_score: 75,
        // Set zero default delay for backward compatibility; contracts can
        // configure non-zero delays in their own policy layers.
        level2_delay_seconds: 0,
        level3_delay_seconds: 0,
        level4_delay_seconds: 0,
    }
}

pub fn level_for_score(score: u32, policy: &ProgressiveAuthPolicy) -> AuthLevel {
    if score >= policy.level4_min_score {
        AuthLevel::Level4
    } else if score >= policy.level3_min_score {
        AuthLevel::Level3
    } else if score >= policy.level2_min_score {
        AuthLevel::Level2
    } else {
        AuthLevel::Level1
    }
}

pub fn requirements_for_level(
    level: AuthLevel,
    policy: &ProgressiveAuthPolicy,
) -> AuthRequirements {
    match level {
        AuthLevel::Level1 => AuthRequirements {
            level,
            min_delay_seconds: 0,
            requires_multisig: false,
            requires_zk_proof: false,
        },
        AuthLevel::Level2 => AuthRequirements {
            level,
            min_delay_seconds: policy.level2_delay_seconds,
            requires_multisig: false,
            requires_zk_proof: false,
        },
        AuthLevel::Level3 => AuthRequirements {
            level,
            min_delay_seconds: policy.level3_delay_seconds,
            requires_multisig: true,
            requires_zk_proof: false,
        },
        AuthLevel::Level4 => AuthRequirements {
            level,
            min_delay_seconds: policy.level4_delay_seconds,
            requires_multisig: true,
            requires_zk_proof: true,
        },
    }
}

/// Evaluate if a risk jump requires step-up auth.
pub fn needs_step_up(previous: AuthLevel, current: AuthLevel) -> bool {
    current.rank() > previous.rank()
}

/// Enforce the resolved level with progressive checks.
pub fn enforce_level(
    env: &Env,
    _caller: &Address,
    level: AuthLevel,
    auth_started_at: u64,
    proposal_id: Option<u64>,
    zk_verified: bool,
    policy: &ProgressiveAuthPolicy,
) -> Result<(), ProgressiveAuthError> {
    let reqs = requirements_for_level(level, policy);
    let now = env.ledger().timestamp();

    if now < auth_started_at.saturating_add(reqs.min_delay_seconds) {
        return Err(ProgressiveAuthError::DelayNotSatisfied);
    }

    if reqs.requires_multisig && !multisig_satisfied(env, proposal_id) {
        return Err(ProgressiveAuthError::MissingMultisigApproval);
    }

    if reqs.requires_zk_proof && !zk_verified {
        return Err(ProgressiveAuthError::MissingZkProof);
    }

    Ok(())
}

/// Enforce auth directly from risk score.
pub fn enforce_for_risk(
    env: &Env,
    caller: &Address,
    score: u32,
    auth_started_at: u64,
    proposal_id: Option<u64>,
    zk_verified: bool,
    policy: &ProgressiveAuthPolicy,
) -> Result<AuthLevel, ProgressiveAuthError> {
    let level = level_for_score(score, policy);
    enforce_level(
        env,
        caller,
        level.clone(),
        auth_started_at,
        proposal_id,
        zk_verified,
        policy,
    )?;
    Ok(level)
}

fn multisig_satisfied(env: &Env, proposal_id: Option<u64>) -> bool {
    if multisig::is_legacy_admin_allowed(env) {
        return true;
    }

    match proposal_id {
        Some(id) => multisig::is_executable(env, id),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multisig;
    use soroban_sdk::{
        contract, contractimpl, symbol_short, testutils::Address as _, BytesN, Env, Vec,
    };

    #[contract]
    struct TestContract;

    #[contractimpl]
    impl TestContract {
        pub fn noop(_env: Env) {}
    }

    #[test]
    fn levels_map_from_risk_score() {
        let policy = default_policy();
        assert_eq!(level_for_score(0, &policy), AuthLevel::Level1);
        assert_eq!(level_for_score(30, &policy), AuthLevel::Level2);
        assert_eq!(level_for_score(60, &policy), AuthLevel::Level3);
        assert_eq!(level_for_score(90, &policy), AuthLevel::Level4);
    }

    #[test]
    fn step_up_detection_works() {
        assert!(needs_step_up(AuthLevel::Level1, AuthLevel::Level2));
        assert!(needs_step_up(AuthLevel::Level2, AuthLevel::Level4));
        assert!(!needs_step_up(AuthLevel::Level3, AuthLevel::Level2));
    }

    #[test]
    fn level3_requires_multisig_when_configured() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(TestContract, ());

        let a1 = Address::generate(&env);
        let a2 = Address::generate(&env);
        let caller = a1.clone();

        let mut signers = Vec::new(&env);
        signers.push_back(a1.clone());
        signers.push_back(a2.clone());
        env.as_contract(&contract_id, || {
            multisig::configure(&env, signers, 2).expect("valid multisig config");
        });

        let policy = default_policy();
        let denied = env.as_contract(&contract_id, || {
            enforce_level(
                &env,
                &caller,
                AuthLevel::Level3,
                env.ledger().timestamp(),
                None,
                false,
                &policy,
            )
        });
        assert_eq!(denied, Err(ProgressiveAuthError::MissingMultisigApproval));

        let proposal = env.as_contract(&contract_id, || {
            multisig::propose(
                &env,
                &a1,
                symbol_short!("SET_RATE"),
                BytesN::from_array(&env, &[1; 32]),
            )
            .expect("proposal should be created")
        });
        env.as_contract(&contract_id, || {
            multisig::approve(&env, &a2, proposal).expect("second signer approval");
        });

        let allowed = env.as_contract(&contract_id, || {
            enforce_level(
                &env,
                &caller,
                AuthLevel::Level3,
                env.ledger().timestamp(),
                Some(proposal),
                false,
                &policy,
            )
        });
        assert!(allowed.is_ok());
    }

    #[test]
    fn level4_requires_zk_proof() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(TestContract, ());

        let caller = Address::generate(&env);
        let policy = default_policy();

        let denied = env.as_contract(&contract_id, || {
            enforce_level(
                &env,
                &caller,
                AuthLevel::Level4,
                env.ledger().timestamp(),
                Some(1),
                false,
                &policy,
            )
        });

        assert_eq!(denied, Err(ProgressiveAuthError::MissingZkProof));
    }
}
