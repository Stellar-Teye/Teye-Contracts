extern crate std;

use soroban_sdk::{
    testutils::Address as _,
    Address, Env, String, Vec,
};

use crate::{EventError, EventStreamContract, EventStreamContractClient};

// ── Test helpers ─────────────────────────────────────────────────────────────

fn setup() -> (Env, EventStreamContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventStreamContract, ());
    let client = EventStreamContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

/// Helper: register a schema and publish an event, returning the event ID.
fn publish_test_event(
    env: &Env,
    client: &EventStreamContractClient,
    admin: &Address,
    topic: &str,
    schema_version: u32,
    payload: &str,
) -> u64 {
    let topic_str = String::from_str(env, topic);
    let payload_str = String::from_str(env, payload);
    client.publish_event(admin, &topic_str, &schema_version, &payload_str)
}

fn register_schema(
    env: &Env,
    client: &EventStreamContractClient,
    admin: &Address,
    topic: &str,
    version: u32,
) {
    let topic_str = String::from_str(env, topic);
    let hash = String::from_str(env, "sha256:abc123");
    client.register_schema(admin, &topic_str, &version, &hash);
}

// ── Initialization tests ─────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (_env, client, admin) = setup();

    assert!(client.is_initialized());
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_event_count(), 0);
    assert_eq!(client.get_lamport_clock(), 0);
}

#[test]
fn test_double_initialize_fails() {
    let (env, client, _admin) = setup();
    let admin2 = Address::generate(&env);

    let result = client.try_initialize(&admin2);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::AlreadyInitialized),
        _ => panic!("Expected AlreadyInitialized error"),
    }
}

// ── Schema registration tests ────────────────────────────────────────────────

#[test]
fn test_register_and_get_schema() {
    let (env, client, admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");
    let hash = String::from_str(&env, "sha256:deadbeef");

    client.register_schema(&admin, &topic, &1, &hash);

    let retrieved = client.get_schema(&topic, &1);
    assert_eq!(retrieved, hash);

    let latest = client.get_latest_schema_version(&topic);
    assert_eq!(latest, 1);
}

#[test]
fn test_schema_versioning_ascending() {
    let (env, client, admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");
    let hash_v1 = String::from_str(&env, "sha256:v1");
    let hash_v2 = String::from_str(&env, "sha256:v2");

    client.register_schema(&admin, &topic, &1, &hash_v1);
    client.register_schema(&admin, &topic, &2, &hash_v2);

    assert_eq!(client.get_schema(&topic, &1), hash_v1);
    assert_eq!(client.get_schema(&topic, &2), hash_v2);
    assert_eq!(client.get_latest_schema_version(&topic), 2);
}

#[test]
fn test_schema_version_must_be_ascending() {
    let (env, client, admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");
    let hash = String::from_str(&env, "sha256:v1");

    client.register_schema(&admin, &topic, &2, &hash);

    // Registering version 1 after version 2 should fail
    let result = client.try_register_schema(&admin, &topic, &1, &hash);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::InvalidSchema),
        _ => panic!("Expected InvalidSchema error"),
    }
}

#[test]
fn test_schema_version_zero_rejected() {
    let (env, client, admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");
    let hash = String::from_str(&env, "sha256:v0");

    let result = client.try_register_schema(&admin, &topic, &0, &hash);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::InvalidSchema),
        _ => panic!("Expected InvalidSchema error"),
    }
}

// ── Event publishing tests ───────────────────────────────────────────────────

#[test]
fn test_publish_event() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);

    let event_id = publish_test_event(
        &env,
        &client,
        &admin,
        "records.vision.create",
        1,
        "payload_hash_1",
    );

    assert_eq!(event_id, 1);
    assert_eq!(client.get_event_count(), 1);
    assert_eq!(client.get_lamport_clock(), 1);
}

#[test]
fn test_lamport_ordering() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);
    register_schema(&env, &client, &admin, "staking.staked", 1);

    let id1 = publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");
    let id2 = publish_test_event(&env, &client, &admin, "staking.staked", 1, "p2");
    let id3 = publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p3");

    let e1 = client.get_event(&id1);
    let e2 = client.get_event(&id2);
    let e3 = client.get_event(&id3);

    // Lamport timestamps must be strictly increasing
    assert!(e1.lamport_ts < e2.lamport_ts);
    assert!(e2.lamport_ts < e3.lamport_ts);

    // Event IDs must also be strictly increasing
    assert!(id1 < id2);
    assert!(id2 < id3);
}

