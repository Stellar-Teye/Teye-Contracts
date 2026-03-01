//! Test to verify all required exports are accessible
//! This test ensures the public API is complete for external usage

#![cfg(test)]

// Test that all types can be imported from the root
use zk_verifier::{ContractError, ZkVerifierContract, ZkVerifierContractClient};

// Test that helper types are accessible
use zk_verifier::{MerkleVerifier, ZkAccessHelper};

// Test that verifier types are accessible
use zk_verifier::{PoseidonHasher, ProofValidationError};

// Test that vk types are accessible
use zk_verifier::vk::{G1Point, G2Point};

// Test that types can be imported from submodules
use zk_verifier::verifier::G1Point as VerifierG1Point;
use zk_verifier::vk::G1Point as VkG1Point;

use soroban_sdk::{testutils::Address as _, BytesN, Env};

#[test]
fn test_all_exports_accessible() {
    // This test just needs to compile to verify exports work
    let env = Env::default();

    // Verify we can create types
    let _g1 = G1Point {
        x: BytesN::from_array(&env, &[1u8; 32]),
        y: BytesN::from_array(&env, &[1u8; 32]),
    };

    let _g2 = G2Point {
        x: (
            BytesN::from_array(&env, &[1u8; 32]),
            BytesN::from_array(&env, &[1u8; 32]),
        ),
        y: (
            BytesN::from_array(&env, &[1u8; 32]),
            BytesN::from_array(&env, &[1u8; 32]),
        ),
    };

    // Verify error enum is accessible
    let _err = ContractError::Unauthorized;

    // If this compiles, all exports are working correctly

}

#[test]
fn test_type_equivalence() {
    // Verify that G1Point from different import paths are the same type
    let env = Env::default();

    let g1_root = G1Point {
        x: BytesN::from_array(&env, &[1u8; 32]),
        y: BytesN::from_array(&env, &[1u8; 32]),
    };

    let g1_vk = VkG1Point {
        x: BytesN::from_array(&env, &[1u8; 32]),
        y: BytesN::from_array(&env, &[1u8; 32]),
    };

    let g1_verifier = VerifierG1Point {
        x: BytesN::from_array(&env, &[1u8; 32]),
        y: BytesN::from_array(&env, &[1u8; 32]),
    };

    // All three should be the same type and comparable
    assert_eq!(g1_root, g1_vk);
    assert_eq!(g1_vk, g1_verifier);
    assert_eq!(g1_root, g1_verifier);
}

#[test]
fn test_contract_client_accessible() {
    let env = Env::default();
    env.mock_all_auths();

    // Verify we can register and create a client
    let contract_id = env.register(ZkVerifierContract, ());
    let _client = ZkVerifierContractClient::new(&env, &contract_id);

    // If this compiles and runs, the contract client export works

}

#[test]
fn test_helper_functions_accessible() {
    let env = Env::default();

    // Test ZkAccessHelper is accessible
    let user = soroban_sdk::Address::generate(&env);
    let _request = ZkAccessHelper::create_request(
        &env,
        user,
        [1u8; 32],
        [1u8; 64],
        [1u8; 128],
        [1u8; 64],
        &[&[1u8; 32]],
        1000,
    );

    // Test MerkleVerifier is accessible
    let leaf = BytesN::from_array(&env, &[1u8; 32]);
    let mut leaves = soroban_sdk::Vec::new(&env);
    leaves.push_back(leaf);
    let _root = MerkleVerifier::compute_merkle_root(&env, &leaves);


}

#[test]
fn test_poseidon_hasher_accessible() {
    let env = Env::default();

    // Test PoseidonHasher is accessible
    let mut inputs = soroban_sdk::Vec::new(&env);
    inputs.push_back(BytesN::from_array(&env, &[1u8; 32]));

    let _hash = PoseidonHasher::hash(&env, &inputs);


}

#[test]
fn test_validation_error_accessible() {
    // Test that ProofValidationError enum is accessible
    let _err1 = ProofValidationError::ZeroedComponent;
    let _err2 = ProofValidationError::EmptyPublicInputs;
    let _err3 = ProofValidationError::MalformedG1PointA;


}

#[test]
fn test_contract_error_accessible() {
    // Test that ContractError enum is accessible and has all variants
    let _err1 = ContractError::Unauthorized;
    let _err2 = ContractError::RateLimited;
    let _err3 = ContractError::InvalidConfig;
    let _err4 = ContractError::EmptyPublicInputs;
    let _err5 = ContractError::TooManyPublicInputs;
    let _err6 = ContractError::DegenerateProof;
    let _err7 = ContractError::OversizedProofComponent;
    let _err8 = ContractError::MalformedG1Point;
    let _err9 = ContractError::MalformedG2Point;
    let _err10 = ContractError::ZeroedPublicInput;
    let _err11 = ContractError::MalformedProofData;
    let _err12 = ContractError::Paused;
    let _err13 = ContractError::InvalidAuthLevel;
    let _err14 = ContractError::ProofRequiredForAuthLevel;


}
