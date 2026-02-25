#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]
#![cfg(test)]

//! Benchmark suite for ZK proof verification.
//!
//! Measures CPU instructions and memory usage via `env.cost_estimate().budget()`
//! across varying proof configurations and public-input sizes.  Results are
//! printed to stdout so CI can capture them in build logs.

use soroban_sdk::{testutils::Address as _, Address, Env};
use zk_verifier::{AccessRequest, ZkAccessHelper, ZkVerifierContract, ZkVerifierContractClient};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a full `AccessRequest` with the given number of public inputs.
fn make_request(env: &Env, user: &Address, num_inputs: u32) -> AccessRequest {
    let mut proof_a = [0u8; 64];
    proof_a[0] = 1;
    let mut proof_b = [0u8; 128];
    proof_b[0] = 1;
    let mut proof_c = [0u8; 64];
    proof_c[0] = 1;

    let pis: std::vec::Vec<[u8; 32]> = (0..num_inputs)
        .map(|i| {
            let mut buf = [0u8; 32];
            if i == 0 {
                buf[0] = 1;
            } else {
                buf[0] = (i % 255 + 1) as u8;
            }
            buf
        })
        .collect();

    let pi_refs: std::vec::Vec<&[u8; 32]> = pis.iter().collect();

    ZkAccessHelper::create_request(
        env,
        user.clone(),
        [5u8; 32],
        proof_a,
        proof_b,
        proof_c,
        &pi_refs,
    )
}

/// Register the contract, initialize with an admin, and return the client.
fn setup_client(env: &Env) -> (ZkVerifierContractClient<'_>, Address) {
    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    client.initialize(&admin);

    (client, admin)
}

// ---------------------------------------------------------------------------
// Budget-based benchmarks
// ---------------------------------------------------------------------------

/// Run `verify_access` and capture the CPU-instruction and memory budget.
fn bench_verify_access(
    label: &str,
    env: &Env,
    client: &ZkVerifierContractClient,
    request: &AccessRequest,
) {
    env.cost_estimate().budget().reset_default();
    let _result = client.try_verify_access(request);

    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let mem = env.cost_estimate().budget().memory_bytes_cost();

    // Print results so CI can capture them via `--nocapture`.
    println!("[BENCH] {label}: cpu_instructions={cpu}, memory_bytes={mem}");
}

#[test]
fn bench_verify_single_input() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_client(&env);

    let user = Address::generate(&env);
    let request = make_request(&env, &user, 1);

    bench_verify_access("verify_single_input", &env, &client, &request);
}

#[test]
fn bench_verify_four_inputs() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_client(&env);

    let user = Address::generate(&env);
    let request = make_request(&env, &user, 4);

    bench_verify_access("verify_4_inputs", &env, &client, &request);
}

#[test]
fn bench_verify_eight_inputs() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_client(&env);

    let user = Address::generate(&env);
    let request = make_request(&env, &user, 8);

    bench_verify_access("verify_8_inputs", &env, &client, &request);
}

#[test]
fn bench_verify_max_inputs() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_client(&env);

    let user = Address::generate(&env);
    // MAX_PUBLIC_INPUTS = 16
    let request = make_request(&env, &user, 16);

    bench_verify_access("verify_max_16_inputs", &env, &client, &request);
}

#[test]
fn bench_verify_with_rate_limit_configured() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup_client(&env);

    // Enable rate-limiting to measure its overhead.
    client.set_rate_limit_config(&admin, &100, &3600);

    let user = Address::generate(&env);
    let request = make_request(&env, &user, 4);

    bench_verify_access("verify_with_rate_limit", &env, &client, &request);
}

#[test]
fn bench_verify_with_whitelist_enabled() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup_client(&env);

    let user = Address::generate(&env);

    client.set_whitelist_enabled(&admin, &true);
    client.add_to_whitelist(&admin, &user);

    let request = make_request(&env, &user, 4);

    bench_verify_access("verify_with_whitelist", &env, &client, &request);
}

