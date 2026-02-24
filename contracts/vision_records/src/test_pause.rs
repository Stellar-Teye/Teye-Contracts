#![cfg(test)]

use crate::{
    circuit_breaker::PauseScope, rbac::Role, ContractError, RecordType, VisionRecordsContract,
    VisionRecordsContractClient,
};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, String};

fn setup_test() -> (Env, VisionRecordsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

#[test]
fn test_global_pause() {
    let (env, client, admin) = setup_test();

    let user = Address::generate(&env);

    // Admin pauses globally
    client.pause_contract(&admin, &PauseScope::Global);

    // Attempt to register a user - should fail
    let res = client.try_register_user(
        &admin,
        &user,
        &Role::Patient,
        &String::from_str(&env, "Test"),
    );
    assert_eq!(res.unwrap_err().unwrap(), ContractError::Paused);

    // Admin resumes globally
    client.resume_contract(&admin, &PauseScope::Global);

    // Should now succeed
    client.register_user(
        &admin,
        &user,
        &Role::Patient,
        &String::from_str(&env, "Test"),
    );
}

#[test]
fn test_granular_pause() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.register_user(
        &admin,
        &patient,
        &Role::Patient,
        &String::from_str(&env, "Pat"),
    );
    client.register_user(
        &admin,
        &doctor,
        &Role::Optometrist,
        &String::from_str(&env, "Doc"),
    );

    // Admin pauses ONLY `ADD_REC` function
    let add_rec_scope = PauseScope::Function(symbol_short!("ADD_REC"));
    client.pause_contract(&admin, &add_rec_scope);

    // Adding a record should fail
    let hash = String::from_str(&env, "hash");
    let res = client.try_add_record(&doctor, &patient, &doctor, &RecordType::Examination, &hash);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::Paused);

    // But granting access should still work (it uses `GRT_ACC`)
    client.grant_access(
        &patient,
        &patient,
        &doctor,
        &crate::AccessLevel::Read,
        &3600,
    );

    // Resume `ADD_REC`
    client.resume_contract(&admin, &add_rec_scope);

    // Should now succeed
    client.add_record(&doctor, &patient, &doctor, &RecordType::Examination, &hash);
}

#[test]
fn test_unauthorized_pause() {
    let (env, client, admin) = setup_test();

    let staff = Address::generate(&env);
    client.register_user(
        &admin,
        &staff,
        &Role::Staff,
        &String::from_str(&env, "Staff"),
    );

    // Staff tries to pause - fails
    let res = client.try_pause_contract(&staff, &PauseScope::Global);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::Unauthorized);
}
