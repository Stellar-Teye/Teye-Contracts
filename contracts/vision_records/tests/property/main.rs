#![allow(clippy::unwrap_used, clippy::expect_used, clippy::arithmetic_side_effects)]
//! Property-based test suite entry point.
//!
//! This file loads all property test sub-modules. Run with:
//!
//! ```bash
//! cargo test --test property --all
//! ```
//!
//! To increase the number of generated cases:
//!
//! ```bash
//! PROPTEST_CASES=512 cargo test --test property --all
//! ```

mod access;
mod core;
mod rbac;
mod state_machine;
