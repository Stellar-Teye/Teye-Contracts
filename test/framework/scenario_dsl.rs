//! # Scenario DSL
//!
//! A declarative, builder-pattern API for defining readable test scenarios
//! in a Given-When-Then style.
//!
//! ## Example
//!
//! ```rust,ignore
//! Scenario::new("Proportional reward distribution")
//!     .given(|ctx| {
//!         let alice = ctx.harness.create_staker(10_000);
//!         ctx.stakers.push(alice);
//!     })
//!     .when("both users stake equal amounts", |ctx| {
//!         ctx.harness.stake(&ctx.stakers[0], 5_000);
//!         ctx.harness.env.advance_time(100);
//!     })
//!     .then("rewards are split equally", |ctx| {
//!         let r0 = ctx.harness.pending_rewards(&ctx.stakers[0]);
//!         assert!(r0 > 0);
//!     })
//!     .run();
//! ```

extern crate std;

use soroban_sdk::Address;
use std::string::String;
use std::vec::Vec;

use super::{StakingTestHarness, TestEnv};
use super::invariants::InvariantSet;

// ── Scenario Context ─────────────────────────────────────────────────────────

/// Mutable context passed to scenario steps.
///
/// Holds a reference to the harness (which itself owns `&mut TestEnv`),
/// plus a user-managed list of staker addresses.
pub struct ScenarioContext<'a, 'b> {
    pub harness: &'a StakingTestHarness<'b>,
    pub stakers: Vec<Address>,
    /// Storage for arbitrary test data between steps.
    pub data: std::collections::HashMap<String, i128>,
}

impl<'a, 'b> ScenarioContext<'a, 'b> {
    fn new(harness: &'a StakingTestHarness<'b>) -> Self {
        Self {
            harness,
            stakers: Vec::new(),
            data: std::collections::HashMap::new(),
        }
    }

    /// Store a named value for use in later steps.
    pub fn store(&mut self, key: &str, value: i128) {
        self.data.insert(key.into(), value);
    }

    /// Retrieve a named value stored by a previous step.
    pub fn load(&self, key: &str) -> i128 {
        *self
            .data
            .get(key)
            .unwrap_or_else(|| panic!("Scenario variable '{}' not found", key))
    }
}

// ── Step Types ───────────────────────────────────────────────────────────────

type StepFn = Box<dyn FnOnce(&mut ScenarioContext<'_, '_>)>;

struct GivenStep {
    action: StepFn,
}

struct WhenStep {
    #[allow(dead_code)]
    description: String,
    action: StepFn,
}

struct ThenStep {
    #[allow(dead_code)]
    description: String,
    assertion: StepFn,
}

// ── Scenario Builder ─────────────────────────────────────────────────────────

/// A declarative test scenario with Given-When-Then structure.
///
/// Steps are collected via the builder and executed in order during `run()`.
/// Invariants can optionally be checked between When and Then phases.
pub struct Scenario {
    name: String,
    reward_rate: i128,
    lock_period: u64,
    given_steps: Vec<GivenStep>,
    when_steps: Vec<WhenStep>,
    then_steps: Vec<ThenStep>,
    invariants: Option<InvariantSet>,
}

