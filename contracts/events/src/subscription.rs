//! Subscription management: topic-based filtering, consumer groups, and webhooks.
//!
//! Subscriptions use hierarchical topic patterns with wildcard support:
//! - `records.vision.create` — exact match
//! - `records.vision.*` — matches any single segment after `records.vision.`
//! - `records.*` — matches any single segment after `records.`
//!
//! Consumer groups allow multiple consumers to share event processing load.
//! Each event matching the group topic is assigned to exactly one member using
//! round-robin distribution based on the group's internal offset counter.

use crate::{EventEnvelope, EventError};
use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Vec};

// ── Storage key constants ────────────────────────────────────────────────────

const SUB_CTR: soroban_sdk::Symbol = symbol_short!("SUB_CTR");
const GRP_CTR: soroban_sdk::Symbol = symbol_short!("GRP_CTR");
const WHK_CTR: soroban_sdk::Symbol = symbol_short!("WHK_CTR");

const MAX_WEBHOOKS_PER_USER: u32 = 10;

// ── Types ────────────────────────────────────────────────────────────────────

/// A topic-based subscription owned by a single subscriber.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Subscription {
    pub id: u64,
    pub subscriber: Address,
    pub topic_pattern: String,
    pub created_at: u64,
    pub active: bool,
}

/// A consumer group that distributes event processing across members.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ConsumerGroup {
    pub id: u64,
    pub owner: Address,
    pub name: String,
    pub topic_pattern: String,
    pub members: Vec<Address>,
    pub offset: u64,
    pub created_at: u64,
}

/// A webhook registration for push-based event notification.
#[contracttype]
#[derive(Clone, Debug)]
pub struct WebhookRegistration {
    pub id: u64,
    pub owner: Address,
    pub topic_pattern: String,
    pub url_hash: String,
    pub created_at: u64,
    pub active: bool,
}

/// Tracks which consumer in a group should receive the next event.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ConsumerOffset {
    pub group_id: u64,
    pub last_event_id: u64,
    pub consumer_index: u32,
}

// ── Storage key helpers ──────────────────────────────────────────────────────

fn sub_key(sub_id: u64) -> (soroban_sdk::Symbol, u64) {
    (symbol_short!("SUB"), sub_id)
}

fn user_subs_key(subscriber: &Address) -> (soroban_sdk::Symbol, Address) {
    (symbol_short!("USR_SUBS"), subscriber.clone())
}

fn group_key(group_id: u64) -> (soroban_sdk::Symbol, u64) {
    (symbol_short!("GRP"), group_id)
}

fn webhook_key(webhook_id: u64) -> (soroban_sdk::Symbol, u64) {
    (symbol_short!("WHK"), webhook_id)
}

fn user_webhooks_key(owner: &Address) -> (soroban_sdk::Symbol, Address) {
    (symbol_short!("USR_WHK"), owner.clone())
}

fn ack_key(group_id: u64, event_id: u64) -> (soroban_sdk::Symbol, u64, u64) {
    (symbol_short!("ACK"), group_id, event_id)
}

fn all_subs_key() -> soroban_sdk::Symbol {
    symbol_short!("ALL_SUBS")
}

fn all_groups_key() -> soroban_sdk::Symbol {
    symbol_short!("ALL_GRPS")
}

fn all_webhooks_key() -> soroban_sdk::Symbol {
    symbol_short!("ALL_WHKS")
}

// ── Subscription CRUD ────────────────────────────────────────────────────────

