/// Searchable encryption for audit log entries.
///
/// # Design
///
/// Full searchable symmetric encryption (SSE) requires a trusted key-server or
/// a structured-encryption scheme.  In an append-only audit log the required
/// query model is:
///
/// > "Return all entries in segment S whose keyword set contains the token T."
///
/// We implement **SSE-1** (the simplest deterministic SSE scheme):
///
/// * A symmetric key `K` is agreed out-of-band and held only by authorised
///   parties.
/// * For every keyword `w` extracted from an entry, the client stores a
///   *search token* `T(w) = HMAC-SHA256(K, w)` alongside the entry's sequence
///   number in a forward index.
/// * A searcher holding `K` derives `T(query)` and looks it up in the index in
///   O(m) where m is the number of matching entries.
///
/// ### Privacy properties
///
/// * An index observer without `K` learns only *which entries share keywords*
///   (access pattern), not the keyword values.  This is the standard access-
///   pattern leakage accepted by SSE-1.
/// * Forward security (hiding future insertions from past tokens) can be added
///   by periodically re-keying; this is left to the application layer.
///
/// ### NOT provided
///
/// * Post-quantum security.
/// * Dynamic deletion of individual index entries (compaction of the underlying
///   log invalidates entries but the index must be rebuilt separately).
///
/// # Complexity
///
/// | Operation        | Time     | Space  |
/// |------------------|----------|--------|
/// | `index_entry`    | O(k)     | O(k)   |
/// | `search`         | O(m)     | O(m)   |
/// | `gen_token`      | O(1)     | O(1)   |
///
/// where k = number of keywords per entry, m = number of matching entries.

use alloc::{collections::BTreeMap, vec::Vec};

use sha2::Sha256;
use hmac::{Hmac, Mac};

use crate::types::AuditError;

// ── Type aliases ──────────────────────────────────────────────────────────────

/// A 32-byte HMAC-SHA256 search token derived from a keyword + key.
pub type SearchToken = [u8; 32];

// ── SearchKey ─────────────────────────────────────────────────────────────────

/// A symmetric key used to derive search tokens.
///
/// The key must be exactly 32 bytes; using a cryptographically random value is
/// strongly recommended.  For test use `SearchKey::test_key()`.
#[derive(Clone)]
pub struct SearchKey([u8; 32]);

impl SearchKey {
    /// Construct from a raw 32-byte slice.
    ///
    /// # Errors
    /// Returns [`AuditError::InvalidSearchToken`] if `raw` is not 32 bytes.
    pub fn from_bytes(raw: &[u8]) -> Result<Self, AuditError> {
        raw.try_into()
            .map(Self)
            .map_err(|_| AuditError::InvalidSearchToken)
    }

    /// A deterministic, fixed key for unit tests.  **Never use in production.**
    #[cfg(test)]
    pub fn test_key() -> Self {
        Self([0x42u8; 32])
    }

    /// Derive the search token for `keyword`.
    ///
    /// `token = HMAC-SHA256(self.key, keyword.as_bytes())`
    ///
    /// Complexity: O(|keyword|) — single HMAC call.
    pub fn token_for(&self, keyword: &str) -> SearchToken {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.0)
            .expect("HMAC accepts any key length ≥ 0");
        mac.update(keyword.as_bytes());
        mac.finalize().into_bytes().into()
    }
}

// ── ForwardIndex ─────────────────────────────────────────────────────────────

/// A forward (keyword → sequence-numbers) searchable index for a single segment.
///
/// Entries are keyed by their search token so that parties without the search
/// key cannot recover keyword values from the index alone.
///
/// ### Storage layout
///
/// ```text
/// BTreeMap<SearchToken, Vec<u64>>
///          ^^ 32 bytes            ^^ list of matching sequence numbers
/// ```
///
/// Using a `BTreeMap` rather than `HashMap` ensures a deterministic iteration
/// order, which is important for reproducible test output and serialisation.
pub struct ForwardIndex {
    /// token → sorted list of sequence numbers that match.
    index: BTreeMap<SearchToken, Vec<u64>>,
}

impl ForwardIndex {
    /// Create an empty index.
    pub fn new() -> Self {
        Self {
            index: BTreeMap::new(),
        }
    }