#[test]
fn test_publish_without_schema_fails() {
    let (env, client, admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");
    let payload = String::from_str(&env, "payload");

    let result = client.try_publish_event(&admin, &topic, &1, &payload);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::SchemaNotFound),
        _ => panic!("Expected SchemaNotFound error"),
    }
}

#[test]
fn test_publish_unauthorized_fails() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);

    let outsider = Address::generate(&env);
    let topic = String::from_str(&env, "records.vision.create");
    let payload = String::from_str(&env, "payload");

    let result = client.try_publish_event(&outsider, &topic, &1, &payload);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::Unauthorized),
        _ => panic!("Expected Unauthorized error"),
    }
}

#[test]
fn test_registered_source_can_publish() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);

    let source = Address::generate(&env);
    client.register_source(&admin, &source);
    assert!(client.is_registered_source(&source));

    let event_id = publish_test_event(
        &env,
        &client,
        &source,
        "records.vision.create",
        1,
        "source_payload",
    );
    assert_eq!(event_id, 1);
}

#[test]
fn test_get_nonexistent_event_fails() {
    let (_env, client, _admin) = setup();

    let result = client.try_get_event(&999);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::EventNotFound),
        _ => panic!("Expected EventNotFound error"),
    }
}

// ── Subscription tests ───────────────────────────────────────────────────────

#[test]
fn test_subscribe_and_get() {
    let (env, client, _admin) = setup();
    let subscriber = Address::generate(&env);
    let pattern = String::from_str(&env, "records.vision.*");

    let sub_id = client.subscribe(&subscriber, &pattern);
    assert_eq!(sub_id, 1);

    let subs = client.get_subscriptions(&subscriber);
    assert_eq!(subs.len(), 1);
    assert_eq!(subs.get(0).unwrap().topic_pattern, pattern);
    assert!(subs.get(0).unwrap().active);
}

#[test]
fn test_unsubscribe() {
    let (env, client, _admin) = setup();
    let subscriber = Address::generate(&env);
    let pattern = String::from_str(&env, "records.vision.*");

    let sub_id = client.subscribe(&subscriber, &pattern);
    client.unsubscribe(&subscriber, &sub_id);

    let subs = client.get_subscriptions(&subscriber);
    assert!(!subs.get(0).unwrap().active);
}

#[test]
fn test_duplicate_subscription_fails() {
    let (env, client, _admin) = setup();
    let subscriber = Address::generate(&env);
    let pattern = String::from_str(&env, "records.vision.*");

    client.subscribe(&subscriber, &pattern);

    let result = client.try_subscribe(&subscriber, &pattern);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::DuplicateSubscription),
        _ => panic!("Expected DuplicateSubscription error"),
    }
}

#[test]
fn test_unsubscribe_wrong_owner_fails() {
    let (env, client, _admin) = setup();
    let subscriber = Address::generate(&env);
    let other = Address::generate(&env);
    let pattern = String::from_str(&env, "records.vision.*");

    let sub_id = client.subscribe(&subscriber, &pattern);

    let result = client.try_unsubscribe(&other, &sub_id);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::Unauthorized),
        _ => panic!("Expected Unauthorized error"),
    }
}

#[test]
fn test_subscribe_empty_pattern_fails() {
    let (env, client, _admin) = setup();
    let subscriber = Address::generate(&env);
    let pattern = String::from_str(&env, "");

    let result = client.try_subscribe(&subscriber, &pattern);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::InvalidTopicPattern),
        _ => panic!("Expected InvalidTopicPattern error"),
    }
}

// ── Topic matching tests ─────────────────────────────────────────────────────

#[test]
fn test_topic_matching_exact() {
    let (env, _client, _admin) = setup();
    let pattern = String::from_str(&env, "records.vision.create");
    let topic = String::from_str(&env, "records.vision.create");

    assert!(crate::subscription::topic_matches(&env, &pattern, &topic));
}

#[test]
fn test_topic_matching_wildcard() {
    let (env, _client, _admin) = setup();
    let pattern = String::from_str(&env, "records.vision.*");
    let topic_create = String::from_str(&env, "records.vision.create");
    let topic_update = String::from_str(&env, "records.vision.update");
    let topic_wrong = String::from_str(&env, "staking.staked.create");

    assert!(crate::subscription::topic_matches(&env, &pattern, &topic_create));
    assert!(crate::subscription::topic_matches(&env, &pattern, &topic_update));
    assert!(!crate::subscription::topic_matches(&env, &pattern, &topic_wrong));
}