/// Create a new subscription for the given subscriber and topic pattern.
#[allow(clippy::arithmetic_side_effects)]
pub fn create_subscription(
    env: &Env,
    subscriber: &Address,
    topic_pattern: &String,
) -> Result<u64, EventError> {
    if topic_pattern.len() == 0 {
        return Err(EventError::InvalidTopicPattern);
    }

    // Check for duplicate active subscriptions on the same pattern
    let existing = get_subscriptions(env, subscriber);
    for sub in existing.iter() {
        if sub.topic_pattern == *topic_pattern && sub.active {
            return Err(EventError::DuplicateSubscription);
        }
    }

    let sub_id = next_sub_id(env);

    let subscription = Subscription {
        id: sub_id,
        subscriber: subscriber.clone(),
        topic_pattern: topic_pattern.clone(),
        created_at: env.ledger().timestamp(),
        active: true,
    };

    env.storage().persistent().set(&sub_key(sub_id), &subscription);

    // Append to user's subscription list
    let user_key = user_subs_key(subscriber);
    let mut user_subs: Vec<u64> = env
        .storage()
        .persistent()
        .get(&user_key)
        .unwrap_or(Vec::new(env));
    user_subs.push_back(sub_id);
    env.storage().persistent().set(&user_key, &user_subs);

    // Append to global subscription index
    let global_key = all_subs_key();
    let mut all_subs: Vec<u64> = env
        .storage()
        .persistent()
        .get(&global_key)
        .unwrap_or(Vec::new(env));
    all_subs.push_back(sub_id);
    env.storage().persistent().set(&global_key, &all_subs);

    env.events().publish(
        (symbol_short!("SUB_NEW"), subscriber.clone()),
        subscription.clone(),
    );

    Ok(sub_id)
}

/// Remove (deactivate) a subscription. Only the original subscriber can do this.
pub fn remove_subscription(
    env: &Env,
    subscriber: &Address,
    subscription_id: u64,
) -> Result<(), EventError> {
    let key = sub_key(subscription_id);
    let mut sub: Subscription = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EventError::SubscriptionNotFound)?;

    if sub.subscriber != *subscriber {
        return Err(EventError::Unauthorized);
    }

    sub.active = false;
    env.storage().persistent().set(&key, &sub);

    env.events().publish(
        (symbol_short!("SUB_REM"), subscriber.clone()),
        subscription_id,
    );

    Ok(())
}

/// Return all subscriptions for a given address.
pub fn get_subscriptions(env: &Env, subscriber: &Address) -> Vec<Subscription> {
    let user_key = user_subs_key(subscriber);
    let sub_ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&user_key)
        .unwrap_or(Vec::new(env));

    let mut result = Vec::new(env);
    for id in sub_ids.iter() {
        if let Some(sub) = env.storage().persistent().get::<_, Subscription>(&sub_key(id)) {
            result.push_back(sub);
        }
    }
    result
}

// ── Consumer groups ──────────────────────────────────────────────────────────

/// Create a consumer group that distributes events across its members.
#[allow(clippy::arithmetic_side_effects)]
pub fn create_consumer_group(
    env: &Env,
    owner: &Address,
    group_name: &String,
    topic_pattern: &String,
    members: &Vec<Address>,
) -> Result<u64, EventError> {
    if group_name.len() == 0 || topic_pattern.len() == 0 {
        return Err(EventError::InvalidInput);
    }

    if members.is_empty() {
        return Err(EventError::InvalidInput);
    }

    let group_id = next_group_id(env);

    let group = ConsumerGroup {
        id: group_id,
        owner: owner.clone(),
        name: group_name.clone(),
        topic_pattern: topic_pattern.clone(),
        members: members.clone(),
        offset: 0,
        created_at: env.ledger().timestamp(),
    };

    env.storage().persistent().set(&group_key(group_id), &group);

    // Append to global group index
    let global_key = all_groups_key();
    let mut all_groups: Vec<u64> = env
        .storage()
        .persistent()
        .get(&global_key)
        .unwrap_or(Vec::new(env));
    all_groups.push_back(group_id);
    env.storage().persistent().set(&global_key, &all_groups);

    env.events().publish(
        (symbol_short!("GRP_NEW"), owner.clone()),
        group.clone(),
    );

    Ok(group_id)
}

/// Retrieve a consumer group by its ID.
pub fn get_consumer_group(env: &Env, group_id: u64) -> Result<ConsumerGroup, EventError> {
    env.storage()
        .persistent()
        .get(&group_key(group_id))
        .ok_or(EventError::ConsumerGroupNotFound)
}

