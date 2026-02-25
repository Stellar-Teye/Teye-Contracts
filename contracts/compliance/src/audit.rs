//! Compliance-aware audit logging.
//!
//! This module is a thin façade over the `audit` crate that pre-configures
//! the Merkle log and searchable-encryption index for the compliance domain.
//!
//! ## Why a façade?
//!
//! The lower-level `audit` crate is deliberately domain-agnostic.  Here we:
//!
//! * Fix the segment to `"compliance"`.
//! * Wire the `SearchEngine` so that every append also updates the keyword
//!   index automatically.
//! * Expose a simplified `record` / `query` / `search` surface that matches
//!   the ergonomics expected by compliance-layer callers.
//! * Retain the old `AuditEntry` type (re-exported from `audit::types`) for
//!   backward compatibility with any existing callers.
//!
//! ## Tamper evidence
//!
//! Internally every `record` call:
//!
//! 1. Appends a [`LogEntry`] to a [`MerkleLog`] (hash-chain + leaf hash).
//! 2. Indexes the entry's actor/action/target/result keywords into a
//!    [`SearchEngine`] so keyword search does not require plaintext storage.
//!
//! A Merkle root can be published at any time by calling
//! [`ComplianceAuditLog::publish_root`]; the returned [`MerkleRoot`] can be
//! anchored off-chain (e.g. written to a ledger event or external CT log).
//!
//! [`LogEntry`]: audit::types::LogEntry
//! [`MerkleLog`]: audit::merkle_log::MerkleLog
//! [`SearchEngine`]: audit::search::SearchEngine

// Re-export key types so callers don't need to import the `audit` crate
// directly.
pub use audit::{
    merkle_log::{InclusionProof, MerkleLog, MerkleRoot, RootCheckpoint},
    search::{SearchEngine, SearchKey},
    types::{AuditError, LogEntry, LogSegmentId},
};

// ── ComplianceAuditLog ────────────────────────────────────────────────────────

/// A compliance-domain audit log combining tamper-evident hash-chain storage
/// with searchable-encryption-based keyword search.
///
/// ### Thread safety
///
/// `ComplianceAuditLog` is `Send` but **not** `Sync` — wrap in a `Mutex` or
/// `RwLock` when shared across threads.
///
/// ### Complexity summary
///
/// | Operation          | Time          | Notes                              |
/// |--------------------|---------------|------------------------------------|
/// | `record`           | O(log n + k)  | n = entries, k = keywords          |
/// | `query_range`      | O(k + log n)  | k = range width                    |
/// | `search`           | O(log I + m)  | I = distinct tokens, m = matches   |
/// | `inclusion_proof`  | O(log n)      |                                    |
/// | `verify_chain`     | O(k · L)      | L = avg. entry byte length         |
/// | `publish_root`     | O(n)          | Triggers full root recomputation   |
pub struct ComplianceAuditLog {
    /// The underlying Merkle log for the compliance segment.
    log: MerkleLog,
    /// HMAC-SHA256-based forward index for keyword search.
    search: SearchEngine,
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub actor: String,
    pub action: String,
    pub target: String,
    pub timestamp: u64,
}

impl ComplianceAuditLog {
    /// The canonical segment label used for all compliance entries.
    pub const SEGMENT: &'static str = "compliance";

    /// Create a new compliance audit log with the given search key.
    ///
    /// # Parameters
    /// * `search_key` – 32-byte symmetric key for searchable encryption.
    ///   Use a cryptographically random value in production.
    pub fn new(search_key: SearchKey) -> Self {
        let seg = LogSegmentId::new(Self::SEGMENT)
            .expect("segment label is valid ASCII ≤ 64 bytes");
        Self {
            log: MerkleLog::new(seg),
            search: SearchEngine::new(search_key),
        }
    }

    // ── Write ─────────────────────────────────────────────────────────────

    /// Record an audit event.
    ///
    /// * `timestamp` – Unix seconds (caller-supplied; use ledger time or
    ///   `std::time::SystemTime` as appropriate).
    /// * `actor`     – Initiating principal (address, user-id, …).
    /// * `action`    – Action label, conventionally `"noun.verb"`.
    /// * `target`    – Affected resource identifier.
    /// * `result`    – Outcome string (`"ok"`, `"denied"`, …).
    ///
    /// Returns the newly assigned sequence number.
    pub fn record(
        &mut self,
        timestamp: u64,
        actor: &str,
        action: &str,
        target: &str,
        result: &str,
    ) -> u64 {
        let seq = self.log.append(timestamp, actor, action, target, result);
        self.search.index_entry(seq, actor, action, target, result, &[]);
        seq
    }

    /// Record an audit event with additional domain-specific keywords that
    /// should be searchable (e.g. `"dataset:EU"`, `"sensitivity:high"`).
    pub fn record_with_keywords(
        &mut self,
        timestamp: u64,
        actor: &str,
        action: &str,
        target: &str,
        result: &str,
        extra_keywords: &[&str],
    ) -> u64 {
        let seq = self.log.append(timestamp, actor, action, target, result);
        self.search
            .index_entry(seq, actor, action, target, result, extra_keywords);
        seq
    }

    // ── Read ──────────────────────────────────────────────────────────────