// ---------------------------------------------------------------------------
// Regression guards — ensure budget stays within expected bounds
// ---------------------------------------------------------------------------

/// Budget regression baseline for a single-input verification.
/// If the CPU cost exceeds this threshold a performance regression was
/// introduced.  Adjust the threshold after intentional changes.
const CPU_REGRESSION_THRESHOLD_SINGLE: u64 = 50_000_000;

/// Budget regression baseline for max-input (16) verification.
const CPU_REGRESSION_THRESHOLD_MAX: u64 = 100_000_000;

#[test]
fn regression_single_input_cpu_budget() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_client(&env);

    let user = Address::generate(&env);
    let request = make_request(&env, &user, 1);

    env.cost_estimate().budget().reset_default();
    let _result = client.try_verify_access(&request);

    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    println!(
        "[REGRESSION] single_input: cpu_instructions={cpu} (threshold={CPU_REGRESSION_THRESHOLD_SINGLE})"
    );
    assert!(
        cpu <= CPU_REGRESSION_THRESHOLD_SINGLE,
        "CPU budget regression detected for single-input verify: \
         measured {cpu} > threshold {CPU_REGRESSION_THRESHOLD_SINGLE}"
    );
}

#[test]
fn regression_max_inputs_cpu_budget() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_client(&env);

    let user = Address::generate(&env);
    let request = make_request(&env, &user, 16);

    env.cost_estimate().budget().reset_default();
    let _result = client.try_verify_access(&request);

    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    println!(
        "[REGRESSION] max_inputs: cpu_instructions={cpu} (threshold={CPU_REGRESSION_THRESHOLD_MAX})"
    );
    assert!(
        cpu <= CPU_REGRESSION_THRESHOLD_MAX,
        "CPU budget regression detected for max-input verify: \
         measured {cpu} > threshold {CPU_REGRESSION_THRESHOLD_MAX}"
    );
}

/// Memory regression guard for max-input verification.
const MEM_REGRESSION_THRESHOLD_MAX: u64 = 50_000_000;

#[test]
fn regression_max_inputs_memory_budget() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_client(&env);

    let user = Address::generate(&env);
    let request = make_request(&env, &user, 16);

    env.cost_estimate().budget().reset_default();
    let _result = client.try_verify_access(&request);

    let mem = env.cost_estimate().budget().memory_bytes_cost();
    println!(
        "[REGRESSION] max_inputs_memory: memory_bytes={mem} (threshold={MEM_REGRESSION_THRESHOLD_MAX})"
    );
    assert!(
        mem <= MEM_REGRESSION_THRESHOLD_MAX,
        "Memory budget regression detected for max-input verify: \
         measured {mem} > threshold {MEM_REGRESSION_THRESHOLD_MAX}"
    );
}

/// Ensure that scaling from 1 → 16 inputs does not cause a super-linear
/// blow-up in CPU cost.
#[test]
fn regression_scaling_factor() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_client(&env);
    let user = Address::generate(&env);

    // Measure single-input cost.
    let req1 = make_request(&env, &user, 1);
    env.cost_estimate().budget().reset_default();
    let _r1 = client.try_verify_access(&req1);
    let cpu1 = env.cost_estimate().budget().cpu_instruction_cost();

    // Measure max-input cost (16).
    let req16 = make_request(&env, &user, 16);
    env.cost_estimate().budget().reset_default();
    let _r16 = client.try_verify_access(&req16);
    let cpu16 = env.cost_estimate().budget().cpu_instruction_cost();

    // Allow up to 4× linear scaling (generous margin).
    let max_allowed = cpu1.saturating_mul(4 * 16);
    println!("[REGRESSION] scaling: cpu_1={cpu1}, cpu_16={cpu16}, max_allowed={max_allowed}");
    assert!(
        cpu16 <= max_allowed,
        "Super-linear scaling detected: cpu_1={cpu1}, cpu_16={cpu16}, \
         ratio={:.2} (expected ≤ 64×)",
        cpu16 as f64 / cpu1 as f64
    );
}
