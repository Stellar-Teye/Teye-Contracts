//! # `audit` — Distributed, Tamper-Evident Audit Logging
//!
//! This crate provides a production-grade, `no_std`-compatible audit logging
//! system built on:
//!
//! * **Hash chains** — every entry's `prev_hash` links it to its predecessor,
//!   making retroactive modification detectable in O(k·L) time.
//! * **Merkle trees** — entries are committed into an append-only Merkle log
//!   whose root can be published as a compact tamper-evidence beacon.
//! * **Consistency proofs** (RFC 6962 §2.1.2) — any two roots can be proven
//!   consistent without replaying the full log (O(log n) proof size).
//! * **Searchable symmetric encryption (SSE-1)** — keyword search without
//!   exposing plaintext keywords to the index store.
//!
//! ## Module layout
//!
//! | Module            | Purpose                                                    |
//! |-------------------|------------------------------------------------------------|
//! | [`types`]         | Core domain types (`LogEntry`, `AuditError`, …)           |
//! | [`merkle_log`]    | `MerkleLog` — append-only log per segment                  |
//! | [`consistency`]   | `ConsistencyProver` / `ConsistencyProof` (RFC 6962)        |
//! | [`search`]        | `SearchEngine` / `ForwardIndex` — SSE-1 keyword search     |
//!
//! ## Quick start
//!
//! ```rust
//! use audit::{
//!     types::LogSegmentId,
//!     merkle_log::MerkleLog,
//!     search::{SearchEngine, SearchKey},
//! };
//!
//! // Create a log for the "healthcare.access" segment.
//! let seg = LogSegmentId::new("healthcare.access").unwrap();
//! let mut log = MerkleLog::new(seg);
//!
//! // Append entries.
//! let seq = log.append(1_700_000_000, "alice", "record.read", "patient:42", "ok");
//!
//! // Generate and verify a Merkle inclusion proof.
//! let root  = log.current_root();
//! let proof = log.inclusion_proof(seq).unwrap();
//! proof.verify(&root).expect("proof must verify");
//!
//! // Full-text search over actor / action / target / result fields.
//! let key = SearchKey::from_bytes(&[0x42u8; 32]).unwrap();
//! let mut engine = SearchEngine::new(key);
//! engine.index_entry(seq, "alice", "record.read", "patient:42", "ok", &[]);
//! assert_eq!(engine.query("alice"), vec![seq]);
//! ```
//!
//! ## `no_std` compatibility
//!
//! The crate is `#![no_std]` with `extern crate alloc`.  It compiles for Wasm
//! targets (Soroban/Stellar) without modification.

#![no_std]

extern crate alloc;

pub mod consistency;
pub mod merkle_log;
pub mod search;
pub mod types;
