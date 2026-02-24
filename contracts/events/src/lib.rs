#![no_std]
#![allow(deprecated)]

pub mod registry;
pub mod replay;
pub mod subscription;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

// ── Storage key constants ────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");
const EVT_CTR: Symbol = symbol_short!("EVT_CTR");
const LAMPORT: Symbol = symbol_short!("LAMPORT");

// ── Error types ──────────────────────────────────────────────────────────────

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EventError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidInput = 4,
    TopicNotFound = 5,
    SubscriptionNotFound = 6,
    SchemaNotFound = 7,
    InvalidSchema = 8,
    CheckpointNotFound = 9,
    ConsumerGroupNotFound = 10,
    WebhookLimitExceeded = 11,
    DuplicateSubscription = 12,
    EventNotFound = 13,
    DeadLetterFull = 14,
    InvalidTopicPattern = 15,
}

// ── Core event types ─────────────────────────────────────────────────────────

/// A structured event envelope stored in the on-chain event log.
#[contracttype]
#[derive(Clone, Debug)]
pub struct EventEnvelope {
    pub event_id: u64,
    pub topic: String,
    pub schema_version: u32,
    pub source_contract: Address,
    pub payload_hash: String,
    pub lamport_ts: u64,
    pub ledger_ts: u64,
}

/// Severity levels for events, allowing consumers to filter by importance.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventSeverity {
    Info,
    Warning,
    Critical,
}

/// Categories that partition the event namespace at the highest level.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventCategory {
    Records,
    Staking,
    Identity,
    Admin,
    System,
}

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct EventStreamContract;

