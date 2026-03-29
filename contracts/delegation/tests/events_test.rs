#![cfg(test)]

extern crate std;

use delegation::{DelegationContract, DelegationContractClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    Address, BytesN, Env, IntoVal, Val,
};

#[test]
fn test_event_emission() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DelegationContract, ());
    let client = DelegationContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    client.initialize(&admin);

    let events = env.events().all();
    assert_eq!(events.len(), 1);

    let init_event = events.get(0).unwrap();
    assert_eq!(init_event.1, (symbol_short!("INIT"),).into_val(&env));
    assert_eq!(init_event.2, admin.into_val(&env));

    let creator = Address::generate(&env);
    let input_data = BytesN::from_array(&env, &[1; 32]);
    let task_id = client.submit_task(&creator, &input_data, &1, &1000);

    let events = env.events().all();
    let sub_event = events.get(1).unwrap();
    assert_eq!(
        sub_event.1,
        (symbol_short!("TSK_SUB"), task_id).into_val(&env)
    );
    assert_eq!(sub_event.2, creator.into_val(&env));

    let executor = Address::generate(&env);
    client.register_executor(&executor);

    let events = env.events().all();
    let reg_event = events.get(2).unwrap();
    assert_eq!(reg_event.1, (symbol_short!("EX_REG"),).into_val(&env));
    assert_eq!(reg_event.2, executor.into_val(&env));

    client.assign_task(&executor, &task_id);

    let events = env.events().all();
    let assign_event = events.get(3).unwrap();
    assert_eq!(
        assign_event.1,
        (symbol_short!("TSK_ASS"), task_id).into_val(&env)
    );
    assert_eq!(assign_event.2, executor.into_val(&env));

    let result = BytesN::from_array(&env, &[2; 32]);
    let proof = BytesN::from_array(&env, &[3; 32]);
    client.submit_result(&executor, &task_id, &result, &proof);

    let events = env.events().all();
    let res_event = events.get(4).unwrap();
    let topics: soroban_sdk::Vec<Val> = res_event.1;
    assert_eq!(
        topics.get(0).unwrap(),
        symbol_short!("TSK_RES").into_val(&env)
    );
    assert_eq!(topics.get(1).unwrap(), task_id.into_val(&env));
    assert_eq!(res_event.2, executor.into_val(&env));
}
