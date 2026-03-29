#![cfg(test)]

extern crate std;

use identity::events::{
    GuardianChangedEvent, OwnerStatusChangedEvent, RecoveryCancelledEvent, RecoveryExecutedEvent,
    RecoveryInitiatedEvent, ZkCredentialVerifiedEvent,
};
use identity::{IdentityContract, IdentityContractClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    vec,
    xdr::{ContractEventBody, ScVal},
    Address, BytesN, Env, IntoVal, TryFromVal, Val, Vec,
};

fn setup() -> (Env, IdentityContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(IdentityContract, ());
    let client = IdentityContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);

    (env, client, owner)
}

fn assert_last_event<T>(env: &Env, expected_topics: &Vec<Val>, expected_data: &T)
where
    T: Clone + IntoVal<Env, Val>,
{
    let events = env.events().all();
    // In soroban-sdk 25, ContractEvents has an events() method that returns a Vec<soroban_sdk::xdr::ContractEvent>
    let event = events.events().last().expect("No events found");
    let ContractEventBody::V0(body) = &event.body;

    let mut expected_topics_scval = std::vec::Vec::new();
    for topic in expected_topics.iter() {
        expected_topics_scval.push(ScVal::try_from_val(env, &topic).unwrap());
    }
    assert_eq!(body.topics.as_slice(), expected_topics_scval.as_slice());

    let expected_val: Val = expected_data.clone().into_val(env);
    let expected_data_scval = ScVal::try_from_val(env, &expected_val).unwrap();
    assert_eq!(body.data, expected_data_scval);
}

#[test]
fn test_initialize_event() {
    let (env, client, owner) = setup();
    client.initialize(&owner);

    let expected_topics: Vec<Val> =
        (symbol_short!("STREAM"), symbol_short!("ID_STAT")).into_val(&env);
    let expected_data = OwnerStatusChangedEvent {
        owner: owner.clone(),
        active: true,
        timestamp: env.ledger().timestamp(),
    };

    assert_last_event(&env, &expected_topics, &expected_data);
}

#[test]
fn test_guardian_changed_event() {
    let (env, client, owner) = setup();
    client.initialize(&owner);

    let guardian = Address::generate(&env);

    // Add guardian
    client.add_guardian(&owner, &guardian);
    let expected_topics: Vec<Val> =
        (symbol_short!("STREAM"), symbol_short!("ID_GUARD")).into_val(&env);
    let expected_data = GuardianChangedEvent {
        owner: owner.clone(),
        guardian: guardian.clone(),
        added: true,
        timestamp: env.ledger().timestamp(),
    };
    assert_last_event(&env, &expected_topics, &expected_data);

    // Remove guardian
    client.remove_guardian(&owner, &guardian);
    let expected_data_rem = GuardianChangedEvent {
        owner: owner.clone(),
        guardian: guardian.clone(),
        added: false,
        timestamp: env.ledger().timestamp(),
    };
    assert_last_event(&env, &expected_topics, &expected_data_rem);
}