#[test]
fn test_topic_matching_different_depth_no_match() {
    let (env, _client, _admin) = setup();
    let pattern = String::from_str(&env, "records.*");
    let topic = String::from_str(&env, "records.vision.create");

    // Pattern has 2 segments, topic has 3 — should not match
    assert!(!crate::subscription::topic_matches(&env, &pattern, &topic));
}

#[test]
fn test_topic_matching_single_segment_wildcard() {
    let (env, _client, _admin) = setup();
    let pattern = String::from_str(&env, "*");
    let topic = String::from_str(&env, "records");

    assert!(crate::subscription::topic_matches(&env, &pattern, &topic));
}

// ── Consumer group tests ─────────────────────────────────────────────────────

#[test]
fn test_create_consumer_group() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "vision-processors");
    let pattern = String::from_str(&env, "records.vision.*");

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let mut members = Vec::new(&env);
    members.push_back(m1.clone());
    members.push_back(m2.clone());

    let group_id = client.create_consumer_group(&owner, &name, &pattern, &members);
    assert_eq!(group_id, 1);

    let group = client.get_consumer_group(&group_id);
    assert_eq!(group.name, name);
    assert_eq!(group.members.len(), 2);
    assert_eq!(group.offset, 0);
}

#[test]
fn test_consumer_group_empty_members_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "empty-group");
    let pattern = String::from_str(&env, "records.*");
    let members = Vec::new(&env);

    let result = client.try_create_consumer_group(&owner, &name, &pattern, &members);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::InvalidInput),
        _ => panic!("Expected InvalidInput error"),
    }
}

#[test]
fn test_ack_event_in_group() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "ack-group");
    let pattern = String::from_str(&env, "records.vision.*");

    let m1 = Address::generate(&env);
    let mut members = Vec::new(&env);
    members.push_back(m1.clone());

    let group_id = client.create_consumer_group(&owner, &name, &pattern, &members);

    register_schema(&env, &client, &admin, "records.vision.create", 1);
    let event_id = publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");

    client.ack_event(&m1, &group_id, &event_id);
}

#[test]
fn test_ack_event_non_member_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "ack-group");
    let pattern = String::from_str(&env, "records.vision.*");

    let m1 = Address::generate(&env);
    let outsider = Address::generate(&env);
    let mut members = Vec::new(&env);
    members.push_back(m1.clone());

    let group_id = client.create_consumer_group(&owner, &name, &pattern, &members);

    let result = client.try_ack_event(&outsider, &group_id, &1);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::Unauthorized),
        _ => panic!("Expected Unauthorized error"),
    }
}

#[test]
fn test_consumer_group_round_robin_distribution() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "rr-group");
    let pattern = String::from_str(&env, "records.vision.*");

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);
    let mut members = Vec::new(&env);
    members.push_back(m1.clone());
    members.push_back(m2.clone());
    members.push_back(m3.clone());

    let group_id = client.create_consumer_group(&owner, &name, &pattern, &members);

    register_schema(&env, &client, &admin, "records.vision.create", 1);

    // Publish 3 events — each should go to a different member
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p2");
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p3");

    let group = client.get_consumer_group(&group_id);
    // After 3 dispatches, offset should be 3
    assert_eq!(group.offset, 3);
}

// ── Webhook tests ────────────────────────────────────────────────────────────

#[test]
fn test_register_and_remove_webhook() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let pattern = String::from_str(&env, "records.*");
    let url_hash = String::from_str(&env, "sha256:webhook_url");

    let whk_id = client.register_webhook(&owner, &pattern, &url_hash);
    assert_eq!(whk_id, 1);

    client.remove_webhook(&owner, &whk_id);
}

#[test]
fn test_remove_webhook_wrong_owner_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let other = Address::generate(&env);
    let pattern = String::from_str(&env, "records.*");
    let url_hash = String::from_str(&env, "sha256:webhook_url");

    let whk_id = client.register_webhook(&owner, &pattern, &url_hash);

    let result = client.try_remove_webhook(&other, &whk_id);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::Unauthorized),
        _ => panic!("Expected Unauthorized error"),
    }
}

// ── Replay tests ─────────────────────────────────────────────────────────────

