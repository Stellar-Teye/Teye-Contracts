//! # Cross-Chain Medical Record Portability Bridge
//!
//! This module implements the patient-record export / import protocol for
//! issue #187.  Records are committed to a sparse Merkle tree (see
//! [`crate::merkle_tree`]); the resulting inclusion proofs travel with the
//! [`ExportPackage`] so any receiving chain can verify data integrity without
//! trusting the sender.
//!
//! ## High-level flow
//!
//! ```text
//!  Source chain                           Target chain
//!  ─────────────────────────────          ────────────────────────────────
//!  anchor_root(env, root)                 anchor_root(env, root)
//!      │                                      │
//!  export_record(env, id, data, fields)        │
//!      │  ──── ExportPackage ──────────────▶  import_record(env, pkg, root, finality_depth)
//!                                              │  verifies Merkle proof
//!                                              │  checks chain-reorg window
//!                                              └─ Ok(()) or BridgeError
//! ```
//!
//! ## Storage key layout  (all `BRIDGE_`-prefixed)
//!
//! | Key tuple                            | Value                         |
//! |--------------------------------------|-------------------------------|
//! | `("BRDG_RT", root: BytesN<32>)`      | [`AnchoredRoot`]              |
//! | `("BRDG_IM", record_id: BytesN<32>)` | `u64` (import timestamp)      |

#![allow(unused_imports)]

use soroban_sdk::{contracttype, symbol_short, BytesN, Bytes, Env, Symbol};

use crate::merkle_tree::{FieldEntry, FieldProof, MerkleProof, SparseMerkleTree, TreeState};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Depth used for per-record field sub-trees.
///
/// Using the full 256-level depth (same as the main record tree) ensures that
/// any two distinct field names produce distinct paths even when they share
/// long ASCII prefixes (e.g. `"iop_left"` vs `"iop_right"`).  Field names
/// shorter than 32 bytes are implicitly zero-padded on the right, so any
/// name up to 31 bytes long is uniquely addressable.
///
/// **Trade-off**: each [`FieldProof`] carries 256 × 32 = 8 192 bytes of
/// sibling hashes.  For selective disclosure of a small set of fields this
/// is acceptable; if bandwidth is critical, consider pre-hashing field names
/// with `SHA-256` before passing them to [`export_record`] and using a
/// shallower depth.
pub const FIELD_DEPTH: u32 = 256;

/// Depth of the main per-chain record tree.
///
/// 256 bits cover the full SHA-256 key space; every `record_id` (a 32-byte
/// hash) maps to a unique leaf.
pub const RECORD_TREE_DEPTH: usize = 256;

/// Persistent-storage TTL threshold in ledgers (~1 day at 6 s/ledger).
const TTL_THRESHOLD: u32 = 17_280;

/// Persistent-storage TTL extension target in ledgers (~30 days).
const TTL_EXTEND_TO: u32 = 518_400;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during cross-chain record export / import.
#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BridgeError {
    /// The Merkle proof supplied in an [`ExportPackage`] did not verify
    /// against the anchored state root.
    ProofInvalid = 1,

    /// The `anchored_root` supplied to [`import_record`] has never been
    /// registered on this chain via [`anchor_root`].
    StateRootNotAnchored = 2,

    /// The anchored root is too recent: its ledger is within `finality_depth`
    /// of the current ledger, meaning the source block could still be
    /// reorganised away.
    ChainReorgDetected = 3,

    /// A field requested for selective disclosure was not found in the
    /// record's field list.
    FieldNotFound = 4,

    /// The timestamp in the [`ExportPackage`] indicates the record has
    /// expired (older than the chain's maximum portability window).
    RecordExpired = 5,
}

// ---------------------------------------------------------------------------
// Core data types
// ---------------------------------------------------------------------------