#[contractimpl]
impl EventStreamContract {
    /// Initialize the event streaming contract with an admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), EventError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(EventError::AlreadyInitialized);
        }

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);
        env.storage().instance().set(&EVT_CTR, &0u64);
        env.storage().instance().set(&LAMPORT, &0u64);

        env.events().publish(
            (symbol_short!("INIT"),),
            admin,
        );

        Ok(())
    }

    /// Check whether the contract has been initialized.
    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&INITIALIZED)
    }

    /// Return the current admin address.
    pub fn get_admin(env: Env) -> Result<Address, EventError> {
        env.storage()
            .instance()
            .get(&ADMIN)
            .ok_or(EventError::NotInitialized)
    }

    // ── Event publishing ─────────────────────────────────────────────────────

    /// Publish a new event into the streaming log.
    ///
    /// Only registered source contracts (or the admin) may call this.
    /// The event is assigned a monotonic event ID and a Lamport timestamp,
    /// persisted in the log, and broadcast via Soroban events for external
    /// indexers.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn publish_event(
        env: Env,
        caller: Address,
        topic: String,
        schema_version: u32,
        payload_hash: String,
    ) -> Result<u64, EventError> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_authorized_publisher(&env, &caller)?;

        if topic.len() == 0 {
            return Err(EventError::InvalidInput);
        }

        registry::require_schema_exists(&env, &topic, schema_version)?;

        let event_id = Self::next_event_id(&env);
        let lamport_ts = Self::tick_lamport(&env);

        let envelope = EventEnvelope {
            event_id,
            topic: topic.clone(),
            schema_version,
            source_contract: caller.clone(),
            payload_hash,
            lamport_ts,
            ledger_ts: env.ledger().timestamp(),
        };

        // Persist the event in the sequential log
        let log_key = (symbol_short!("LOG"), event_id);
        env.storage().persistent().set(&log_key, &envelope);

        // Update the latest event pointer for this topic
        let topic_key = (symbol_short!("T_LATEST"), topic.clone());
        env.storage().persistent().set(&topic_key, &event_id);

        // Append to the global ordered index
        let idx_key = symbol_short!("LOG_IDX");
        let mut index: Vec<u64> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or(Vec::new(&env));
        index.push_back(event_id);
        env.storage().persistent().set(&idx_key, &index);

        // Dispatch to subscriber matching
        subscription::dispatch_to_subscribers(&env, &envelope);

        env.events().publish(
            (symbol_short!("EVT_PUB"), topic, caller),
            envelope.clone(),
        );

        Ok(event_id)
    }

    /// Retrieve an event by its ID.
    pub fn get_event(env: Env, event_id: u64) -> Result<EventEnvelope, EventError> {
        Self::require_initialized(&env)?;
        let log_key = (symbol_short!("LOG"), event_id);
        env.storage()
            .persistent()
            .get(&log_key)
            .ok_or(EventError::EventNotFound)
    }

    /// Return the current Lamport clock value (useful for external ordering).
    pub fn get_lamport_clock(env: Env) -> Result<u64, EventError> {
        Self::require_initialized(&env)?;
        Ok(env.storage().instance().get(&LAMPORT).unwrap_or(0u64))
    }

    /// Return the total number of events published so far.
    pub fn get_event_count(env: Env) -> Result<u64, EventError> {
        Self::require_initialized(&env)?;
        Ok(env.storage().instance().get(&EVT_CTR).unwrap_or(0u64))
    }

    // ── Schema registration (delegated to registry module) ───────────────────

    /// Register a new versioned schema for a topic.
    pub fn register_schema(
        env: Env,
        caller: Address,
        topic: String,
        version: u32,
        schema_hash: String,
    ) -> Result<(), EventError> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;
        registry::register_schema(&env, &topic, version, &schema_hash)
    }

    /// Retrieve the schema hash for a given topic and version.
    pub fn get_schema(
        env: Env,
        topic: String,
        version: u32,
    ) -> Result<String, EventError> {
        Self::require_initialized(&env)?;
        registry::get_schema(&env, &topic, version)
    }

    /// Return the latest schema version registered for a topic.
    pub fn get_latest_schema_version(
        env: Env,
        topic: String,
    ) -> Result<u32, EventError> {
        Self::require_initialized(&env)?;
        registry::get_latest_version(&env, &topic)
    }

    // ── Subscription management (delegated to subscription module) ───────────

    /// Create a topic-based subscription with hierarchical filtering.
    ///
    /// `topic_pattern` supports wildcard matching:
    /// - `records.*` matches `records.vision`, `records.prescription`, etc.
    /// - `records.vision.create` matches exactly that topic.
    pub fn subscribe(
        env: Env,
        subscriber: Address,
        topic_pattern: String,
    ) -> Result<u64, EventError> {
        subscriber.require_auth();
        Self::require_initialized(&env)?;
        subscription::create_subscription(&env, &subscriber, &topic_pattern)
    }

    /// Remove an existing subscription.
    pub fn unsubscribe(
        env: Env,
        subscriber: Address,
        subscription_id: u64,
    ) -> Result<(), EventError> {
        subscriber.require_auth();
        Self::require_initialized(&env)?;
        subscription::remove_subscription(&env, &subscriber, subscription_id)
    }

    /// Return the subscriptions for a given address.
    pub fn get_subscriptions(
        env: Env,
        subscriber: Address,
    ) -> Result<Vec<subscription::Subscription>, EventError> {
        Self::require_initialized(&env)?;
        Ok(subscription::get_subscriptions(&env, &subscriber))
    }

    // ── Consumer groups ──────────────────────────────────────────────────────

    /// Create a consumer group with the specified members.
    /// Events matching the group topic will be distributed across members.
    pub fn create_consumer_group(
        env: Env,
        caller: Address,
        group_name: String,
        topic_pattern: String,
        members: Vec<Address>,
    ) -> Result<u64, EventError> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        subscription::create_consumer_group(&env, &caller, &group_name, &topic_pattern, &members)
    }

    /// Return the consumer group details for a given group ID.
    pub fn get_consumer_group(
        env: Env,
        group_id: u64,
    ) -> Result<subscription::ConsumerGroup, EventError> {
        Self::require_initialized(&env)?;
        subscription::get_consumer_group(&env, group_id)
    }

    /// Acknowledge that a consumer in a group has processed an event.
    pub fn ack_event(
        env: Env,
        consumer: Address,
        group_id: u64,
        event_id: u64,
    ) -> Result<(), EventError> {
        consumer.require_auth();
        Self::require_initialized(&env)?;
        subscription::ack_event(&env, &consumer, group_id, event_id)
    }

    // ── Webhook registration ─────────────────────────────────────────────────

    /// Register a webhook URL for push-based notification on a topic pattern.
    pub fn register_webhook(
        env: Env,
        caller: Address,
        topic_pattern: String,
        url_hash: String,
    ) -> Result<u64, EventError> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        subscription::register_webhook(&env, &caller, &topic_pattern, &url_hash)
    }

    /// Remove a previously registered webhook.
    pub fn remove_webhook(
        env: Env,
        caller: Address,
        webhook_id: u64,
    ) -> Result<(), EventError> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        subscription::remove_webhook(&env, &caller, webhook_id)
    }

    // ── Replay & compaction (delegated to replay module) ─────────────────────

    /// Replay events starting from `from_event_id` up to `limit` entries.
    /// Used by consumers to catch up after downtime.
    pub fn replay_events(
        env: Env,
        from_event_id: u64,
        limit: u32,
    ) -> Result<Vec<EventEnvelope>, EventError> {
        Self::require_initialized(&env)?;
        replay::replay_from(&env, from_event_id, limit)
    }

    /// Replay events for a specific topic starting from a given event ID.
    pub fn replay_topic_events(
        env: Env,
        topic: String,
        from_event_id: u64,
        limit: u32,
    ) -> Result<Vec<EventEnvelope>, EventError> {
        Self::require_initialized(&env)?;
        replay::replay_topic(&env, &topic, from_event_id, limit)
    }

    /// Create a checkpoint at the current event log position.
    /// Returns the checkpoint ID that can be used for future replays.
    pub fn create_checkpoint(
        env: Env,
        caller: Address,
    ) -> Result<u64, EventError> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;
        replay::create_checkpoint(&env)
    }

    /// Retrieve the event ID stored at a given checkpoint.
    pub fn get_checkpoint(
        env: Env,
        checkpoint_id: u64,
    ) -> Result<u64, EventError> {
        Self::require_initialized(&env)?;
        replay::get_checkpoint(&env, checkpoint_id)
    }

    /// Compact events for a topic: merges sequential update events to reduce
    /// replay overhead while preserving the final state.
    pub fn compact_topic(
        env: Env,
        caller: Address,
        topic: String,
    ) -> Result<u32, EventError> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;
        replay::compact_topic(&env, &topic)
    }

    // ── Dead letter queue ────────────────────────────────────────────────────

    /// Push a failed delivery into the dead letter queue for later retry.
    pub fn push_dead_letter(
        env: Env,
        caller: Address,
        event_id: u64,
        subscriber: Address,
        reason: String,
    ) -> Result<(), EventError> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        replay::push_dead_letter(&env, event_id, &subscriber, &reason)
    }

    /// Return all dead letter entries (failed deliveries awaiting retry).
    pub fn get_dead_letters(
        env: Env,
    ) -> Result<Vec<replay::DeadLetterEntry>, EventError> {
        Self::require_initialized(&env)?;
        Ok(replay::get_dead_letters(&env))
    }

    /// Retry a specific dead letter entry by re-dispatching the event.
    pub fn retry_dead_letter(
        env: Env,
        caller: Address,
        dead_letter_index: u32,
    ) -> Result<(), EventError> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;
        replay::retry_dead_letter(&env, dead_letter_index)
    }

    // ── Source contract registration ─────────────────────────────────────────

    /// Register an address as an authorized event publisher.
    pub fn register_source(
        env: Env,
        caller: Address,
        source: Address,
    ) -> Result<(), EventError> {
        caller.require_auth();
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;
        registry::register_source(&env, &source);
        Ok(())
    }

    /// Check whether an address is registered as an event source.
    pub fn is_registered_source(env: Env, source: Address) -> bool {
        registry::is_registered_source(&env, &source)
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    fn require_initialized(env: &Env) -> Result<(), EventError> {
        if !env.storage().instance().has(&INITIALIZED) {
            return Err(EventError::NotInitialized);
        }
        Ok(())
    }

    fn require_admin(env: &Env, caller: &Address) -> Result<(), EventError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(EventError::NotInitialized)?;
        if *caller != admin {
            return Err(EventError::Unauthorized);
        }
        Ok(())
    }

    fn require_authorized_publisher(env: &Env, caller: &Address) -> Result<(), EventError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(EventError::NotInitialized)?;
        if *caller == admin {
            return Ok(());
        }
        if registry::is_registered_source(env, caller) {
            return Ok(());
        }
        Err(EventError::Unauthorized)
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn next_event_id(env: &Env) -> u64 {
        let current: u64 = env.storage().instance().get(&EVT_CTR).unwrap_or(0);
        let next = current + 1;
        env.storage().instance().set(&EVT_CTR, &next);
        next
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn tick_lamport(env: &Env) -> u64 {
        let current: u64 = env.storage().instance().get(&LAMPORT).unwrap_or(0);
        let next = current + 1;
        env.storage().instance().set(&LAMPORT, &next);
        next
    }
}