/// Acknowledge that a consumer in a group has processed an event.
pub fn ack_event(
    env: &Env,
    consumer: &Address,
    group_id: u64,
    event_id: u64,
) -> Result<(), EventError> {
    let group: ConsumerGroup = env
        .storage()
        .persistent()
        .get(&group_key(group_id))
        .ok_or(EventError::ConsumerGroupNotFound)?;

    // Verify the consumer is a member of this group
    let mut is_member = false;
    for member in group.members.iter() {
        if member == *consumer {
            is_member = true;
            break;
        }
    }
    if !is_member {
        return Err(EventError::Unauthorized);
    }

    let key = ack_key(group_id, event_id);
    env.storage().persistent().set(&key, &true);

    env.events().publish(
        (symbol_short!("EVT_ACK"), consumer.clone(), group_id),
        event_id,
    );

    Ok(())
}

// ── Webhook registration ─────────────────────────────────────────────────────

/// Register a webhook for push-based event notification.
/// `url_hash` is a hash of the actual URL (the URL itself is not stored on-chain
/// for privacy/security reasons).
#[allow(clippy::arithmetic_side_effects)]
pub fn register_webhook(
    env: &Env,
    owner: &Address,
    topic_pattern: &String,
    url_hash: &String,
) -> Result<u64, EventError> {
    if topic_pattern.len() == 0 || url_hash.len() == 0 {
        return Err(EventError::InvalidInput);
    }

    // Enforce per-user webhook limit
    let user_key = user_webhooks_key(owner);
    let existing: Vec<u64> = env
        .storage()
        .persistent()
        .get(&user_key)
        .unwrap_or(Vec::new(env));
    if existing.len() >= MAX_WEBHOOKS_PER_USER {
        return Err(EventError::WebhookLimitExceeded);
    }

    let webhook_id = next_webhook_id(env);

    let webhook = WebhookRegistration {
        id: webhook_id,
        owner: owner.clone(),
        topic_pattern: topic_pattern.clone(),
        url_hash: url_hash.clone(),
        created_at: env.ledger().timestamp(),
        active: true,
    };

    env.storage().persistent().set(&webhook_key(webhook_id), &webhook);

    // Append to user's webhook list
    let mut user_whks: Vec<u64> = env
        .storage()
        .persistent()
        .get(&user_key)
        .unwrap_or(Vec::new(env));
    user_whks.push_back(webhook_id);
    env.storage().persistent().set(&user_key, &user_whks);

    // Append to global webhook index
    let global_key = all_webhooks_key();
    let mut all_whks: Vec<u64> = env
        .storage()
        .persistent()
        .get(&global_key)
        .unwrap_or(Vec::new(env));
    all_whks.push_back(webhook_id);
    env.storage().persistent().set(&global_key, &all_whks);

    env.events().publish(
        (symbol_short!("WHK_NEW"), owner.clone()),
        webhook.clone(),
    );

    Ok(webhook_id)
}

/// Remove (deactivate) a webhook. Only the original owner can do this.
pub fn remove_webhook(
    env: &Env,
    owner: &Address,
    webhook_id: u64,
) -> Result<(), EventError> {
    let key = webhook_key(webhook_id);
    let mut webhook: WebhookRegistration = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EventError::SubscriptionNotFound)?;

    if webhook.owner != *owner {
        return Err(EventError::Unauthorized);
    }

    webhook.active = false;
    env.storage().persistent().set(&key, &webhook);

    env.events().publish(
        (symbol_short!("WHK_REM"), owner.clone()),
        webhook_id,
    );

    Ok(())
}

// ── Topic pattern matching ───────────────────────────────────────────────────

/// Match a concrete topic against a subscription pattern.
///
/// Supported patterns:
/// - `records.vision.create` — exact match only
/// - `records.vision.*` — matches `records.vision.<anything>` (single segment)
/// - `records.*` — matches `records.<anything>` (single segment)
/// - `*` — matches any single-segment topic
///
/// The matching operates on dot-delimited segments.
pub fn topic_matches(env: &Env, pattern: &String, topic: &String) -> bool {
    if *pattern == *topic {
        return true;
    }

    let pattern_str = string_to_segments(env, pattern);
    let topic_str = string_to_segments(env, topic);

    if pattern_str.len() != topic_str.len() {
        return false;
    }

    for i in 0..pattern_str.len() {
        let p_seg = pattern_str.get(i);
        let t_seg = topic_str.get(i);
        match (p_seg, t_seg) {
            (Some(p), Some(t)) => {
                let star = String::from_str(env, "*");
                if p != star && p != t {
                    return false;
                }
            }
            _ => return false,
        }
    }

    true
}