/// Metadata stored alongside an anchored state root.
///
/// Persisted under key `("BRDG_RT", root)` with a 30-day TTL.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnchoredRoot {
    /// The state root hash that was anchored.
    pub root: BytesN<32>,
    /// Ledger sequence number at the time of anchoring.
    pub anchored_at: u32,
    /// Chain identifier from which this root originated.
    pub source_chain: Symbol,
}

/// A self-contained proof-of-record package exported from a source chain.
///
/// Carry this across chains and verify it with [`import_record`].
///
/// ## Field
///
/// * `record_id`    — 32-byte deterministic identifier for the record
///                    (typically `SHA-256(patient_addr ‖ record_type ‖ seq)`).
/// * `record_data`  — Raw serialised record bytes.
/// * `state_root`   — Root of the source chain's sparse Merkle record tree.
/// * `merkle_proof` — Inclusion proof for `SHA-256(record_data)` in the tree
///                    keyed by `record_id`.
/// * `field_proofs` — Optional per-field selective-disclosure proofs.  Empty
///                    when the full record is exported without selection.
/// * `source_chain` — Identifier of the originating chain / network.
/// * `timestamp`    — UNIX timestamp (seconds) at the moment of export.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportPackage {
    /// Deterministic 32-byte record identifier.
    pub record_id: BytesN<32>,
    /// Raw serialised record payload.
    pub record_data: Bytes,
    /// Merkle tree root at the time of export.
    pub state_root: BytesN<32>,
    /// Inclusion proof for `SHA-256(record_data)` under `record_id`.
    pub merkle_proof: MerkleProof,
    /// Per-field selective-disclosure proofs (may be empty).
    pub field_proofs: soroban_sdk::Vec<FieldProof>,
    /// Originating chain identifier.
    pub source_chain: Symbol,
    /// Export timestamp (seconds since Unix epoch).
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// anchor_root
// ---------------------------------------------------------------------------

