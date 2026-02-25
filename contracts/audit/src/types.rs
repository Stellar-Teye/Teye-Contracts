/// Core domain types for the distributed tamper-evident audit log.
///
/// Design principles:
/// - Every public type derives only what is strictly necessary.
/// - Byte buffers use fixed-size arrays where the size is known at compile time
///   (e.g. a SHA-256 digest is always 32 bytes) to eliminate heap allocation.
/// - `LogEntry` is intentionally immutable after construction; callers receive
///   an owned value and should never need to mutate it.

// ─── Re-exports so consumers only need `audit::types::*` ────────────────────
pub use crate::merkle_log::MerkleRoot;

// ── Digest ────────────────────────────────────────────────────────────────────

/// A SHA-256 digest (32 bytes).  Used for `prev_hash`, `entry_hash`, and
/// Merkle-tree node hashes throughout the crate.
pub type Digest = [u8; 32];

// ── Segment identifier ────────────────────────────────────────────────────────

/// Identifies a logical partition of the audit log.
///
/// Logs can be segmented by contract address, tenant, or sensitivity level.
/// A segment is represented by an ASCII label of at most 64 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LogSegmentId(pub(crate) [u8; 64], pub(crate) usize /* used bytes */);

impl LogSegmentId {
    /// Create a new segment identifier.
    ///
    /// # Errors
    /// Returns [`AuditError::InvalidSegmentId`] when `label` exceeds 64 bytes.
    pub fn new(label: &str) -> Result<Self, AuditError> {
        let bytes = label.as_bytes();
        if bytes.len() > 64 {
            return Err(AuditError::InvalidSegmentId);
        }
        let mut buf = [0u8; 64];
        buf[..bytes.len()].copy_from_slice(bytes);
        Ok(Self(buf, bytes.len()))
    }

    /// View the label as a byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0[..self.1]
    }

    /// View the label as a string slice.
    ///
    /// # Panics (won't): segments are always created from valid `&str` above.
    #[inline]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or("")
    }
}

impl core::fmt::Display for LogSegmentId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Log entry ─────────────────────────────────────────────────────────────────

/// A single immutable audit-log record.
///
/// ### Hash chain
/// `prev_hash` is the SHA-256 digest of the *previous* `LogEntry`'s canonical
/// byte representation.  For the first entry in a segment `prev_hash` is
/// `[0u8; 32]`.  This forms a singly-linked hash chain; modifying any earlier
/// entry breaks every subsequent `prev_hash`.
///
/// ### Complexity
/// Construction is O(1).  Serialisation for hashing is O(|actor| + |action| +
/// |target| + |result|) ≈ O(L) where L is total field length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// Monotonically increasing sequence number within its segment.  
    /// Enables O(1) range queries when entries are stored in a sorted map.
    pub sequence: u64,

    /// Unix timestamp (seconds since epoch) supplied by the caller.
    pub timestamp: u64,

    /// Identity of the initiating party (e.g. address, user-id).
    /// Kept as a heap-allocated string to avoid bounding actor length.
    pub actor: alloc::string::String,

    /// High-level action label (e.g. `"record.create"`, `"access.grant"`).
    pub action: alloc::string::String,

    /// Resource that was acted upon.
    pub target: alloc::string::String,

    /// Outcome of the action (`"ok"`, `"denied"`, etc.).
    pub result: alloc::string::String,

    /// Digest of the *previous* entry in the same segment.
    pub prev_hash: Digest,

    /// Digest of *this* entry (computed at construction time).
    pub entry_hash: Digest,

    /// Logical segment this entry belongs to.
    pub segment: LogSegmentId,
}

impl LogEntry {
    /// Serialise the entry into a canonical byte representation used for
    /// hashing.  The format is:
    /// ```text
    /// sequence(8 LE) ‖ timestamp(8 LE) ‖ actor ‖ '\0' ‖ action ‖ '\0'
    ///  ‖ target ‖ '\0' ‖ result ‖ '\0' ‖ prev_hash(32) ‖ segment_label
    /// ```
    /// Null bytes act as field separators; no field may contain a null byte.
    pub fn canonical_bytes(&self) -> alloc::vec::Vec<u8> {
        let mut buf = alloc::vec::Vec::with_capacity(
            8 + 8
                + self.actor.len()
                + 1
                + self.action.len()
                + 1
                + self.target.len()
                + 1
                + self.result.len()
                + 1
                + 32
                + self.segment.as_bytes().len(),
        );
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        buf.extend_from_slice(self.actor.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.action.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.target.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.result.as_bytes());
        buf.push(0);
        buf.extend_from_slice(&self.prev_hash);
        buf.extend_from_slice(self.segment.as_bytes());
        buf
    }
}