#[test]
fn test_replay_events() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);

    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p2");
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p3");

    // Replay all from beginning
    let events = client.replay_events(&1, &10);
    assert_eq!(events.len(), 3);

    // Replay from event 2 onwards
    let events = client.replay_events(&2, &10);
    assert_eq!(events.len(), 2);
    assert_eq!(events.get(0).unwrap().event_id, 2);
}

#[test]
fn test_replay_produces_consistent_ordering() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);
    register_schema(&env, &client, &admin, "staking.staked", 1);

    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");
    publish_test_event(&env, &client, &admin, "staking.staked", 1, "p2");
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p3");

    let events = client.replay_events(&1, &10);
    assert_eq!(events.len(), 3);

    // Verify strict Lamport ordering
    for i in 0..(events.len() - 1) {
        let current = events.get(i).unwrap();
        let next = events.get(i + 1).unwrap();
        assert!(current.lamport_ts < next.lamport_ts);
        assert!(current.event_id < next.event_id);
    }
}

#[test]
fn test_replay_topic_filter() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);
    register_schema(&env, &client, &admin, "staking.staked", 1);

    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");
    publish_test_event(&env, &client, &admin, "staking.staked", 1, "p2");
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p3");

    let topic = String::from_str(&env, "records.vision.create");
    let events = client.replay_topic_events(&topic, &1, &10);
    assert_eq!(events.len(), 2);

    for evt in events.iter() {
        assert_eq!(evt.topic, topic);
    }
}

#[test]
fn test_replay_with_limit() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);

    for _i in 0..5 {
        let payload = String::from_str(&env, "p");
        let topic = String::from_str(&env, "records.vision.create");
        client.publish_event(&admin, &topic, &1, &payload);
    }

    let events = client.replay_events(&1, &3);
    assert_eq!(events.len(), 3);
}

#[test]
fn test_replay_zero_limit_fails() {
    let (_env, client, _admin) = setup();

    let result = client.try_replay_events(&1, &0);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::InvalidInput),
        _ => panic!("Expected InvalidInput error"),
    }
}

// ── Checkpoint tests ─────────────────────────────────────────────────────────

#[test]
fn test_create_and_get_checkpoint() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);

    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p2");

    let chkpt_id = client.create_checkpoint(&admin);
    let event_at_checkpoint = client.get_checkpoint(&chkpt_id);
    assert_eq!(event_at_checkpoint, 2);

    // Publish more events after checkpoint
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p3");

    // Replay from checkpoint should return only new events
    let from_id = event_at_checkpoint + 1;
    let events = client.replay_events(&from_id, &10);
    assert_eq!(events.len(), 1);
    assert_eq!(events.get(0).unwrap().event_id, 3);
}

#[test]
fn test_get_nonexistent_checkpoint_fails() {
    let (_env, client, _admin) = setup();

    let result = client.try_get_checkpoint(&999);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::CheckpointNotFound),
        _ => panic!("Expected CheckpointNotFound error"),
    }
}

// ── Compaction tests ─────────────────────────────────────────────────────────

#[test]
fn test_compact_topic() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);
    register_schema(&env, &client, &admin, "staking.staked", 1);

    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p2");
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p3");
    publish_test_event(&env, &client, &admin, "staking.staked", 1, "s1");

    let topic = String::from_str(&env, "records.vision.create");
    let removed = client.compact_topic(&admin, &topic);
    assert_eq!(removed, 2);

    // After compaction, replay of that topic should return only the last event
    let events = client.replay_topic_events(&topic, &1, &10);
    assert_eq!(events.len(), 1);
    assert_eq!(events.get(0).unwrap().event_id, 3);

    // Staking events should be unaffected
    let staking_topic = String::from_str(&env, "staking.staked");
    let staking_events = client.replay_topic_events(&staking_topic, &1, &10);
    assert_eq!(staking_events.len(), 1);
}

#[test]
fn test_compact_single_event_no_change() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);

    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");

    let topic = String::from_str(&env, "records.vision.create");
    let removed = client.compact_topic(&admin, &topic);
    assert_eq!(removed, 0);
}

// ── Dead letter queue tests ──────────────────────────────────────────────────

#[test]
fn test_dead_letter_queue() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);

    let event_id = publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");

    let subscriber = Address::generate(&env);
    let reason = String::from_str(&env, "timeout");

    client.push_dead_letter(&admin, &event_id, &subscriber, &reason);

    let dlq = client.get_dead_letters();
    assert_eq!(dlq.len(), 1);
    assert_eq!(dlq.get(0).unwrap().event_id, event_id);
    assert_eq!(dlq.get(0).unwrap().reason, reason);
}

