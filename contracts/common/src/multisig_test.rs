#![cfg(test)]

use super::{
    approve, configure, get_config, is_executable, mark_executed, propose, MultisigError, Proposal,
};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, BytesN, Env, Symbol, Vec};

#[test]
fn test_configure_valid() {
    let env = Env::default();
    let signers = Vec::from_array(&env, [Address::generate(&env), Address::generate(&env)]);
    let result = configure(&env, signers.clone(), 2);
    assert!(result.is_ok());

    let cfg = get_config(&env).unwrap();
    assert_eq!(cfg.threshold, 2);
    assert_eq!(cfg.signers.len(), 2);
}

#[test]
fn test_configure_invalid() {
    let env = Env::default();
    let signers = Vec::from_array(&env, [Address::generate(&env)]);

    // Threshold 0
    assert_eq!(
        configure(&env, signers.clone(), 0),
        Err(MultisigError::InvalidConfig)
    );

    // Threshold > Signers
    assert_eq!(
        configure(&env, signers.clone(), 2),
        Err(MultisigError::InvalidConfig)
    );
}

#[test]
fn test_propose_and_approve_quorum() {
    let env = Env::default();
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let s3 = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone(), s2.clone(), s3.clone()]);

    configure(&env, signers, 2).unwrap();

    let action = symbol_short!("ACTION");
    let data_hash = BytesN::from_array(&env, &[0u8; 32]);

    // Proposer is s1
    let id = propose(&env, &s1, action.clone(), data_hash.clone()).unwrap();

    // approvals.len() is 1 (s1)
    assert!(!is_executable(&env, id));

    // s2 approves
    approve(&env, &s2, id).unwrap();

    // approvals.len() is 2 (s1, s2). Threshold is 2.
    assert!(is_executable(&env, id));

    // Mark as executed
    mark_executed(&env, id).unwrap();

    // Already executed
    assert_eq!(mark_executed(&env, id), Err(MultisigError::AlreadyExecuted));
}

#[test]
fn test_duplicate_approval_fails() {
    let env = Env::default();
    let s1 = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone(), Address::generate(&env)]);
    configure(&env, signers, 2).unwrap();

    let id = propose(
        &env,
        &s1,
        symbol_short!("ACT"),
        BytesN::from_array(&env, &[0u8; 32]),
    )
    .unwrap();

    // Proposer already approved
    assert_eq!(approve(&env, &s1, id), Err(MultisigError::AlreadyApproved));
}

#[test]
fn test_non_signer_cannot_approve() {
    let env = Env::default();
    let s1 = Address::generate(&env);
    let stranger = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone()]);
    configure(&env, signers, 1).unwrap();

    let id = propose(
        &env,
        &s1,
        symbol_short!("ACT"),
        BytesN::from_array(&env, &[0u8; 32]),
    )
    .unwrap();

    assert_eq!(approve(&env, &stranger, id), Err(MultisigError::NotASigner));
}

#[test]
fn test_large_signer_set() {
    let env = Env::default();
    let mut signers = Vec::new(&env);
    for _ in 0..20 {
        signers.push_back(Address::generate(&env));
    }

    let threshold = 15;
    configure(&env, signers.clone(), threshold).unwrap();

    let proposer = signers.get(0).unwrap();
    let id = propose(
        &env,
        &proposer,
        symbol_short!("ACT"),
        BytesN::from_array(&env, &[0u8; 32]),
    )
    .unwrap();

    // Add 13 more approvals (total 14)
    for i in 1..14 {
        approve(&env, &signers.get(i).unwrap(), id).unwrap();
    }
    assert!(!is_executable(&env, id));

    // Add 15th approval
    approve(&env, &signers.get(14).unwrap(), id).unwrap();
    assert!(is_executable(&env, id));
}

#[test]
fn test_threshold_transition() {
    let env = Env::default();
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone(), s2.clone()]);

    // Initial threshold 2
    configure(&env, signers.clone(), 2).unwrap();

    let id = propose(
        &env,
        &s1,
        symbol_short!("ACT"),
        BytesN::from_array(&env, &[0u8; 32]),
    )
    .unwrap();
    assert!(!is_executable(&env, id));

    // Change threshold to 1
    configure(&env, signers.clone(), 1).unwrap();

    // Now it should be executable because it has 1 approval (s1)
    assert!(is_executable(&env, id));
}

#[test]
fn test_remove_signer_impact() {
    let env = Env::default();
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone(), s2.clone()]);

    configure(&env, signers.clone(), 2).unwrap();

    let id = propose(
        &env,
        &s1,
        symbol_short!("ACT"),
        BytesN::from_array(&env, &[0u8; 32]),
    )
    .unwrap();
    approve(&env, &s2, id).unwrap();
    assert!(is_executable(&env, id));

    // Remove s2 from signers. New config: threshold 1, signers [s1]
    let new_signers = Vec::from_array(&env, [s1.clone()]);
    configure(&env, new_signers, 1).unwrap();

    // In current implementation, it's STILL executable because it has 2 approvals in persistent storage,
    // and threshold is now 1.
    assert!(is_executable(&env, id));
}
