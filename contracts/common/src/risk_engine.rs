//! Risk scoring primitives for progressive authorization.
//!
//! The engine combines operation intent, data sensitivity, runtime context,
//! and behavioral anomalies to produce a bounded risk score (0..100).

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

const RISK_BEHAVIOR: Symbol = symbol_short!("RSK_BEH");
const BEHAVIOR_WINDOW_SECONDS: u64 = 3_600;
const BEHAVIOR_ANOMALY_THRESHOLD: u32 = 8;

/// Operation type influences baseline risk.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ActionType {
    Read = 1,
    Write = 2,
    Share = 3,
    Delete = 4,
    AdminChange = 5,
    EmergencyOverride = 6,
}

/// Sensitivity classification for data touched by the action.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DataSensitivity {
    Public = 1,
    Internal = 2,
    Sensitive = 3,
    Restricted = 4,
}

/// Runtime context dimensions that can increase risk.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskContext {
    pub off_hours: bool,
    pub unusual_location: bool,
    pub unusual_frequency: bool,
    pub recent_auth_failures: u32,
    pub emergency_signal: bool,
}

/// Input envelope for risk scoring.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationRiskInput {
    pub actor: Address,
    pub operation: Symbol,
    pub action: ActionType,
    pub sensitivity: DataSensitivity,
    pub context: RiskContext,
}

/// Persisted behavior tracker per actor + operation window.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BehaviorState {
    pub window_start: u64,
    pub count: u32,
    pub anomaly_hits: u32,
}

/// Score decomposition for auditability.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskAssessment {
    pub base_score: u32,
    pub behavioral_adjustment: u32,
    pub final_score: u32,
}

pub type RiskScoringFn = fn(&OperationRiskInput) -> u32;

fn behavior_key(actor: &Address, operation: Symbol) -> (Symbol, Address, Symbol) {
    (RISK_BEHAVIOR, actor.clone(), operation)
}

fn base_weight_for_action(action: &ActionType) -> u32 {
    match action {
        ActionType::Read => 5,
        ActionType::Write => 20,
        ActionType::Share => 25,
        ActionType::Delete => 35,
        ActionType::AdminChange => 40,
        ActionType::EmergencyOverride => 45,
    }
}

fn base_weight_for_sensitivity(sensitivity: &DataSensitivity) -> u32 {
    match sensitivity {
        DataSensitivity::Public => 5,
        DataSensitivity::Internal => 15,
        DataSensitivity::Sensitive => 30,
        DataSensitivity::Restricted => 45,
    }
}

/// Default risk scoring model.
pub fn default_risk_score(input: &OperationRiskInput) -> u32 {
    let mut score =
        base_weight_for_action(&input.action) + base_weight_for_sensitivity(&input.sensitivity);
    if input.context.off_hours {
        score += 8;
    }
    if input.context.unusual_location {
        score += 10;
    }
    if input.context.unusual_frequency {
        score += 12;
    }
    score += input.context.recent_auth_failures.min(6) * 3;
    if input.context.emergency_signal {
        score += 12;
    }
    score.min(100)
}

/// Score risk and update behavioral state to dynamically adjust anomaly pressure.
pub fn evaluate_risk(
    env: &Env,
    input: &OperationRiskInput,
    scorer: Option<RiskScoringFn>,
) -> RiskAssessment {
    let score_fn = scorer.unwrap_or(default_risk_score);
    let base_score = score_fn(input).min(100);

    let now = env.ledger().timestamp();
    let key = behavior_key(&input.actor, input.operation.clone());
    let mut state: BehaviorState = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(BehaviorState {
            window_start: now,
            count: 0,
            anomaly_hits: 0,
        });

    if now.saturating_sub(state.window_start) > BEHAVIOR_WINDOW_SECONDS {
        state.window_start = now;
        state.count = 0;
        state.anomaly_hits = 0;
    }

    state.count = state.count.saturating_add(1);

    let mut behavioral_adjustment: u32 = 0;
    if state.count > BEHAVIOR_ANOMALY_THRESHOLD {
        state.anomaly_hits = state.anomaly_hits.saturating_add(1);
        behavioral_adjustment = behavioral_adjustment
            .saturating_add((state.count - BEHAVIOR_ANOMALY_THRESHOLD).saturating_mul(2));
    }

    if input.context.recent_auth_failures > 2 {
        state.anomaly_hits = state.anomaly_hits.saturating_add(1);
        behavioral_adjustment = behavioral_adjustment.saturating_add(6);
    }

    env.storage().persistent().set(&key, &state);

    RiskAssessment {
        base_score,
        behavioral_adjustment,
        final_score: base_score.saturating_add(behavioral_adjustment).min(100),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _};

    #[contract]
    struct TestContract;

    #[contractimpl]
    impl TestContract {
        pub fn noop(_env: Env) {}
    }

    #[test]
    fn default_model_adds_context_risk() {
        let env = Env::default();
        let actor = Address::generate(&env);
        let input = OperationRiskInput {
            actor,
            operation: symbol_short!("READ"),
            action: ActionType::Read,
            sensitivity: DataSensitivity::Public,
            context: RiskContext {
                off_hours: true,
                unusual_location: true,
                unusual_frequency: true,
                recent_auth_failures: 2,
                emergency_signal: false,
            },
        };

        let score = default_risk_score(&input);
        assert!(score >= 40);
    }

    #[test]
    fn behavioral_anomaly_increases_score() {
        let env = Env::default();
        let contract_id = env.register(TestContract, ());
        let actor = Address::generate(&env);

        let input = OperationRiskInput {
            actor,
            operation: symbol_short!("WRITE"),
            action: ActionType::Write,
            sensitivity: DataSensitivity::Sensitive,
            context: RiskContext {
                off_hours: false,
                unusual_location: false,
                unusual_frequency: false,
                recent_auth_failures: 0,
                emergency_signal: false,
            },
        };

        let mut assessment = env.as_contract(&contract_id, || evaluate_risk(&env, &input, None));
        env.as_contract(&contract_id, || {
            for _ in 0..10 {
                assessment = evaluate_risk(&env, &input, None);
            }
        });

        assert!(assessment.behavioral_adjustment > 0);
        assert!(assessment.final_score >= assessment.base_score);
    }
}