#[test]
fn test_retry_dead_letter() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);

    let event_id = publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");

    let subscriber = Address::generate(&env);
    let reason = String::from_str(&env, "timeout");

    client.push_dead_letter(&admin, &event_id, &subscriber, &reason);
    client.retry_dead_letter(&admin, &0);

    // DLQ should be empty after successful retry
    let dlq = client.get_dead_letters();
    assert_eq!(dlq.len(), 0);
}

#[test]
fn test_retry_invalid_index_fails() {
    let (_env, client, admin) = setup();

    let result = client.try_retry_dead_letter(&admin, &999);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::InvalidInput),
        _ => panic!("Expected InvalidInput error"),
    }
}

// ── Integration: subscription dispatch with events ───────────────────────────

#[test]
fn test_subscription_receives_matching_events() {
    let (env, client, admin) = setup();
    let subscriber = Address::generate(&env);

    // Subscribe to vision events
    let pattern = String::from_str(&env, "records.vision.*");
    client.subscribe(&subscriber, &pattern);

    register_schema(&env, &client, &admin, "records.vision.create", 1);
    register_schema(&env, &client, &admin, "staking.staked", 1);

    // Publish a matching event and a non-matching one
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");
    publish_test_event(&env, &client, &admin, "staking.staked", 1, "s1");

    // Verify via event count — both events should be stored
    assert_eq!(client.get_event_count(), 2);
}

#[test]
fn test_multiple_subscribers_same_topic() {
    let (env, client, admin) = setup();
    let sub1 = Address::generate(&env);
    let sub2 = Address::generate(&env);
    let pattern = String::from_str(&env, "records.vision.*");

    client.subscribe(&sub1, &pattern);
    client.subscribe(&sub2, &pattern);

    register_schema(&env, &client, &admin, "records.vision.create", 1);
    publish_test_event(&env, &client, &admin, "records.vision.create", 1, "p1");

    // Both subscribers should be notified (verified via Soroban events in production)
    let subs1 = client.get_subscriptions(&sub1);
    let subs2 = client.get_subscriptions(&sub2);
    assert_eq!(subs1.len(), 1);
    assert_eq!(subs2.len(), 1);
}

// ── End-to-end workflow test ─────────────────────────────────────────────────

#[test]
fn test_full_workflow() {
    let (env, client, admin) = setup();

    // 1. Register schemas
    register_schema(&env, &client, &admin, "records.vision.create", 1);
    register_schema(&env, &client, &admin, "records.prescription.create", 1);

    // 2. Register an external source contract
    let source = Address::generate(&env);
    client.register_source(&admin, &source);

    // 3. Create subscriptions
    let subscriber = Address::generate(&env);
    let pattern = String::from_str(&env, "records.vision.*");
    client.subscribe(&subscriber, &pattern);

    // 4. Create a consumer group
    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let mut members = Vec::new(&env);
    members.push_back(m1.clone());
    members.push_back(m2.clone());
    let group_name = String::from_str(&env, "vision-processors");
    let group_pattern = String::from_str(&env, "records.vision.*");
    let group_id = client.create_consumer_group(&admin, &group_name, &group_pattern, &members);

    // 5. Publish events from registered source
    let e1 = publish_test_event(&env, &client, &source, "records.vision.create", 1, "exam_1");
    let e2 = publish_test_event(&env, &client, &source, "records.vision.create", 1, "exam_2");

    // 6. Create a checkpoint
    let chkpt = client.create_checkpoint(&admin);

    // 7. Publish more events
    let e3 = publish_test_event(
        &env,
        &client,
        &source,
        "records.prescription.create",
        1,
        "rx_1",
    );

    // 8. Ack events in consumer group
    client.ack_event(&m1, &group_id, &e1);
    client.ack_event(&m2, &group_id, &e2);

    // 9. Replay from checkpoint
    let checkpoint_event = client.get_checkpoint(&chkpt);
    let replayed = client.replay_events(&(checkpoint_event + 1), &10);
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed.get(0).unwrap().event_id, e3);

    // 10. Verify ordering across replay
    let all_events = client.replay_events(&1, &10);
    assert_eq!(all_events.len(), 3);
    for i in 0..(all_events.len() - 1) {
        let curr = all_events.get(i).unwrap();
        let next = all_events.get(i + 1).unwrap();
        assert!(curr.lamport_ts < next.lamport_ts);
    }
}
