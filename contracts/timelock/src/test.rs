extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    symbol_short, Address, Bytes, BytesN, Env, Vec,
};

use crate::{
    LeafData, NestedPayload, PayloadNode, TimelockContract, TimelockContractClient, TimelockError,
    MAX_LEAF_COUNT, MAX_PAYLOAD_DEPTH, MAX_PAYLOAD_WIDTH,
};

// ── Shared helpers ────────────────────────────────────────────────────────────

fn setup(env: &Env) -> (TimelockContractClient, Address) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let contract_id = env.register(TimelockContract, ());
    let client = TimelockContractClient::new(env, &contract_id);
    client.initialize(&admin);
    (client, admin)
}

fn make_id(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

fn make_leaf(env: &Env, seed: u8) -> LeafData {
    LeafData {
        key: symbol_short!("k"),
        value: BytesN::from_array(env, &[seed; 32]),
    }
}

fn empty_payload(env: &Env) -> NestedPayload {
    NestedPayload {
        version: 1,
        nodes: Vec::new(env),
    }
}

/// Build a payload with one node at the given `depth` containing `leaf_count` leaves.
fn single_node_payload(env: &Env, depth: u32, leaf_count: u32) -> NestedPayload {
    let mut leaves: Vec<LeafData> = Vec::new(env);
    for i in 0..leaf_count {
        leaves.push_back(make_leaf(env, (i % 256) as u8));
    }
    let node = PayloadNode {
        depth,
        data: Bytes::new(env),
        leaves,
    };
    let mut nodes: Vec<PayloadNode> = Vec::new(env);
    nodes.push_back(node);
    NestedPayload { version: 1, nodes }
}

// ── Branch 1: Basic Serialization Tests (#570) ───────────────────────────────

/// Queuing with an empty (zero-node) payload serialises and round-trips cleanly.
#[test]
fn test_queue_empty_payload_round_trip() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 1);

    env.ledger().set_timestamp(1_000);
    let payload = empty_payload(&env);
    let entry = client.queue_tx(&caller, &id, &target, &100, &payload);

    assert_eq!(entry.id, id);
    assert_eq!(entry.payload.version, 1);
    assert_eq!(entry.payload.nodes.len(), 0);
    assert_eq!(entry.execute_after, 1_100);
}

/// Queuing with a single-node, single-leaf payload preserves all fields exactly.
#[test]
fn test_queue_single_node_payload_fields_preserved() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 2);

    env.ledger().set_timestamp(500);
    let payload = single_node_payload(&env, 3, 1);
    let entry = client.queue_tx(&caller, &id, &target, &60, &payload);

    assert_eq!(entry.payload.nodes.len(), 1);
    let node = entry.payload.nodes.get(0).unwrap();
    assert_eq!(node.depth, 3);
    assert_eq!(node.leaves.len(), 1);
}

/// `get_tx` returns the exact same entry that was queued — payload included.
#[test]
fn test_get_tx_matches_queued_entry() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 3);

    env.ledger().set_timestamp(0);
    let payload = single_node_payload(&env, 1, 2);
    let queued = client.queue_tx(&caller, &id, &target, &300, &payload);
    let fetched = client.get_tx(&id);

    assert_eq!(queued, fetched);
    assert_eq!(fetched.payload.nodes.len(), 1);
    assert_eq!(fetched.payload.nodes.get(0).unwrap().leaves.len(), 2);
}

/// Multiple transactions with distinct IDs serialise independently — each
/// payload's depth and leaf count is retrieved correctly.
#[test]
fn test_multiple_txs_serialise_independently() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);

    env.ledger().set_timestamp(1_000);
    for i in 1u8..=5 {
        let id = make_id(&env, i);
        let payload = single_node_payload(&env, i as u32, i as u32);
        client.queue_tx(&caller, &id, &target, &100, &payload);
    }

    let queue = client.get_queue();
    assert_eq!(queue.len(), 5);

    for i in 1u8..=5 {
        let id = make_id(&env, i);
        let entry = client.get_tx(&id);
        let node = entry.payload.nodes.get(0).unwrap();
        assert_eq!(node.depth, i as u32);
        assert_eq!(node.leaves.len(), i as u32);
    }
}

