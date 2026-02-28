# ZK Verifier Compilation Checklist

## Pre-Compilation Verification

### ✅ File Structure
- [x] `src/lib.rs` exists and has proper exports
- [x] `src/vk.rs` defines G1Point, G2Point, VerificationKey
- [x] `src/verifier.rs` defines Proof and re-exports G1Point, G2Point
- [x] `src/helpers.rs` defines ZkAccessHelper, MerkleVerifier
- [x] `src/audit.rs` defines AuditRecord, AuditTrail
- [x] `src/events.rs` defines AccessRejectedEvent
- [x] `src/plonk.rs` defines PlonkVerifier
- [x] `tests/bench_verify.rs` exists
- [x] `tests/test_nonce_replay.rs` exists
- [x] `tests/test_zk_access.rs` exists

### ✅ Type Definitions
- [x] G1Point defined in vk.rs (not duplicated)
- [x] G2Point defined in vk.rs (not duplicated)
- [x] Proof defined in verifier.rs
- [x] AccessRequest defined in lib.rs
- [x] ContractError defined in lib.rs
- [x] VerificationKey defined in vk.rs

### ✅ Public Exports in lib.rs
- [x] `pub mod events;`
- [x] `pub mod plonk;`
- [x] `pub mod verifier;`
- [x] `pub mod vk;`
- [x] `pub use crate::audit::{AuditRecord, AuditTrail};`
- [x] `pub use crate::events::AccessRejectedEvent;`
- [x] `pub use crate::helpers::{MerkleVerifier, ZkAccessHelper};`
- [x] `pub use crate::verifier::{Bn254Verifier, PoseidonHasher, Proof, ProofValidationError, ZkVerifier};`
- [x] `pub use crate::vk::{G1Point, G2Point, VerificationKey};`
- [x] `pub use AccessRequest;`
- [x] `pub use BatchAccessAuditEvent;`
- [x] `pub use BatchVerificationSummary;`
- [x] `pub use ContractError;`
- [x] `pub use ZkVerifierContractClient;`

### ✅ Re-exports in verifier.rs
- [x] `pub use crate::vk::{G1Point, G2Point};`
- [x] `pub type VerificationKey = crate::vk::VerificationKey;`

### ✅ Test Imports
#### bench_verify.rs
- [x] `use zk_verifier::{AccessRequest, ZkAccessHelper, ZkVerifierContract, ZkVerifierContractClient};`

#### test_nonce_replay.rs
- [x] `use zk_verifier::verifier::{G1Point, G2Point, Proof};`
- [x] `use zk_verifier::{AccessRequest, ContractError, ZkVerifierContract, ZkVerifierContractClient};`

#### test_zk_access.rs
- [x] `use zk_verifier::vk::{G1Point, G2Point, VerificationKey};`
- [x] `use zk_verifier::{MerkleVerifier, ZkAccessHelper};`
- [x] `use zk_verifier::{AccessRejectedEvent, ContractError, ZkVerifierContract, ZkVerifierContractClient};`
- [x] `zk_verifier::PoseidonHasher::hash(...)` calls work

## Compilation Commands

### 1. Basic Compilation Check
```bash
cargo check -p zk_verifier
```
**Expected**: No errors, all imports resolve

### 2. Test Compilation Check
```bash
cargo check -p zk_verifier --tests
```
**Expected**: All test files compile without errors

### 3. All Targets Check
```bash
cargo check -p zk_verifier --all-targets
```
**Expected**: Library, tests, and benches all compile

### 4. Clippy Linting
```bash
cargo clippy -p zk_verifier --all-targets -- -D warnings
```
**Expected**: No warnings or errors

### 5. Format Check
```bash
cargo fmt --all -- --check
```
**Expected**: All files properly formatted

### 6. Run Tests
```bash
cargo test -p zk_verifier
```
**Expected**: All tests compile and run (may have test failures due to mock verification logic)

### 7. Specific Test Files
```bash
cargo test -p zk_verifier --test bench_verify
cargo test -p zk_verifier --test test_nonce_replay
cargo test -p zk_verifier --test test_zk_access
```
**Expected**: Each test file compiles and runs independently

## Error Resolution Guide

### Error: "unresolved import"
**Cause**: Type not exported from lib.rs
**Fix**: Add `pub use TypeName;` to lib.rs exports section

### Error: "cannot find type in this scope"
**Cause**: Module not public or type not re-exported
**Fix**: Ensure module is `pub mod` and type is in public exports

### Error: "duplicate definitions"
**Cause**: Type defined in multiple modules
**Fix**: Keep single definition, use re-exports elsewhere

### Error: "private type in public interface"
**Cause**: Public function uses private type
**Fix**: Make the type public or change function signature

### Error: "symbol_short not found"
**Cause**: Missing soroban_sdk import
**Fix**: Ensure `use soroban_sdk::symbol_short;` in file

## Success Criteria

All of the following must pass:

- ✅ `cargo check -p zk_verifier` → Success
- ✅ `cargo clippy -p zk_verifier --all-targets -- -D warnings` → No warnings
- ✅ `cargo fmt --all -- --check` → No formatting issues
- ✅ `cargo test -p zk_verifier --no-fail-fast` → All tests compile

## Post-Compilation Verification

### Verify Exports
```bash
# Check that types are accessible
cargo doc -p zk_verifier --no-deps --open
```
Look for:
- AccessRequest in public API
- ContractError in public API
- G1Point, G2Point in vk module
- Proof in verifier module
- ZkVerifierContractClient in public API

### Verify Test Compilation
```bash
# Compile tests with verbose output
cargo test -p zk_verifier --no-run --verbose
```
Should show successful compilation of all test files.

## Related Issues

This checklist addresses:
- Issue #273: zk_verifier test compilation failures
- Issue #271: zk_prover (same root cause)
- Issue #272: zk_voting (same root cause)

## Notes

- All changes are backward compatible
- No breaking changes to existing API
- Tests may have runtime failures due to mock verification logic
- The important part is that everything COMPILES without errors