    /// Insert `sequence` under each search token in `tokens`.
    ///
    /// `tokens` are derived by the caller via [`SearchKey::token_for`]; the
    /// index itself never sees or stores raw keywords.
    ///
    /// Complexity: O(k · log I) where k = |tokens|, I = number of distinct
    /// tokens currently in the index.
    pub fn index_entry(&mut self, sequence: u64, tokens: &[SearchToken]) {
        for &token in tokens {
            let bucket = self.index.entry(token).or_default();
            // Insert in sorted order to support range-based sub-queries.
            match bucket.binary_search(&sequence) {
                Ok(_) => { /* duplicate — idempotent */ }
                Err(pos) => bucket.insert(pos, sequence),
            }
        }
    }

    /// Return all sequence numbers matching `token`.
    ///
    /// Returns an empty slice if no entries match.
    ///
    /// Complexity: O(log I + m) — one BTreeMap lookup + slice copy.
    pub fn search(&self, token: &SearchToken) -> Vec<u64> {
        self.index
            .get(token)
            .cloned()
            .unwrap_or_default()
    }

    /// Remove all index entries for sequences in `removed`.
    ///
    /// Called after compaction to keep the index consistent with the live log.
    ///
    /// Complexity: O(|removed| · log I · log m) — binary search + removal per
    /// sequence in each bucket.
    pub fn purge_sequences(&mut self, removed: &[u64]) {
        for bucket in self.index.values_mut() {
            bucket.retain(|seq| !removed.contains(seq));
        }
        // Drop empty buckets to avoid unbounded growth.
        self.index.retain(|_, bucket| !bucket.is_empty());
    }

    /// Number of distinct tokens currently indexed.
    pub fn token_count(&self) -> usize {
        self.index.len()
    }

    /// Total number of (token, sequence) pairs in the index.
    pub fn entry_count(&self) -> usize {
        self.index.values().map(|v| v.len()).sum()
    }
}

impl Default for ForwardIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ── KeywordExtractor ─────────────────────────────────────────────────────────

/// Extracts a canonical set of searchable keywords from a log-entry field set.
///
/// This is kept as a pure function rather than a trait to avoid dynamic
/// dispatch overhead.  Callers can extend keyword extraction by appending to
/// the returned `Vec`.
///
/// ### Current extraction strategy
///
/// * `actor`  — indexed verbatim.
/// * `action` — indexed verbatim and by its "verb" prefix up to the first `.`
///              (e.g. `"record.create"` → tokens for both `"record.create"` and
///              `"record"`).
/// * `target` — indexed verbatim.
/// * `result` — indexed verbatim.
///
/// Consumers may call `extract_keywords` and then append domain-specific extra
/// keywords before calling `ForwardIndex::index_entry`.
///
/// Complexity: O(|actor| + |action| + |target| + |result|).
pub fn extract_keywords<'a>(
    actor: &'a str,
    action: &'a str,
    target: &'a str,
    result: &'a str,
) -> Vec<alloc::string::String> {
    let mut kws: Vec<alloc::string::String> = Vec::with_capacity(5);

    push_nonempty(&mut kws, actor);
    push_nonempty(&mut kws, action);

    // Index action prefix (namespace) e.g. "record" from "record.create".
    if let Some(dot) = action.find('.') {
        push_nonempty(&mut kws, &action[..dot]);
    }

    push_nonempty(&mut kws, target);
    push_nonempty(&mut kws, result);

    kws
}

#[inline]
fn push_nonempty(vec: &mut Vec<alloc::string::String>, s: &str) {
    if !s.is_empty() {
        vec.push(alloc::string::String::from(s));
    }
}

// ── SearchEngine ─────────────────────────────────────────────────────────────

/// High-level API combining a [`SearchKey`] and a [`ForwardIndex`].
///
/// Maintains the index and exposes `index_entry` / `query` / `purge` without
/// exposing raw token arithmetic to callers.
pub struct SearchEngine {
    key: SearchKey,
    index: ForwardIndex,
}

impl SearchEngine {
    /// Create a new engine with the given search key.
    pub fn new(key: SearchKey) -> Self {
        Self {
            key,
            index: ForwardIndex::new(),
        }
    }

    /// Index a log entry using automatically extracted keywords.
    ///
    /// `extra_keywords` allows callers to supplement the default extraction.
    ///
    /// Complexity: O(k · log I) where k = total keyword count.
    pub fn index_entry(
        &mut self,
        sequence: u64,
        actor: &str,
        action: &str,
        target: &str,
        result: &str,
        extra_keywords: &[&str],
    ) {
        let mut kws = extract_keywords(actor, action, target, result);
        for &extra in extra_keywords {
            push_nonempty(&mut kws, extra);
        }

        let tokens: Vec<SearchToken> = kws.iter().map(|kw| self.key.token_for(kw)).collect();
        self.index.index_entry(sequence, &tokens);
    }