/// A payload with multiple nodes serialises and all node depths are preserved.
#[test]
fn test_multi_node_payload_all_depths_preserved() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 6);

    env.ledger().set_timestamp(0);
    let mut nodes: Vec<PayloadNode> = Vec::new(&env);
    for i in 0..4u32 {
        let mut leaves: Vec<LeafData> = Vec::new(&env);
        leaves.push_back(make_leaf(&env, i as u8));
        nodes.push_back(PayloadNode {
            depth: i + 1,
            data: Bytes::new(&env),
            leaves,
        });
    }
    let payload = NestedPayload { version: 2, nodes };
    let entry = client.queue_tx(&caller, &id, &target, &100, &payload);

    assert_eq!(entry.payload.version, 2);
    assert_eq!(entry.payload.nodes.len(), 4);
    for i in 0..4u32 {
        assert_eq!(entry.payload.nodes.get(i).unwrap().depth, i + 1);
    }
}

/// Queuing the same transaction ID twice is rejected with `TxAlreadyQueued`.
#[test]
fn test_duplicate_id_rejected() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 10);

    env.ledger().set_timestamp(0);
    client.queue_tx(&caller, &id, &target, &100, &empty_payload(&env));

    let result = client.try_queue_tx(&caller, &id, &target, &100, &empty_payload(&env));
    assert_eq!(result, Err(Ok(TimelockError::TxAlreadyQueued)));
}

/// A zero delay is rejected with `InvalidDelay` before any storage write occurs.
#[test]
fn test_zero_delay_rejected() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 11);

    let result = client.try_queue_tx(&caller, &id, &target, &0, &empty_payload(&env));
    assert_eq!(result, Err(Ok(TimelockError::InvalidDelay)));
}

/// After the delay elapses, `execute_tx` succeeds and removes the entry.
#[test]
fn test_execute_after_delay_removes_entry() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 12);

    env.ledger().set_timestamp(1_000);
    let payload = single_node_payload(&env, 2, 3);
    client.queue_tx(&caller, &id, &target, &500, &payload);

    env.ledger().set_timestamp(1_501);
    let executed = client.execute_tx(&caller, &id);
    assert_eq!(executed.id, id);
    assert_eq!(executed.payload.nodes.get(0).unwrap().leaves.len(), 3);

    // Entry must no longer be in the queue
    let result = client.try_get_tx(&id);
    assert_eq!(result, Err(Ok(TimelockError::TxNotFound)));
}

/// Executing before the delay elapses is rejected with `TooEarlyToExecute`.
#[test]
fn test_execute_before_delay_rejected() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 13);

    env.ledger().set_timestamp(1_000);
    client.queue_tx(&caller, &id, &target, &500, &empty_payload(&env));

    env.ledger().set_timestamp(1_100);
    let result = client.try_execute_tx(&caller, &id);
    assert_eq!(result, Err(Ok(TimelockError::TooEarlyToExecute)));
}

/// Prioritised transactions can be executed immediately before the delay elapses.
#[test]
fn test_prioritized_tx_executes_early() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 14);

    env.ledger().set_timestamp(1_000);
    client.queue_tx(&caller, &id, &target, &9999, &empty_payload(&env));
    client.prioritize_tx(&admin, &id);

    env.ledger().set_timestamp(1_001);
    let executed = client.execute_tx(&caller, &id);
    assert!(executed.priority);
}

/// Non-admin callers cannot prioritise transactions.
#[test]
fn test_non_admin_cannot_prioritize() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 15);

    env.ledger().set_timestamp(0);
    client.queue_tx(&caller, &id, &target, &100, &empty_payload(&env));

    let imposter = Address::generate(&env);
    let result = client.try_prioritize_tx(&imposter, &id);
    assert_eq!(result, Err(Ok(TimelockError::Unauthorized)));
}

/// Executing a non-existent transaction ID returns `TxNotFound`.
#[test]
fn test_execute_unknown_id_returns_not_found() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let id = make_id(&env, 99);

    let result = client.try_execute_tx(&caller, &id);
    assert_eq!(result, Err(Ok(TimelockError::TxNotFound)));
}

/// `get_queue` on an empty queue returns an empty vector without panic.
#[test]
fn test_get_queue_empty_returns_empty_vec() {
    let env = Env::default();
    let (client, _) = setup(&env);
    assert_eq!(client.get_queue().len(), 0);
}