// ── Witness / co-signing ──────────────────────────────────────────────────────

/// A witness co-signature attesting to a Merkle root at a specific log size.
///
/// Multiple witnesses endorsing the same `(root, tree_size)` pair provides
/// Byzantine fault tolerance: an attacker must compromise a threshold of
/// witnesses to forge a consistent history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessSignature {
    /// Human-readable identifier for the signing party.
    pub witness_id: alloc::string::String,

    /// The Merkle root being endorsed.
    pub root: MerkleRoot,

    /// Number of log entries covered by this root.
    pub tree_size: u64,

    /// Timestamp at which the signature was produced.
    pub signed_at: u64,

    /// Raw signature bytes (format is application-defined; stored opaquely).
    pub signature: alloc::vec::Vec<u8>,
}

// ── Retention policy ──────────────────────────────────────────────────────────

/// Governs how long entries in a segment must be retained.
///
/// ### Verifiable deletion
/// When entries are purged, the `MerkleLog` computes a *compaction receipt*
/// (a Merkle proof over the deleted range) so that auditors can verify which
/// entries were removed and confirm the remaining log's integrity has been
/// preserved.
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    /// Segment this policy applies to.
    pub segment: LogSegmentId,

    /// Minimum number of seconds an entry must be retained before deletion.
    pub min_retention_secs: u64,

    /// If `true`, the segment may hold entries of higher regulatory sensitivity
    /// and requires additional witness co-signing before compaction.
    pub requires_witness_for_deletion: bool,
}

// ── Error types ───────────────────────────────────────────────────────────────

/// All error conditions produced by the audit-log subsystem.
///
/// Uses a closed enum so that callers can exhaustively match and the compiler
/// enforces handling of every variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditError {
    /// The hash chain is broken: `prev_hash` of the verified entry does not
    /// equal the digest of its predecessor.
    HashChainBroken {
        /// Sequence number of the offending entry.
        at_sequence: u64,
    },

    /// A Merkle inclusion proof is invalid.
    InvalidInclusionProof,

    /// A Merkle consistency proof is invalid.
    InvalidConsistencyProof,

    /// An attempt was made to search with an invalid or malformed token.
    InvalidSearchToken,

    /// The requested log entry does not exist.
    EntryNotFound { sequence: u64 },

    /// The segment label is too long or otherwise invalid.
    InvalidSegmentId,

    /// Compaction was requested but insufficient witnesses have co-signed.
    InsufficientWitnesses {
        required: usize,
        present: usize,
    },

    /// Retention policy prevents deletion of this entry.
    RetentionPolicyViolation {
        sequence: u64,
        retained_until: u64,
    },

    /// The provided root does not match the computed root for the given size.
    RootMismatch,

    /// An internal invariant was violated — indicates a bug.
    InternalError(&'static str),
}

impl core::fmt::Display for AuditError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AuditError::HashChainBroken { at_sequence } => {
                write!(f, "hash chain broken at sequence {at_sequence}")
            }
            AuditError::InvalidInclusionProof => write!(f, "invalid Merkle inclusion proof"),
            AuditError::InvalidConsistencyProof => write!(f, "invalid Merkle consistency proof"),
            AuditError::InvalidSearchToken => write!(f, "invalid searchable-encryption token"),
            AuditError::EntryNotFound { sequence } => {
                write!(f, "entry at sequence {sequence} not found")
            }
            AuditError::InvalidSegmentId => {
                write!(f, "segment id must be non-empty and ≤64 bytes")
            }
            AuditError::InsufficientWitnesses { required, present } => {
                write!(
                    f,
                    "insufficient witnesses: {present} present, {required} required"
                )
            }
            AuditError::RetentionPolicyViolation {
                sequence,
                retained_until,
            } => write!(
                f,
                "entry {sequence} must be retained until timestamp {retained_until}"
            ),
            AuditError::RootMismatch => write!(f, "Merkle root mismatch"),
            AuditError::InternalError(msg) => write!(f, "internal error: {msg}"),
        }
    }
}
