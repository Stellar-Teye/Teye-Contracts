#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Zero-Value Parameter Passing Edge Cases
//!
//! Sends zero, empty, or blank inputs to critical state-modifying functions
//! and verifies consistent handling and correct revert behavior.

use events::{EventError, EventStreamContract, EventStreamContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

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

// ==========================================================================
// Schema Registration — Zero-Value Edge Cases
// ==========================================================================

#[test]
fn test_register_schema_empty_topic_rejected() {
    let (env, client, admin) = setup();
    let empty_topic = String::from_str(&env, "");
    let hash = String::from_str(&env, "sha256:abc");

    let result = client.try_register_schema(&admin, &empty_topic, &1, &hash);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_register_schema_empty_hash_rejected() {
    let (env, client, admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");
    let empty_hash = String::from_str(&env, "");

    let result = client.try_register_schema(&admin, &topic, &1, &empty_hash);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_register_schema_version_zero_rejected() {
    let (env, client, admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");
    let hash = String::from_str(&env, "sha256:abc");

    let result = client.try_register_schema(&admin, &topic, &0, &hash);
    assert_eq!(result, Err(Ok(EventError::InvalidSchema)));
}

#[test]
fn test_register_schema_both_empty_rejected() {
    let (env, client, admin) = setup();
    let empty_topic = String::from_str(&env, "");
    let empty_hash = String::from_str(&env, "");

    let result = client.try_register_schema(&admin, &empty_topic, &0, &empty_hash);
    // Empty topic/hash check comes before version check
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

// ==========================================================================
// Event Publishing — Zero-Value Edge Cases
// ==========================================================================

#[test]
fn test_publish_event_empty_topic_rejected() {
    let (env, client, admin) = setup();
    let empty_topic = String::from_str(&env, "");
    let payload = String::from_str(&env, "payload");

    let result = client.try_publish_event(&admin, &empty_topic, &1, &payload);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_publish_event_schema_version_zero_rejected() {
    let (env, client, admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");
    let payload = String::from_str(&env, "payload");

    // No schema registered at version 0
    let result = client.try_publish_event(&admin, &topic, &0, &payload);
    assert_eq!(result, Err(Ok(EventError::SchemaNotFound)));
}

#[test]
fn test_publish_event_empty_payload_hash_accepted() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);
    let topic = String::from_str(&env, "records.vision.create");
    let empty_payload = String::from_str(&env, "");

    // payload_hash is a content-addressable hash; contract does not validate it
    let event_id = client.publish_event(&admin, &topic, &1, &empty_payload);
    assert_eq!(event_id, 1);
}

#[test]
fn test_get_event_id_zero_not_found() {
    let (_env, client, _admin) = setup();

    let result = client.try_get_event(&0);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::EventNotFound),
        _ => panic!("Expected EventNotFound error"),
    }
}

// ==========================================================================
// Subscription — Zero-Value Edge Cases
// ==========================================================================

#[test]
fn test_subscribe_empty_pattern_rejected() {
    let (env, client, _admin) = setup();
    let subscriber = Address::generate(&env);
    let empty_pattern = String::from_str(&env, "");

    let result = client.try_subscribe(&subscriber, &empty_pattern);
    assert_eq!(result, Err(Ok(EventError::InvalidTopicPattern)));
}

#[test]
fn test_unsubscribe_id_zero_not_found() {
    let (env, client, _admin) = setup();
    let subscriber = Address::generate(&env);

    // Subscription ID 0 was never issued (counter starts at 1)
    let result = client.try_unsubscribe(&subscriber, &0);
    assert_eq!(result, Err(Ok(EventError::SubscriptionNotFound)));
}

#[test]
fn test_get_subscriptions_fresh_address_returns_empty() {
    let (env, client, _admin) = setup();
    let fresh_addr = Address::generate(&env);

    let subs = client.get_subscriptions(&fresh_addr);
    assert_eq!(subs.len(), 0);
}

// ==========================================================================
// Consumer Groups — Zero-Value Edge Cases
// ==========================================================================

#[test]
fn test_create_consumer_group_empty_name_rejected() {
    let (env, client, admin) = setup();
    let empty_name = String::from_str(&env, "");
    let pattern = String::from_str(&env, "records.*");
    let mut members: Vec<Address> = Vec::new(&env);
    members.push_back(Address::generate(&env));

    let result = client.try_create_consumer_group(&admin, &empty_name, &pattern, &members);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_create_consumer_group_empty_pattern_rejected() {
    let (env, client, admin) = setup();
    let name = String::from_str(&env, "group1");
    let empty_pattern = String::from_str(&env, "");
    let mut members: Vec<Address> = Vec::new(&env);
    members.push_back(Address::generate(&env));

    let result = client.try_create_consumer_group(&admin, &name, &empty_pattern, &members);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_create_consumer_group_empty_members_rejected() {
    let (env, client, admin) = setup();
    let name = String::from_str(&env, "group1");
    let pattern = String::from_str(&env, "records.*");
    let empty_members: Vec<Address> = Vec::new(&env);

    let result = client.try_create_consumer_group(&admin, &name, &pattern, &empty_members);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_create_consumer_group_all_empty_rejected() {
    let (env, client, admin) = setup();
    let empty_name = String::from_str(&env, "");
    let empty_pattern = String::from_str(&env, "");
    let empty_members: Vec<Address> = Vec::new(&env);

    let result =
        client.try_create_consumer_group(&admin, &empty_name, &empty_pattern, &empty_members);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_get_consumer_group_id_zero_not_found() {
    let (_env, client, _admin) = setup();

    let result = client.try_get_consumer_group(&0);
    match result {
        Err(Ok(e)) => assert_eq!(e, EventError::ConsumerGroupNotFound),
        _ => panic!("Expected ConsumerGroupNotFound error"),
    }
}

#[test]
fn test_ack_event_group_id_zero_not_found() {
    let (env, client, _admin) = setup();
    let consumer = Address::generate(&env);

    let result = client.try_ack_event(&consumer, &0, &1);
    assert_eq!(result, Err(Ok(EventError::ConsumerGroupNotFound)));
}

#[test]
fn test_ack_event_event_id_zero() {
    let (env, client, admin) = setup();
    let member = Address::generate(&env);
    let name = String::from_str(&env, "group1");
    let pattern = String::from_str(&env, "records.*");
    let mut members: Vec<Address> = Vec::new(&env);
    members.push_back(member.clone());

    let group_id = client.create_consumer_group(&admin, &name, &pattern, &members);

    // Ack event_id=0 should succeed — it's a valid ack (just stores the flag)
    client.ack_event(&member, &group_id, &0);
}

// ==========================================================================
// Webhook Registration — Zero-Value Edge Cases
// ==========================================================================

#[test]
fn test_register_webhook_empty_pattern_rejected() {
    let (env, client, admin) = setup();
    let empty_pattern = String::from_str(&env, "");
    let url_hash = String::from_str(&env, "sha256:webhook_url");

    let result = client.try_register_webhook(&admin, &empty_pattern, &url_hash);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_register_webhook_empty_url_hash_rejected() {
    let (env, client, admin) = setup();
    let pattern = String::from_str(&env, "records.*");
    let empty_url = String::from_str(&env, "");

    let result = client.try_register_webhook(&admin, &pattern, &empty_url);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_register_webhook_both_empty_rejected() {
    let (env, client, admin) = setup();
    let empty_pattern = String::from_str(&env, "");
    let empty_url = String::from_str(&env, "");

    let result = client.try_register_webhook(&admin, &empty_pattern, &empty_url);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_remove_webhook_id_zero_not_found() {
    let (_env, client, admin) = setup();

    let result = client.try_remove_webhook(&admin, &0);
    assert_eq!(result, Err(Ok(EventError::SubscriptionNotFound)));
}

// ==========================================================================
// Replay & Compaction — Zero-Value Edge Cases
// ==========================================================================

#[test]
fn test_replay_events_limit_zero_rejected() {
    let (_env, client, _admin) = setup();

    let result = client.try_replay_events(&0, &0);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_replay_events_from_id_zero_returns_all() {
    let (env, client, admin) = setup();
    register_schema(&env, &client, &admin, "records.vision.create", 1);
    let topic = String::from_str(&env, "records.vision.create");
    let payload = String::from_str(&env, "p1");
    client.publish_event(&admin, &topic, &1, &payload);

    let events = client.replay_events(&0, &10);
    assert_eq!(events.len(), 1);
}

#[test]
fn test_replay_topic_empty_topic_rejected() {
    let (env, client, _admin) = setup();
    let empty_topic = String::from_str(&env, "");

    let result = client.try_replay_topic_events(&empty_topic, &0, &10);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_replay_topic_limit_zero_rejected() {
    let (env, client, _admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");

    let result = client.try_replay_topic_events(&topic, &0, &0);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_compact_topic_empty_topic_rejected() {
    let (env, client, admin) = setup();
    let empty_topic = String::from_str(&env, "");

    let result = client.try_compact_topic(&admin, &empty_topic);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_compact_topic_no_events_returns_zero() {
    let (env, client, admin) = setup();
    let topic = String::from_str(&env, "records.vision.create");

    let removed = client.compact_topic(&admin, &topic);
    assert_eq!(removed, 0);
}

// ==========================================================================
// Checkpoint — Zero-Value Edge Cases
// ==========================================================================

#[test]
fn test_get_checkpoint_id_zero_not_found() {
    let (_env, client, _admin) = setup();

    let result = client.try_get_checkpoint(&0);
    assert_eq!(result, Err(Ok(EventError::CheckpointNotFound)));
}

#[test]
fn test_create_checkpoint_at_zero_events() {
    let (_env, client, admin) = setup();

    let chkpt_id = client.create_checkpoint(&admin);
    let event_id = client.get_checkpoint(&chkpt_id);
    assert_eq!(event_id, 0);
}

// ==========================================================================
// Dead Letter Queue — Zero-Value Edge Cases
// ==========================================================================

#[test]
fn test_push_dead_letter_event_id_zero() {
    let (env, client, admin) = setup();
    let subscriber = Address::generate(&env);
    let reason = String::from_str(&env, "timeout");

    // event_id=0 doesn't correspond to a published event, but push_dead_letter
    // records it for later retry
    client.push_dead_letter(&admin, &0, &subscriber, &reason);

    let dlq = client.get_dead_letters();
    assert_eq!(dlq.len(), 1);
    assert_eq!(dlq.get(0).unwrap().event_id, 0);
}

#[test]
fn test_push_dead_letter_empty_reason() {
    let (env, client, admin) = setup();
    let subscriber = Address::generate(&env);
    let empty_reason = String::from_str(&env, "");

    // Empty reason string is accepted — it's metadata, not a key
    client.push_dead_letter(&admin, &1, &subscriber, &empty_reason);

    let dlq = client.get_dead_letters();
    assert_eq!(dlq.len(), 1);
}

#[test]
fn test_retry_dead_letter_index_zero_when_empty() {
    let (_env, client, admin) = setup();

    // DLQ is empty, index 0 is out of bounds
    let result = client.try_retry_dead_letter(&admin, &0);
    assert_eq!(result, Err(Ok(EventError::InvalidInput)));
}

#[test]
fn test_get_dead_letters_initially_empty() {
    let (_env, client, _admin) = setup();

    let dlq = client.get_dead_letters();
    assert_eq!(dlq.len(), 0);
}

// ==========================================================================
// Source Registration — Zero-Value Edge Cases
// ==========================================================================

#[test]
fn test_is_registered_source_fresh_address_returns_false() {
    let (env, client, _admin) = setup();
    let fresh = Address::generate(&env);

    assert!(!client.is_registered_source(&fresh));
}

// ==========================================================================
// Uninitialized Contract — Zero-Value Edge Cases
// ==========================================================================

#[test]
fn test_operations_before_initialize_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventStreamContract, ());
    let client = EventStreamContractClient::new(&env, &contract_id);
    let caller = Address::generate(&env);

    // publish_event on uninitialized contract
    let topic = String::from_str(&env, "records.vision.create");
    let payload = String::from_str(&env, "payload");
    assert_eq!(
        client.try_publish_event(&caller, &topic, &1, &payload),
        Err(Ok(EventError::NotInitialized))
    );

    // register_schema on uninitialized contract
    let hash = String::from_str(&env, "sha256:abc");
    assert_eq!(
        client.try_register_schema(&caller, &topic, &1, &hash),
        Err(Ok(EventError::NotInitialized))
    );

    // subscribe on uninitialized contract
    let pattern = String::from_str(&env, "records.*");
    assert_eq!(
        client.try_subscribe(&caller, &pattern),
        Err(Ok(EventError::NotInitialized))
    );

    // get_event_count on uninitialized contract
    assert_eq!(
        client.try_get_event_count(),
        Err(Ok(EventError::NotInitialized))
    );

    // replay on uninitialized contract
    assert_eq!(
        client.try_replay_events(&0, &10),
        Err(Ok(EventError::NotInitialized))
    );
}