/// Double-initialisation is rejected with `AlreadyInitialized`.
#[test]
fn test_double_initialize_rejected() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let result = client.try_initialize(&admin);
    assert_eq!(result, Err(Ok(TimelockError::AlreadyInitialized)));
}

// ── Branch 2: Stack-Overflow / Depth-Limit Guard Tests (#570) ────────────────
//
// These tests verify that the contract never panics or causes runaway
// recursion when presented with maximally or illegally deep nested payloads.
// Each limit is probed at exactly the boundary (accepted) and one beyond
// (rejected), plus the u32::MAX sentinel case.

/// A node at exactly `MAX_PAYLOAD_DEPTH` is accepted — sits at the boundary.
#[test]
fn test_payload_node_at_max_depth_accepted() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 20);

    env.ledger().set_timestamp(0);
    let payload = single_node_payload(&env, MAX_PAYLOAD_DEPTH, 1);
    let entry = client.queue_tx(&caller, &id, &target, &100, &payload);
    assert_eq!(entry.payload.nodes.get(0).unwrap().depth, MAX_PAYLOAD_DEPTH);
}

/// A node with depth `MAX_PAYLOAD_DEPTH + 1` is rejected with `PayloadTooDeep`
/// before any ledger write occurs.
#[test]
fn test_payload_node_exceeds_max_depth_rejected() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 21);

    env.ledger().set_timestamp(0);
    let payload = single_node_payload(&env, MAX_PAYLOAD_DEPTH + 1, 1);
    let result = client.try_queue_tx(&caller, &id, &target, &100, &payload);
    assert_eq!(result, Err(Ok(TimelockError::PayloadTooDeep)));
}

/// A node with `depth = u32::MAX` is rejected without triggering a panic or
/// integer overflow in the depth-comparison logic.
#[test]
fn test_payload_node_at_u32_max_depth_rejected_without_panic() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 22);

    env.ledger().set_timestamp(0);
    let payload = single_node_payload(&env, u32::MAX, 1);
    let result = client.try_queue_tx(&caller, &id, &target, &100, &payload);
    assert_eq!(result, Err(Ok(TimelockError::PayloadTooDeep)));
}

/// A node with exactly `MAX_LEAF_COUNT` leaves is accepted.
#[test]
fn test_payload_node_at_max_leaf_count_accepted() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 23);

    env.ledger().set_timestamp(0);
    let payload = single_node_payload(&env, 1, MAX_LEAF_COUNT);
    let entry = client.queue_tx(&caller, &id, &target, &100, &payload);
    assert_eq!(
        entry.payload.nodes.get(0).unwrap().leaves.len(),
        MAX_LEAF_COUNT
    );
}

/// A node with `MAX_LEAF_COUNT + 1` leaves is rejected with `PayloadTooWide`.
#[test]
fn test_payload_node_exceeds_max_leaf_count_rejected() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 24);

    env.ledger().set_timestamp(0);
    let payload = single_node_payload(&env, 1, MAX_LEAF_COUNT + 1);
    let result = client.try_queue_tx(&caller, &id, &target, &100, &payload);
    assert_eq!(result, Err(Ok(TimelockError::PayloadTooWide)));
}

/// A payload with exactly `MAX_PAYLOAD_WIDTH` nodes is accepted.
#[test]
fn test_payload_at_max_width_accepted() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 25);

    env.ledger().set_timestamp(0);
    let mut nodes: Vec<PayloadNode> = Vec::new(&env);
    for i in 0..MAX_PAYLOAD_WIDTH {
        nodes.push_back(PayloadNode {
            depth: 1,
            data: Bytes::new(&env),
            leaves: {
                let mut l: Vec<LeafData> = Vec::new(&env);
                l.push_back(make_leaf(&env, i as u8));
                l
            },
        });
    }
    let payload = NestedPayload { version: 1, nodes };
    let entry = client.queue_tx(&caller, &id, &target, &100, &payload);
    assert_eq!(entry.payload.nodes.len(), MAX_PAYLOAD_WIDTH);
}