/// Split a dot-delimited string into segments.
fn string_to_segments(env: &Env, s: &String) -> Vec<String> {
    let mut segments = Vec::new(env);
    let len = s.len() as usize;

    if len == 0 {
        return segments;
    }

    // Copy the string contents into a fixed buffer and split on '.'
    let mut buf = [0u8; 256];
    s.copy_into_slice(&mut buf[..len]);

    let mut start = 0usize;
    let mut i = 0usize;
    while i < len {
        if buf[i] == b'.' {
            if i > start {
                if let Ok(segment_str) = core::str::from_utf8(&buf[start..i]) {
                    segments.push_back(String::from_str(env, segment_str));
                }
            }
            start = i + 1;
        }
        i += 1;
    }

    // Last segment after final dot (or entire string if no dots)
    if start < len {
        if let Ok(segment_str) = core::str::from_utf8(&buf[start..len]) {
            segments.push_back(String::from_str(env, segment_str));
        }
    }

    segments
}

// ── Event dispatching ────────────────────────────────────────────────────────

/// Dispatch an event to all matching subscriptions and consumer groups.
///
/// For individual subscriptions, a Soroban event is emitted per match so that
/// off-chain indexers can push notifications. For consumer groups, events are
/// assigned to exactly one member via round-robin.
#[allow(clippy::arithmetic_side_effects)]
pub fn dispatch_to_subscribers(env: &Env, envelope: &EventEnvelope) {
    // Dispatch to individual subscriptions
    let all_sub_ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&all_subs_key())
        .unwrap_or(Vec::new(env));

    for sub_id in all_sub_ids.iter() {
        if let Some(sub) = env
            .storage()
            .persistent()
            .get::<_, Subscription>(&sub_key(sub_id))
        {
            if sub.active && topic_matches(env, &sub.topic_pattern, &envelope.topic) {
                env.events().publish(
                    (symbol_short!("DISPATCH"), sub.subscriber.clone(), envelope.event_id),
                    envelope.topic.clone(),
                );
            }
        }
    }

    // Dispatch to consumer groups (round-robin)
    let all_group_ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&all_groups_key())
        .unwrap_or(Vec::new(env));

    for gid in all_group_ids.iter() {
        let key = group_key(gid);
        if let Some(mut group) = env
            .storage()
            .persistent()
            .get::<_, ConsumerGroup>(&key)
        {
            if topic_matches(env, &group.topic_pattern, &envelope.topic) {
                let member_count = group.members.len();
                if member_count > 0 {
                    let target_index = (group.offset as u32) % member_count;
                    if let Some(target) = group.members.get(target_index) {
                        env.events().publish(
                            (symbol_short!("GRP_DISP"), target.clone(), gid),
                            envelope.event_id,
                        );
                    }
                    group.offset += 1;
                    env.storage().persistent().set(&key, &group);
                }
            }
        }
    }

    // Notify webhooks
    let all_whk_ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&all_webhooks_key())
        .unwrap_or(Vec::new(env));

    for whk_id in all_whk_ids.iter() {
        if let Some(whk) = env
            .storage()
            .persistent()
            .get::<_, WebhookRegistration>(&webhook_key(whk_id))
        {
            if whk.active && topic_matches(env, &whk.topic_pattern, &envelope.topic) {
                env.events().publish(
                    (symbol_short!("WHK_FIRE"), whk.owner.clone(), whk_id),
                    envelope.event_id,
                );
            }
        }
    }
}

// ── Counter helpers ──────────────────────────────────────────────────────────

#[allow(clippy::arithmetic_side_effects)]
fn next_sub_id(env: &Env) -> u64 {
    let current: u64 = env.storage().instance().get(&SUB_CTR).unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&SUB_CTR, &next);
    next
}

#[allow(clippy::arithmetic_side_effects)]
fn next_group_id(env: &Env) -> u64 {
    let current: u64 = env.storage().instance().get(&GRP_CTR).unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&GRP_CTR, &next);
    next
}

#[allow(clippy::arithmetic_side_effects)]
fn next_webhook_id(env: &Env) -> u64 {
    let current: u64 = env.storage().instance().get(&WHK_CTR).unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&WHK_CTR, &next);
    next
}