    /// Retrieve a single entry by sequence number.
    pub fn get_entry(&self, sequence: u64) -> Result<&LogEntry, AuditError> {
        self.log.get_entry(sequence)
    }

    /// Retrieve all entries in the inclusive sequence range `[from, to]`.
    pub fn query_range(&self, from: u64, to: u64) -> Vec<&LogEntry> {
        self.log.query_range(from, to)
    }

    /// Total number of live entries.
    pub fn len(&self) -> u64 {
        self.log.len()
    }

    /// True when the log contains no entries.
    pub fn is_empty(&self) -> bool {
        self.log.is_empty()
    }

    // ── Integrity ─────────────────────────────────────────────────────────

    /// Compute the current Merkle root without publishing a checkpoint.
    pub fn current_root(&self) -> MerkleRoot {
        self.log.current_root()
    }

    /// Publish the current root as a named checkpoint.  Returns the root.
    ///
    /// Call this periodically (e.g. every N entries or on a time schedule)
    /// to create anchoring points for consistency proofs.
    pub fn publish_root(&mut self, published_at: u64) -> MerkleRoot {
        self.log.publish_root(published_at)
    }

    /// All published root checkpoints.
    pub fn checkpoints(&self) -> &[RootCheckpoint] {
        self.log.checkpoints()
    }

    /// Generate a Merkle inclusion proof for `sequence`.
    pub fn inclusion_proof(&self, sequence: u64) -> Result<InclusionProof, AuditError> {
        self.log.inclusion_proof(sequence)
    }

    /// Verify the hash chain for entries in `[from_seq, to_seq]`.
    pub fn verify_chain(&self, from_seq: u64, to_seq: u64) -> Result<(), AuditError> {
        self.log.verify_chain(from_seq, to_seq)
    }

    // ── Search ────────────────────────────────────────────────────────────

    /// Return the sequence numbers of all entries that match `keyword`.
    ///
    /// The keyword is hashed to a token before the lookup so the engine
    /// never stores or compares plaintext keywords.
    pub fn search(&self, keyword: &str) -> Vec<u64> {
        self.search.query(keyword)
    }
}

// ── Backward-compatibility alias ──────────────────────────────────────────────

/// A single audit log record.
///
/// This is an alias for the `LogEntry` from the `audit` crate, provided for
/// backward compatibility with code that previously used
/// `compliance::AuditEntry`.
pub type AuditEntry = LogEntry;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_log() -> ComplianceAuditLog {
        let key = SearchKey::from_bytes(&[0x77u8; 32]).unwrap();
        ComplianceAuditLog::new(key)
    }

    #[test]
    fn record_and_retrieve() {
        let mut log = make_log();
        let seq = log.record(1_000, "alice", "record.create", "patient:1", "ok");
        assert_eq!(seq, 1);
        let entry = log.get_entry(1).unwrap();
        assert_eq!(entry.actor, "alice");
        assert_eq!(entry.action, "record.create");
        assert_eq!(entry.sequence, 1);
        assert_eq!(entry.timestamp, 1_000);
    }

    #[test]
    fn hash_chain_is_sound() {
        let mut log = make_log();
        for i in 0..5u64 {
            log.record(i * 100, "sys", "boot", "node", "ok");
        }
        assert!(log.verify_chain(1, 5).is_ok());
    }

    #[test]
    fn inclusion_proof_verifies() {
        let mut log = make_log();
        for i in 0..4u64 {
            log.record(i, "u", "a", "t", "ok");
        }
        let root = log.current_root();
        for seq in 1..=4u64 {
            let proof = log.inclusion_proof(seq).unwrap();
            assert!(proof.verify(&root).is_ok());
        }
    }

    #[test]
    fn keyword_search_finds_entries() {
        let mut log = make_log();
        log.record(1, "alice", "record.read", "patient:42", "ok");
        log.record(2, "bob", "record.write", "patient:42", "ok");
        log.record(3, "alice", "access.grant", "patient:42", "ok");

        assert_eq!(log.search("alice"), vec![1, 3]);
        assert_eq!(log.search("bob"), vec![2]);
        assert_eq!(log.search("record"), vec![1, 2]); // namespace prefix
    }

    #[test]
    fn publish_root_records_checkpoint() {
        let mut log = make_log();
        log.record(1, "a", "b", "c", "ok");
        let root = log.publish_root(9_999);
        assert_eq!(log.checkpoints().len(), 1);
        assert_eq!(log.checkpoints()[0].root, root);
    }

    #[test]
    fn record_with_extra_keywords() {
        let mut log = make_log();
        log.record_with_keywords(
            500,
            "sys",
            "export",
            "dataset:EU",
            "ok",
            &["datacenter:EU", "sensitivity:high"],
        );
        assert_eq!(log.search("datacenter:EU"), vec![1]);
        assert_eq!(log.search("sensitivity:high"), vec![1]);

impl AuditLog {
    /// Records an audit entry. For key rotation, use action="rotate_master_secure" and target="master_key".
    pub fn record(&mut self, actor: &str, action: &str, target: &str, now: u64) {
        self.entries.push(AuditEntry {
            actor: actor.to_string(),
            action: action.to_string(),
            target: target.to_string(),
            timestamp: now,
        });
    }

    pub fn query(&self) -> &[AuditEntry] {
        &self.entries
    }
}