/// A payload with `MAX_PAYLOAD_WIDTH + 1` nodes is rejected with `PayloadTooWide`.
#[test]
fn test_payload_exceeds_max_width_rejected() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 26);

    env.ledger().set_timestamp(0);
    let mut nodes: Vec<PayloadNode> = Vec::new(&env);
    for i in 0..(MAX_PAYLOAD_WIDTH + 1) {
        nodes.push_back(PayloadNode {
            depth: 1,
            data: Bytes::new(&env),
            leaves: {
                let mut l: Vec<LeafData> = Vec::new(&env);
                l.push_back(make_leaf(&env, i as u8));
                l
            },
        });
    }
    let payload = NestedPayload { version: 1, nodes };
    let result = client.try_queue_tx(&caller, &id, &target, &100, &payload);
    assert_eq!(result, Err(Ok(TimelockError::PayloadTooWide)));
}

/// A single node at `MAX_PAYLOAD_DEPTH` with `MAX_LEAF_COUNT` leaves is
/// accepted — worst-case combination that stays within both limits.
#[test]
fn test_max_depth_and_max_leaf_count_simultaneously_accepted() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 27);

    env.ledger().set_timestamp(0);
    let payload = single_node_payload(&env, MAX_PAYLOAD_DEPTH, MAX_LEAF_COUNT);
    let entry = client.queue_tx(&caller, &id, &target, &100, &payload);
    let node = entry.payload.nodes.get(0).unwrap();
    assert_eq!(node.depth, MAX_PAYLOAD_DEPTH);
    assert_eq!(node.leaves.len(), MAX_LEAF_COUNT);
}

/// In a mixed payload where one node is valid and one exceeds the depth
/// limit, the entire queue_tx call is rejected.
#[test]
fn test_mixed_payload_one_over_depth_node_rejects_whole_tx() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 28);

    env.ledger().set_timestamp(0);
    let mut nodes: Vec<PayloadNode> = Vec::new(&env);
    // First node is valid
    nodes.push_back(PayloadNode {
        depth: 2,
        data: Bytes::new(&env),
        leaves: Vec::new(&env),
    });
    // Second node violates the depth limit
    nodes.push_back(PayloadNode {
        depth: MAX_PAYLOAD_DEPTH + 5,
        data: Bytes::new(&env),
        leaves: Vec::new(&env),
    });
    let payload = NestedPayload { version: 1, nodes };
    let result = client.try_queue_tx(&caller, &id, &target, &100, &payload);
    assert_eq!(result, Err(Ok(TimelockError::PayloadTooDeep)));
}

/// A depth-zero node (root level) combined with any valid leaf count is accepted.
#[test]
fn test_depth_zero_root_node_accepted() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let id = make_id(&env, 29);

    env.ledger().set_timestamp(0);
    let payload = single_node_payload(&env, 0, 5);
    let entry = client.queue_tx(&caller, &id, &target, &50, &payload);
    assert_eq!(entry.payload.nodes.get(0).unwrap().depth, 0);
}

/// Nodes with depth exactly `MAX_PAYLOAD_DEPTH - 1` through `MAX_PAYLOAD_DEPTH`
/// are all accepted without panic (boundary sweep).
#[test]
fn test_depth_boundary_sweep_all_accepted() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);

    env.ledger().set_timestamp(0);
    for depth in (MAX_PAYLOAD_DEPTH - 1)..=MAX_PAYLOAD_DEPTH {
        let id = make_id(&env, (50 + depth) as u8);
        let payload = single_node_payload(&env, depth, 1);
        let entry = client.queue_tx(&caller, &id, &target, &100, &payload);
        assert_eq!(entry.payload.nodes.get(0).unwrap().depth, depth);
    }
}

/// Depths `MAX_PAYLOAD_DEPTH + 1` through `MAX_PAYLOAD_DEPTH + 3` are all
/// rejected (boundary-plus sweep) to confirm the guard has no off-by-one flaw.
#[test]
fn test_depth_boundary_plus_sweep_all_rejected() {
    let env = Env::default();
    let (client, _) = setup(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);

    env.ledger().set_timestamp(0);
    for depth in (MAX_PAYLOAD_DEPTH + 1)..=(MAX_PAYLOAD_DEPTH + 3) {
        let id = make_id(&env, (60 + depth) as u8);
        let payload = single_node_payload(&env, depth, 1);
        let result = client.try_queue_tx(&caller, &id, &target, &100, &payload);
        assert_eq!(
            result,
            Err(Ok(TimelockError::PayloadTooDeep)),
            "depth {} should be rejected",
            depth
        );
    }
}
