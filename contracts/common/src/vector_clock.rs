//! Vector clock implementation for tracking causal ordering of concurrent
//! modifications to medical records.
//!
//! Each record maintains a vector clock where entries map provider addresses
//! (represented as `u32` node identifiers) to monotonically increasing counters.
//! When two clocks are compared, the relationship is one of:
//! - **Equal** – identical histories.
//! - **Before / After** – one causally precedes the other.
//! - **Concurrent** – the modifications happened independently and may conflict.

use soroban_sdk::{contracttype, Env, Map};

/// Maximum number of distinct nodes (providers) tracked per record clock.
/// Keeps storage bounded in a smart-contract environment.
pub const MAX_VECTOR_CLOCK_NODES: u32 = 64;

/// Outcome of comparing two vector clocks.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClockOrdering {
    /// The two clocks are identical.
    Equal,
    /// The left clock causally precedes the right clock.
    Before,
    /// The left clock causally follows the right clock.
    After,
    /// The clocks are concurrent – neither precedes the other.
    Concurrent,
}

/// A vector clock stored on-chain as a map from node ID to counter.
///
/// Node IDs are compact `u32` identifiers assigned externally (e.g. derived
/// from a provider registry counter) to avoid storing full `Address` values
/// inside the clock.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VectorClock {
    pub entries: Map<u32, u64>,
}

impl VectorClock {
    /// Creates an empty vector clock.
    pub fn new(env: &Env) -> Self {
        Self {
            entries: Map::new(env),
        }
    }

    /// Increments the counter for `node_id` by one, returning the new value.
    ///
    /// If the clock already tracks [`MAX_VECTOR_CLOCK_NODES`] distinct nodes
    /// and `node_id` is not among them, the increment is ignored and `0` is
    /// returned to signal the limit was hit.
    pub fn increment(&mut self, env: &Env, node_id: u32) -> u64 {
        let current = self.entries.get(node_id).unwrap_or(0);

        // Enforce node-count cap for new entries.
        if current == 0 && self.entries.len() >= MAX_VECTOR_CLOCK_NODES {
            return 0;
        }

        let next = current.saturating_add(1);
        self.entries.set(node_id, next);
        let _ = env; // kept for future diagnostics
        next
    }

    /// Returns the counter value for a given node, or `0` if absent.
    pub fn get(&self, node_id: u32) -> u64 {
        self.entries.get(node_id).unwrap_or(0)
    }

    /// Merges `other` into `self` by taking the component-wise maximum.
    ///
    /// After merging, `self` dominates both its prior state and `other`.
    pub fn merge(&mut self, other: &VectorClock) {
        for node_id in other.entries.keys() {
            let other_val = other.entries.get(node_id).unwrap_or(0);
            let self_val = self.entries.get(node_id).unwrap_or(0);
            if other_val > self_val {
                self.entries.set(node_id, other_val);
            }
        }
    }

    /// Compares two vector clocks to determine causal ordering.
    pub fn compare(&self, other: &VectorClock) -> ClockOrdering {
        let mut self_has_greater = false;
        let mut other_has_greater = false;

        // Check all entries present in self.
        for node_id in self.entries.keys() {
            let s = self.entries.get(node_id).unwrap_or(0);
            let o = other.entries.get(node_id).unwrap_or(0);
            if s > o {
                self_has_greater = true;
            } else if o > s {
                other_has_greater = true;
            }
            if self_has_greater && other_has_greater {
                return ClockOrdering::Concurrent;
            }
        }

        // Check entries present only in other.
        for node_id in other.entries.keys() {
            if self.entries.get(node_id).is_some() {
                continue; // already compared above
            }
            // self implicitly has 0 for this node
            let o = other.entries.get(node_id).unwrap_or(0);
            if o > 0 {
                other_has_greater = true;
            }
            if self_has_greater && other_has_greater {
                return ClockOrdering::Concurrent;
            }
        }

        match (self_has_greater, other_has_greater) {
            (false, false) => ClockOrdering::Equal,
            (true, false) => ClockOrdering::After,
            (false, true) => ClockOrdering::Before,
            (true, true) => ClockOrdering::Concurrent,
        }
    }

    /// Returns `true` when `self` dominates (is equal to or after) `other`.
    pub fn dominates(&self, other: &VectorClock) -> bool {
        matches!(
            self.compare(other),
            ClockOrdering::Equal | ClockOrdering::After
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_clocks_are_equal() {
        let env = Env::default();
        let a = VectorClock::new(&env);
        let b = VectorClock::new(&env);
        assert_eq!(a.compare(&b), ClockOrdering::Equal);
    }

    #[test]
    fn single_increment_is_after_empty() {
        let env = Env::default();
        let mut a = VectorClock::new(&env);
        a.increment(&env, 1);
        let b = VectorClock::new(&env);
        assert_eq!(a.compare(&b), ClockOrdering::After);
        assert_eq!(b.compare(&a), ClockOrdering::Before);
    }

    #[test]
    fn concurrent_detection() {
        let env = Env::default();
        let mut a = VectorClock::new(&env);
        let mut b = VectorClock::new(&env);
        a.increment(&env, 1);
        b.increment(&env, 2);
        assert_eq!(a.compare(&b), ClockOrdering::Concurrent);
    }

    #[test]
    fn merge_produces_dominating_clock() {
        let env = Env::default();
        let mut a = VectorClock::new(&env);
        let mut b = VectorClock::new(&env);
        a.increment(&env, 1);
        a.increment(&env, 1);
        b.increment(&env, 2);
        b.increment(&env, 2);
        b.increment(&env, 2);

        let mut merged = a.clone();
        merged.merge(&b);
        assert!(merged.dominates(&a));
        assert!(merged.dominates(&b));
        assert_eq!(merged.get(1), 2);
        assert_eq!(merged.get(2), 3);
    }

    #[test]
    fn node_cap_prevents_unbounded_growth() {
        let env = Env::default();
        let mut clock = VectorClock::new(&env);
        for i in 0..MAX_VECTOR_CLOCK_NODES {
            assert!(clock.increment(&env, i) > 0);
        }
        // One more distinct node should be rejected.
        assert_eq!(clock.increment(&env, MAX_VECTOR_CLOCK_NODES), 0);
        // Existing node still works.
        assert_eq!(clock.increment(&env, 0), 2);
    }
}