impl Scenario {
    /// Create a new scenario with a descriptive name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            reward_rate: 10,
            lock_period: 86_400,
            given_steps: Vec::new(),
            when_steps: Vec::new(),
            then_steps: Vec::new(),
            invariants: None,
        }
    }

    /// Set the reward rate for the staking contract.
    pub fn with_reward_rate(mut self, rate: i128) -> Self {
        self.reward_rate = rate;
        self
    }

    /// Set the lock period for the staking contract.
    pub fn with_lock_period(mut self, period: u64) -> Self {
        self.lock_period = period;
        self
    }

    /// Add a precondition/setup step.
    pub fn given<F>(mut self, action: F) -> Self
    where
        F: FnOnce(&mut ScenarioContext<'_, '_>) + 'static,
    {
        self.given_steps.push(GivenStep {
            action: Box::new(action),
        });
        self
    }

    /// Add an action step with a description.
    pub fn when<F>(mut self, description: &str, action: F) -> Self
    where
        F: FnOnce(&mut ScenarioContext<'_, '_>) + 'static,
    {
        self.when_steps.push(WhenStep {
            description: description.into(),
            action: Box::new(action),
        });
        self
    }

    /// Add an assertion step with a description.
    pub fn then<F>(mut self, description: &str, assertion: F) -> Self
    where
        F: FnOnce(&mut ScenarioContext<'_, '_>) + 'static,
    {
        self.then_steps.push(ThenStep {
            description: description.into(),
            assertion: Box::new(assertion),
        });
        self
    }

    /// Attach invariants to check between when and then phases.
    pub fn with_invariants(mut self, invariants: InvariantSet) -> Self {
        self.invariants = Some(invariants);
        self
    }

    /// Execute the scenario.
    ///
    /// Initializes the test environment, runs all steps in order, and reports
    /// results. Panics on assertion failure with a descriptive message.
    pub fn run(self) {
        let mut env = TestEnv::new();
        let harness = StakingTestHarness::new(&mut env, self.reward_rate, self.lock_period);
        let mut ctx = ScenarioContext::new(&harness);

        // Execute Given steps.
        for step in self.given_steps {
            (step.action)(&mut ctx);
        }

        // Execute When steps.
        for step in self.when_steps {
            (step.action)(&mut ctx);
        }

        // Check invariants between When and Then if configured.
        if let Some(ref invariants) = self.invariants {
            let snapshot = harness.snapshot(&ctx.stakers);
            let violations = invariants.check_all(&snapshot);
            if !violations.is_empty() {
                let mut report = std::format!(
                    "Scenario '{}' — invariant violations after actions:\n",
                    self.name
                );
                for (name, msg) in &violations {
                    report.push_str(&std::format!("  ✗ [{}]: {}\n", name, msg));
                }
                panic!("{}", report);
            }
        }

        // Execute Then steps.
        for step in self.then_steps {
            (step.assertion)(&mut ctx);
        }
    }
}

// ── Assertion Helpers ────────────────────────────────────────────────────────

/// Assert that an action fails with the expected contract error code.
///
/// Uses `try_*` client methods that return `Result`.
#[macro_export]
macro_rules! assert_contract_error {
    ($result:expr, $expected:expr) => {
        match $result {
            Err(Ok(e)) => assert_eq!(
                e, $expected,
                "Expected error {:?}, got {:?}",
                $expected, e
            ),
            Err(Err(e)) => panic!("Unexpected SDK error: {:?}", e),
            Ok(_) => panic!("Expected error {:?}, but operation succeeded", $expected),
        }
    };
}

/// Assert that a value is within a percentage tolerance of the expected value.
///
/// Useful for reward calculations where fixed-point truncation causes rounding.
#[macro_export]
macro_rules! assert_within_tolerance {
    ($actual:expr, $expected:expr, $tolerance_pct:expr) => {{
        let actual = $actual as f64;
        let expected = $expected as f64;
        let tolerance = expected.abs() * ($tolerance_pct as f64 / 100.0);
        let diff = (actual - expected).abs();
        assert!(
            diff <= tolerance,
            "Value {} is not within {}% of expected {}: diff = {}",
            actual,
            $tolerance_pct,
            expected,
            diff,
        );
    }};
}

// ── Batch Scenario Runner ────────────────────────────────────────────────────

/// Run multiple scenarios and collect results.
///
/// Returns the number of passed and failed scenarios.
pub fn run_scenarios(scenarios: Vec<Scenario>) -> (usize, usize) {
    let total = scenarios.len();
    let mut failures = 0;

    for scenario in scenarios {
        let name = scenario.name.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scenario.run();
        }));

        match result {
            Ok(()) => {
                std::eprintln!("  ✓ {}", name);
            }
            Err(_) => {
                std::eprintln!("  ✗ {}", name);
                failures += 1;
            }
        }
    }

    (total - failures, failures)
}