    /// Search for entries matching `keyword`.
    ///
    /// The keyword is hashed into a token before the lookup so the engine never
    /// stores plaintext keywords.
    ///
    /// Complexity: O(log I + m).
    pub fn query(&self, keyword: &str) -> Vec<u64> {
        let token = self.key.token_for(keyword);
        self.index.search(&token)
    }

    /// Remove compacted sequences from the index.
    ///
    /// Complexity: O(|removed| · log I · log m).
    pub fn purge(&mut self, removed_sequences: &[u64]) {
        self.index.purge_sequences(removed_sequences);
    }

    /// Number of distinct tokens in the index.
    pub fn token_count(&self) -> usize {
        self.index.token_count()
    }

    /// Total (token, sequence) pairs indexed.
    pub fn entry_count(&self) -> usize {
        self.index.entry_count()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use alloc::vec;
    use super::*;

    fn engine() -> SearchEngine {
        SearchEngine::new(SearchKey::test_key())
    }

    #[test]
    fn roundtrip_index_and_query() {
        let mut eng = engine();
        eng.index_entry(1, "alice", "record.create", "patient:42", "ok", &[]);
        eng.index_entry(2, "bob", "record.read", "patient:42", "ok", &[]);
        eng.index_entry(3, "alice", "access.grant", "patient:42", "ok", &[]);

        // Query by actor
        assert_eq!(eng.query("alice"), vec![1, 3]);
        assert_eq!(eng.query("bob"), vec![2]);

        // Query by action namespace prefix
        assert_eq!(eng.query("record"), vec![1, 2]);
        assert_eq!(eng.query("access"), vec![3]);

        // Query by full action
        assert_eq!(eng.query("record.create"), vec![1]);

        // Query by target
        let target_matches = eng.query("patient:42");
        assert_eq!(target_matches, vec![1, 2, 3]);

        // Unknown keyword returns empty
        assert!(eng.query("unknown:xyz").is_empty());
    }

    #[test]
    fn purge_removes_compacted_sequences() {
        let mut eng = engine();
        eng.index_entry(1, "alice", "create", "r:1", "ok", &[]);
        eng.index_entry(2, "alice", "create", "r:2", "ok", &[]);
        eng.index_entry(3, "alice", "create", "r:3", "ok", &[]);

        // Compact entries 1 and 2
        eng.purge(&[1, 2]);

        let hits = eng.query("alice");
        assert_eq!(hits, vec![3]);
    }

    #[test]
    fn extra_keywords_are_indexed() {
        let mut eng = engine();
        eng.index_entry(10, "sys", "boot", "node:A", "ok", &["datacenter:EU", "high-priority"]);

        assert_eq!(eng.query("datacenter:EU"), vec![10]);
        assert_eq!(eng.query("high-priority"), vec![10]);
    }

    #[test]
    fn different_keys_produce_different_tokens() {
        let k1 = SearchKey::from_bytes(&[0x01; 32]).unwrap();
        let k2 = SearchKey::from_bytes(&[0x02; 32]).unwrap();
        assert_ne!(k1.token_for("hello"), k2.token_for("hello"));
    }

    #[test]
    fn index_is_idempotent_for_duplicate_sequences() {
        let mut idx = ForwardIndex::new();
        let token = SearchKey::test_key().token_for("dup");
        idx.index_entry(5, &[token]);
        idx.index_entry(5, &[token]); // duplicate
        assert_eq!(idx.search(&token), vec![5]); // still only one entry
    }

    #[test]
    fn keyword_extraction_splits_action_namespace() {
        let kws = extract_keywords("user", "record.delete", "res:99", "denied");
        assert!(kws.contains(&alloc::string::String::from("record.delete")));
        assert!(kws.contains(&alloc::string::String::from("record")));
    }

    #[test]
    fn empty_keyword_not_indexed() {
        let kws = extract_keywords("", "action", "target", "ok");
        // "" should not appear
        assert!(!kws.contains(&alloc::string::String::from("")));
    }

    #[test]
    fn from_bytes_rejects_wrong_length() {
        assert!(SearchKey::from_bytes(&[0u8; 16]).is_err());
        assert!(SearchKey::from_bytes(&[0u8; 64]).is_err());
    }
}