#[test]
fn test_recovery_events() {
    let (env, client, owner) = setup();
    client.initialize(&owner);

    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);
    client.add_guardian(&owner, &g1);
    client.add_guardian(&owner, &g2);
    client.add_guardian(&owner, &g3);
    client.set_recovery_threshold(&owner, &3);

    let new_owner = Address::generate(&env);

    // Initiate recovery
    client.initiate_recovery(&g1, &owner, &new_owner);
    let expected_topics_init: Vec<Val> =
        (symbol_short!("STREAM"), symbol_short!("ID_RINIT")).into_val(&env);
    let expected_data_init = RecoveryInitiatedEvent {
        owner: owner.clone(),
        new_address: new_owner.clone(),
        initiated_by: g1.clone(),
        timestamp: env.ledger().timestamp(),
    };
    assert_last_event(&env, &expected_topics_init, &expected_data_init);

    // Cancel recovery
    client.cancel_recovery(&owner);
    let expected_topics_cncl: Vec<Val> =
        (symbol_short!("STREAM"), symbol_short!("ID_RCNCL")).into_val(&env);
    let expected_data_cncl = RecoveryCancelledEvent {
        owner: owner.clone(),
        timestamp: env.ledger().timestamp(),
    };
    assert_last_event(&env, &expected_topics_cncl, &expected_data_cncl);

    // Re-initiate and execute
    client.initiate_recovery(&g1, &owner, &new_owner);
    client.approve_recovery(&g2, &owner);
    client.approve_recovery(&g3, &owner);

    let req = client.get_recovery_request(&owner).unwrap();
    // Advance time beyond cooldown
    env.ledger().set_timestamp(req.execute_after + 1);

    let caller = Address::generate(&env);
    client.execute_recovery(&caller, &owner);

    let events = env.events().all();
    let events_vec = events.events();
    let len = events_vec.len();

    // 1. RecoveryExecutedEvent (ID_REXEC)
    let exec_event = &events_vec[len - 3];
    let ContractEventBody::V0(exec_body) = &exec_event.body;
    let expected_exec_val: Val = RecoveryExecutedEvent {
        old_address: owner.clone(),
        new_address: new_owner.clone(),
        timestamp: env.ledger().timestamp(),
    }
    .into_val(&env);
    assert_eq!(
        exec_body.data,
        ScVal::try_from_val(&env, &expected_exec_val).unwrap()
    );

    // 2. OwnerStatusChangedEvent (ID_STAT, active=false) for old owner
    let deact_event = &events_vec[len - 2];
    let ContractEventBody::V0(deact_body) = &deact_event.body;
    let expected_deact_val: Val = OwnerStatusChangedEvent {
        owner: owner.clone(),
        active: false,
        timestamp: env.ledger().timestamp(),
    }
    .into_val(&env);
    assert_eq!(
        deact_body.data,
        ScVal::try_from_val(&env, &expected_deact_val).unwrap()
    );

    // 3. OwnerStatusChangedEvent (ID_STAT, active=true) for new owner
    let act_event = &events_vec[len - 1];
    let ContractEventBody::V0(act_body) = &act_event.body;
    let expected_act_val: Val = OwnerStatusChangedEvent {
        owner: new_owner.clone(),
        active: true,
        timestamp: env.ledger().timestamp(),
    }
    .into_val(&env);
    assert_eq!(
        act_body.data,
        ScVal::try_from_val(&env, &expected_act_val).unwrap()
    );
}

#[test]
fn test_credential_binding_events() {
    let (env, client, owner) = setup();
    client.initialize(&owner);

    let credential_id = BytesN::from_array(&env, &[1u8; 32]);

    // Bind
    client.bind_credential(&owner, &credential_id);
    let expected_topics: Vec<Val> = (symbol_short!("CRD_BIND"), owner.clone()).into_val(&env);
    assert_last_event(&env, &expected_topics, &credential_id);

    // Unbind
    client.unbind_credential(&owner, &credential_id);
    let expected_topics_ubnd: Vec<Val> = (symbol_short!("CRD_UBND"), owner.clone()).into_val(&env);
    assert_last_event(&env, &expected_topics_ubnd, &credential_id);
}

#[test]
fn test_zk_credential_event() {
    let (env, client, owner) = setup();
    client.initialize(&owner);

    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[5u8; 32]);
    let proof = soroban_sdk::Bytes::new(&env);
    let pi = vec![&env, BytesN::from_array(&env, &[0u8; 32])];

    client.verify_zk_credential(&user, &resource_id, &proof, &proof, &proof, &pi, &1000);

    let expected_topics: Vec<Val> =
        (symbol_short!("STREAM"), symbol_short!("ID_ZKCRD")).into_val(&env);
    let expected_data = ZkCredentialVerifiedEvent {
        user: user.clone(),
        verified: true,
        timestamp: env.ledger().timestamp(),
    };
    assert_last_event(&env, &expected_topics, &expected_data);
}