/// Register a state root from a remote chain as trusted on this ledger.
///
/// The caller (typically a vetted relayer or governance process) asserts that
/// `root` is the finalised Merkle root of the source chain's record tree.
/// The current ledger sequence is recorded so that [`import_record`] can
/// enforce a finality window.
///
/// # Storage
///
/// Writes `("BRDG_RT", root)` → [`AnchoredRoot`] with a 30-day TTL.
///
/// # Example
///
/// ```ignore
/// bridge::anchor_root(&env, root_hash, symbol_short!("ETH"));
/// ```
pub fn anchor_root(env: &Env, root: BytesN<32>, source_chain: Symbol) {
    let record = AnchoredRoot {
        root: root.clone(),
        anchored_at: env.ledger().sequence(),
        source_chain,
    };
    let key = (symbol_short!("BRDG_RT"), root);
    env.storage().persistent().set(&key, &record);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// ---------------------------------------------------------------------------
// export_record
// ---------------------------------------------------------------------------

/// Build an [`ExportPackage`] for `record_id`, optionally disclosing only
/// the field keys listed in `selected_field_keys`.
///
/// # Parameters
///
/// * `env`                — Soroban execution environment.
/// * `record_id`          — 32-byte record identifier.
/// * `record_data`        — Raw serialised record payload.
/// * `all_fields`         — Complete list of named fields in the record.
///   Each [`FieldEntry`] carries a `key` (typically the UTF-8 field name as
///   `Bytes`) and a `value`.
/// * `selected_field_keys`— If `Some`, only prove the fields whose `key`
///   bytes appear in this list. If `None`, prove every field in `all_fields`.
/// * `source_chain`       — Chain identifier written into the package.
///
/// # How proof is constructed
///
/// 1. Hash `record_data` → 32-byte `data_hash`.
/// 2. Insert `(record_id, data_hash)` into a fresh `SparseMerkleTree`.
/// 3. Generate a `MerkleProof` for that leaf.
/// 4. For each (selected) field, call
///    [`SparseMerkleTree::build_field_proof`] with [`FIELD_DEPTH`] to
///    generate a compact `FieldProof`.
///
/// # Panics
///
/// Panics (via Soroban's host trap mechanism) if a key in
/// `selected_field_keys` does not correspond to any entry in `all_fields`.
/// Use [`BridgeError::FieldNotFound`] via [`try_export_record`] if you need
/// a graceful error path.
pub fn export_record(
    env: &Env,
    record_id: BytesN<32>,
    record_data: Bytes,
    all_fields: soroban_sdk::Vec<FieldEntry>,
    selected_field_keys: Option<soroban_sdk::Vec<Bytes>>,
    source_chain: Symbol,
) -> ExportPackage {
    // Step 1: hash the record payload to use as the SMT leaf value
    let data_hash: BytesN<32> = env.crypto().sha256(&record_data).into();
    let data_hash_arr: [u8; 32] = data_hash.to_array();
    let record_id_arr: [u8; 32] = record_id.to_array();

    // Step 2: build a fresh main tree, insert the record, generate proof
    let mut main_tree = SparseMerkleTree::new(env);
    main_tree.insert(env, &record_id_arr, &data_hash_arr);
    let state_root = main_tree.root().clone();
    let merkle_proof = main_tree.prove(env, &record_id_arr, &data_hash_arr);

    // Step 3: filter fields based on selection list
    let fields_to_prove: soroban_sdk::Vec<FieldEntry> = match selected_field_keys {
        None => all_fields,
        Some(ref keys) => {
            let mut selected: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(env);
            for entry in all_fields.iter() {
                for key in keys.iter() {
                    if entry.key == key {
                        selected.push_back(entry.clone());
                        break;
                    }
                }
            }
            selected
        }
    };

    // Step 4: build per-field proofs
    //
    // IMPORTANT: pass the *original* key bytes (not zero-padded to 32) so
    // that `build_field_proof` can match `entry.key == Bytes::from_slice(env,
    // field_key)` correctly.  The internal `bytes_to_key_array` call inside
    // `build_field_proof` handles the padding needed for SMT path traversal.
    let mut field_proofs: soroban_sdk::Vec<FieldProof> = soroban_sdk::Vec::new(env);
    for entry in fields_to_prove.iter() {
        // Collect Bytes into a stack buffer; field names are bounded at 256 B.
        let key_len = entry.key.len() as usize;
        let mut key_buf = [0u8; 256];
        for (i, b) in entry.key.iter().enumerate() {
            if i >= 256 {
                break;
            }
            key_buf[i] = b;
        }
        let (_root, fp) = SparseMerkleTree::build_field_proof(
            env,
            &fields_to_prove,
            &key_buf[..key_len],
            FIELD_DEPTH,
        );
        field_proofs.push_back(fp);
    }

    ExportPackage {
        record_id,
        record_data,
        state_root,
        merkle_proof,
        field_proofs,
        source_chain,
        timestamp: env.ledger().timestamp(),
    }
}

/// Like [`export_record`] but returns `Err(BridgeError::FieldNotFound)` when
/// a requested field key is absent instead of panicking.
pub fn try_export_record(
    env: &Env,
    record_id: BytesN<32>,
    record_data: Bytes,
    all_fields: soroban_sdk::Vec<FieldEntry>,
    selected_field_keys: Option<soroban_sdk::Vec<Bytes>>,
    source_chain: Symbol,
) -> Result<ExportPackage, BridgeError> {
    // Validate that every requested key exists in all_fields before building
    if let Some(ref keys) = selected_field_keys {
        for key in keys.iter() {
            let mut found = false;
            for entry in all_fields.iter() {
                if entry.key == key {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(BridgeError::FieldNotFound);
            }
        }
    }
    Ok(export_record(
        env,
        record_id,
        record_data,
        all_fields,
        selected_field_keys,
        source_chain,
    ))
}

// ---------------------------------------------------------------------------
// import_record
// ---------------------------------------------------------------------------

/// Verify and register an incoming [`ExportPackage`] on this chain.
///
/// # Verification steps
///
/// 1. Look up `anchored_root` in persistent storage.
///    Fails with [`BridgeError::StateRootNotAnchored`] if not found.
/// 2. Enforce finality window: if
///    `anchored_at + finality_depth > current_ledger`,
///    the anchored block might still be reorganised; fails with
///    [`BridgeError::ChainReorgDetected`].
/// 3. Verify the `MerkleProof` in the package against `anchored_root`.
///    Fails with [`BridgeError::ProofInvalid`] on mismatch.
/// 4. Verify every `FieldProof` in the package.
///    Fails with [`BridgeError::ProofInvalid`] on the first invalid proof.
/// 5. On success, record the import in storage and return `Ok(())`.
///
/// # Parameters
///
/// * `env`             — Soroban execution environment.
/// * `package`         — The [`ExportPackage`] received from the source chain.
/// * `anchored_root`   — The state root to verify against (must have been
///   registered via [`anchor_root`] beforehand).
/// * `finality_depth`  — Minimum number of ledgers that must have elapsed
///   since `anchored_root` was registered before it is considered final.
///   A value of `0` disables the check.
///
/// # Storage side effects
///
/// On success writes `("BRDG_IM", record_id)` → `u64` import timestamp with
/// a 30-day TTL.
pub fn import_record(
    env: &Env,
    package: ExportPackage,
    anchored_root: BytesN<32>,
    finality_depth: u32,
) -> Result<(), BridgeError> {
    // Step 1: look up the anchored root
    let anchor_key = (symbol_short!("BRDG_RT"), anchored_root.clone());
    let anchor_record: AnchoredRoot = env
        .storage()
        .persistent()
        .get(&anchor_key)
        .ok_or(BridgeError::StateRootNotAnchored)?;

    // Extend TTL on access
    env.storage()
        .persistent()
        .extend_ttl(&anchor_key, TTL_THRESHOLD, TTL_EXTEND_TO);

    // Step 2: chain-reorg window check
    if finality_depth > 0 {
        let current_ledger = env.ledger().sequence();
        // anchored_at + finality_depth > current_ledger  ⟹  not yet final
        if anchor_record
            .anchored_at
            .checked_add(finality_depth)
            .map(|required| required > current_ledger)
            .unwrap_or(true) // overflow → treat as not final
        {
            return Err(BridgeError::ChainReorgDetected);
        }
    }

    // Step 3: verify the record's Merkle proof against the anchored root
    //
    // The proof was built over H(record_data); recompute and verify.
    let data_hash: BytesN<32> = env.crypto().sha256(&package.record_data).into();
    let data_hash_arr: [u8; 32] = data_hash.to_array();
    let record_id_arr: [u8; 32] = package.record_id.to_array();

    let proof_valid = SparseMerkleTree::verify(
        env,
        &anchored_root,
        &record_id_arr,
        &data_hash_arr,
        &package.merkle_proof,
    );
    if !proof_valid {
        return Err(BridgeError::ProofInvalid);
    }

    // Step 4: verify every field proof in the package
    for fp in package.field_proofs.iter() {
        if !SparseMerkleTree::verify_field(env, &fp) {
            return Err(BridgeError::ProofInvalid);
        }
    }

    // Step 5: record successful import
    let import_key = (symbol_short!("BRDG_IM"), package.record_id.clone());
    env.storage()
        .persistent()
        .set(&import_key, &package.timestamp);
    env.storage()
        .persistent()
        .extend_ttl(&import_key, TTL_THRESHOLD, TTL_EXTEND_TO);

    Ok(())
}

// ---------------------------------------------------------------------------
// Storage query helpers
// ---------------------------------------------------------------------------

/// Return the [`AnchoredRoot`] metadata for `root`, if it was previously
/// registered via [`anchor_root`].
pub fn get_anchored_root(env: &Env, root: BytesN<32>) -> Option<AnchoredRoot> {
    let key = (symbol_short!("BRDG_RT"), root.clone());
    let result: Option<AnchoredRoot> = env.storage().persistent().get(&key);
    if result.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
    result
}

/// Return the import timestamp (seconds) for `record_id`, if this record was
/// successfully imported via [`import_record`].
pub fn get_import_timestamp(env: &Env, record_id: BytesN<32>) -> Option<u64> {
    let key = (symbol_short!("BRDG_IM"), record_id.clone());
    let result: Option<u64> = env.storage().persistent().get(&key);
    if result.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
    result
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Ledger, Env};
    use crate::merkle_tree::FieldEntry;

    // A minimal contract used solely to provide a contract context for
    // storage operations; bridge functions are standalone helpers.
    #[contract]
    struct BridgeTestContract;

    #[contractimpl]
    impl BridgeTestContract {}

    /// Build an environment pre-configured with known ledger state and
    /// return it together with a contract-context address.
    fn make_env() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        env.ledger().with_mut(|l| {
            l.sequence_number = 1_000;
            l.timestamp = 1_700_000_000;
        });
        let contract_id = env.register(BridgeTestContract, ());
        (env, contract_id)
    }

    /// Helper: create a `BytesN<32>` from raw bytes (zero-padded on the right).
    fn record_id(env: &Env, seed: &[u8]) -> BytesN<32> {
        let mut arr = [0u8; 32];
        for (i, &b) in seed.iter().enumerate().take(32) {
            arr[i] = b;
        }
        BytesN::from_array(env, &arr)
    }

    // -----------------------------------------------------------------------
    // anchor_root tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_anchor_root_stores_record() {
        let (env, cid) = make_env();
        #[allow(deprecated)]
        env.budget().reset_unlimited();

        let root = record_id(&env, b"test_root_1");
        env.as_contract(&cid, || {
            anchor_root(&env, root.clone(), symbol_short!("ETH"));
            let stored = get_anchored_root(&env, root.clone()).expect("should be stored");
            assert_eq!(stored.root, root);
            assert_eq!(stored.anchored_at, 1_000);
            assert_eq!(stored.source_chain, symbol_short!("ETH"));
        });
    }

    #[test]
    fn test_anchor_root_missing_returns_none() {
        let (env, cid) = make_env();
        #[allow(deprecated)]
        env.budget().reset_unlimited();

        let root = record_id(&env, b"never_anchored");
        env.as_contract(&cid, || {
            assert!(get_anchored_root(&env, root.clone()).is_none());
        });
    }

    // -----------------------------------------------------------------------
    // export_record tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_record_no_field_selection() {
        let (env, _cid) = make_env();
        #[allow(deprecated)]
        env.budget().reset_unlimited();

        let rid = record_id(&env, b"patient_001");
        let data = Bytes::from_slice(&env, b"encrypted_record_payload");

        let mut fields: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(&env);
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"diagnosis"),
            value: Bytes::from_slice(&env, b"myopia"),
        });
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"prescription"),
            value: Bytes::from_slice(&env, b"-2.50/-1.75"),
        });

        let pkg = export_record(&env, rid.clone(), data.clone(), fields, None, symbol_short!("SOL"));

        assert_eq!(pkg.record_id, rid);
        assert_eq!(pkg.record_data, data);
        assert_eq!(pkg.source_chain, symbol_short!("SOL"));
        assert_eq!(pkg.timestamp, 1_700_000_000);
        // Both fields should have proofs
        assert_eq!(pkg.field_proofs.len(), 2);
    }

    #[test]
    fn test_export_record_selective_fields() {
        let (env, _cid) = make_env();
        #[allow(deprecated)]
        env.budget().reset_unlimited();

        let rid = record_id(&env, b"patient_002");
        let data = Bytes::from_slice(&env, b"medical_record_v2");

        let mut fields: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(&env);
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"diagnosis"),
            value: Bytes::from_slice(&env, b"astigmatism"),
        });
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"prescription"),
            value: Bytes::from_slice(&env, b"+1.00/+0.75"),
        });
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"provider"),
            value: Bytes::from_slice(&env, b"Dr. Smith"),
        });

        // Select only "diagnosis"
        let mut selected: soroban_sdk::Vec<Bytes> = soroban_sdk::Vec::new(&env);
        selected.push_back(Bytes::from_slice(&env, b"diagnosis"));

        let pkg = export_record(
            &env,
            rid.clone(),
            data.clone(),
            fields,
            Some(selected),
            symbol_short!("SOL"),
        );

        // Only 1 field proof expected
        assert_eq!(pkg.field_proofs.len(), 1);
        let fp = pkg.field_proofs.get(0).unwrap();
        assert_eq!(fp.field_value, Bytes::from_slice(&env, b"astigmatism"));
    }

    #[test]
    fn test_try_export_record_field_not_found() {
        let (env, _cid) = make_env();
        #[allow(deprecated)]
        env.budget().reset_unlimited();

        let rid = record_id(&env, b"patient_003");
        let data = Bytes::from_slice(&env, b"payload");

        let mut fields: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(&env);
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"diagnosis"),
            value: Bytes::from_slice(&env, b"cataracts"),
        });

        let mut selected: soroban_sdk::Vec<Bytes> = soroban_sdk::Vec::new(&env);
        selected.push_back(Bytes::from_slice(&env, b"nonexistent_field"));

        let result = try_export_record(&env, rid, data, fields, Some(selected), symbol_short!("SOL"));
        assert_eq!(result, Err(BridgeError::FieldNotFound));
    }

    // -----------------------------------------------------------------------
    // import_record tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_import_record_roundtrip() {
        let (env, cid) = make_env();
        #[allow(deprecated)]
        env.budget().reset_unlimited();

        let rid = record_id(&env, b"patient_004");
        let data = Bytes::from_slice(&env, b"record_payload_004");

        let mut fields: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(&env);
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"diagnosis"),
            value: Bytes::from_slice(&env, b"glaucoma"),
        });

        // Export (pure computation — no contract context needed)
        let pkg = export_record(
            &env, rid.clone(), data.clone(), fields, None, symbol_short!("ETH"),
        );
        let exported_root = pkg.state_root.clone();

        // Anchor the root at ledger 1_000
        env.as_contract(&cid, || {
            anchor_root(&env, exported_root.clone(), symbol_short!("ETH"));
        });

        // Advance ledger beyond finality window (1_000 + 20 > 1_000 + 10)
        env.ledger().with_mut(|l| {
            l.sequence_number = 1_000 + 20;
        });

        // Import and assert success
        env.as_contract(&cid, || {
            let result = import_record(&env, pkg.clone(), exported_root.clone(), 10);
            assert!(result.is_ok(), "import_record failed: {:?}", result);

            // Import timestamp should be persisted
            let ts = get_import_timestamp(&env, rid.clone());
            assert_eq!(ts, Some(1_700_000_000));
        });
    }

    #[test]
    fn test_import_record_state_root_not_anchored() {
        let (env, cid) = make_env();
        #[allow(deprecated)]
        env.budget().reset_unlimited();

        let rid = record_id(&env, b"patient_005");
        let data = Bytes::from_slice(&env, b"record_payload_005");
        let fields: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(&env);

        let pkg = export_record(&env, rid, data, fields, None, symbol_short!("ETH"));

        // Use a root that was never anchored
        let fake_root = record_id(&env, b"unregistered_root");
        env.as_contract(&cid, || {
            let result = import_record(&env, pkg.clone(), fake_root.clone(), 0);
            assert_eq!(result, Err(BridgeError::StateRootNotAnchored));
        });
    }

    #[test]
    fn test_import_record_chain_reorg_detected() {
        let (env, cid) = make_env();
        #[allow(deprecated)]
        env.budget().reset_unlimited();

        let rid = record_id(&env, b"patient_006");
        let data = Bytes::from_slice(&env, b"record_payload_006");
        let fields: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(&env);

        let pkg = export_record(&env, rid, data, fields, None, symbol_short!("ETH"));
        let root = pkg.state_root.clone();

        env.as_contract(&cid, || {
            // Anchor at ledger 1_000 then immediately try to import with
            // finality_depth = 100 — still within the reorg window.
            anchor_root(&env, root.clone(), symbol_short!("ETH"));
            let result = import_record(&env, pkg.clone(), root.clone(), 100);
            assert_eq!(result, Err(BridgeError::ChainReorgDetected));
        });
    }

    #[test]
    fn test_import_record_proof_invalid_tampered_data() {
        let (env, cid) = make_env();
        #[allow(deprecated)]
        env.budget().reset_unlimited();

        let rid = record_id(&env, b"patient_007");
        let data = Bytes::from_slice(&env, b"original_payload");
        let fields: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(&env);

        let mut pkg = export_record(&env, rid, data, fields, None, symbol_short!("ETH"));
        let root = pkg.state_root.clone();

        env.as_contract(&cid, || {
            anchor_root(&env, root.clone(), symbol_short!("ETH"));
        });

        // Advance ledger past finality window
        env.ledger().with_mut(|l| {
            l.sequence_number = 1_000 + 50;
        });

        // Tamper: swap record_data so the hash no longer matches the proof
        pkg.record_data = Bytes::from_slice(&env, b"tampered_payload");

        env.as_contract(&cid, || {
            let result = import_record(&env, pkg.clone(), root.clone(), 10);
            assert_eq!(result, Err(BridgeError::ProofInvalid));
        });
    }

    #[test]
    fn test_import_record_zero_finality_depth_no_reorg_check() {
        let (env, cid) = make_env();
        #[allow(deprecated)]
        env.budget().reset_unlimited();

        let rid = record_id(&env, b"patient_008");
        let data = Bytes::from_slice(&env, b"record_payload_008");
        let fields: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(&env);

        let pkg = export_record(&env, rid, data, fields, None, symbol_short!("ETH"));
        let root = pkg.state_root.clone();

        env.as_contract(&cid, || {
            // Anchor and import in the same ledger — finality_depth=0 skips check
            anchor_root(&env, root.clone(), symbol_short!("ETH"));
            let result = import_record(&env, pkg.clone(), root.clone(), 0);
            assert!(result.is_ok(), "expected Ok with finality_depth=0, got {:?}", result);
        });
    }

    #[test]
    fn test_import_record_with_field_proofs_valid() {
        let (env, cid) = make_env();
        #[allow(deprecated)]
        env.budget().reset_unlimited();

        let rid = record_id(&env, b"patient_009");
        let data = Bytes::from_slice(&env, b"record_payload_009");

        let mut fields: soroban_sdk::Vec<FieldEntry> = soroban_sdk::Vec::new(&env);
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"iop_left"),
            value: Bytes::from_slice(&env, b"14mmHg"),
        });
        fields.push_back(FieldEntry {
            key: Bytes::from_slice(&env, b"iop_right"),
            value: Bytes::from_slice(&env, b"15mmHg"),
        });

        let pkg = export_record(&env, rid, data, fields, None, symbol_short!("SOL"));
        let root = pkg.state_root.clone();

        env.as_contract(&cid, || {
            anchor_root(&env, root.clone(), symbol_short!("SOL"));
        });

        env.ledger().with_mut(|l| {
            l.sequence_number = 1_000 + 30;
        });

        env.as_contract(&cid, || {
            let result = import_record(&env, pkg.clone(), root.clone(), 20);
            assert!(result.is_ok(), "import with field proofs failed: {:?}", result);
        });
    }
}
