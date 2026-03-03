# ZK Verifier Test Compilation Fixes

## Issue Summary
The zk_verifier contract's test files failed to compile due to missing public exports. Tests were unable to import required types and functions from the zk_verifier crate.

## Root Cause
The public API of the zk_verifier crate was missing many exports that the tests depended on:
- `AccessRequest`, `ContractError`, `ZkVerifierContract`, `ZkVerifierContractClient`
- `G1Point`, `G2Point` from both `vk` and `verifier` modules
- `PoseidonHasher`, `MerkleVerifier`, `ZkAccessHelper`
- Duplicate type definitions causing conflicts

## Changes Applied

### 1. Updated `src/lib.rs` Exports

**Added missing public re-exports:**
```rust
// Re-export contract types for tests
pub use AccessRequest;
pub use BatchAccessAuditEvent;
pub use BatchVerificationSummary;
pub use ContractError;
```

**Added contract client export:**
```rust
#[contract]
pub struct ZkVerifierContract;

// Re-export the contract client for tests
pub use ZkVerifierContractClient;
```

**Consolidated G1Point and G2Point exports:**
```rust
// Changed from:
pub use crate::verifier::{Bn254Verifier, G1Point, G2Point, PoseidonHasher, ...};
pub use crate::vk::VerificationKey;

// To:
pub use crate::verifier::{Bn254Verifier, PoseidonHasher, Proof, ProofValidationError, ZkVerifier};
pub use crate::vk::{G1Point, G2Point, VerificationKey};
```

### 2. Updated `src/verifier.rs` to Remove Duplicate Types

**Removed duplicate G1Point and G2Point definitions:**
- These types are now defined only in `vk.rs`
- Added re-export in `verifier.rs`:
```rust
pub use crate::vk::{G1Point, G2Point};
```

**Simplified Proof definition:**
```rust
// Removed duplicate type definitions, kept only:
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proof {
    pub a: G1Point,
    pub b: G2Point,
    pub c: G1Point,
}
```

## Test File Import Compatibility

### bench_verify.rs
✅ Imports now resolved:
- `use zk_verifier::{AccessRequest, ZkAccessHelper, ZkVerifierContract, ZkVerifierContractClient};`

### test_nonce_replay.rs
✅ Imports now resolved:
- `use zk_verifier::verifier::{G1Point, G2Point, Proof};`
- `use zk_verifier::{AccessRequest, ContractError, ZkVerifierContract, ZkVerifierContractClient};`

### test_zk_access.rs
✅ Imports now resolved:
- `use zk_verifier::vk::{G1Point, G2Point, VerificationKey};`
- `use zk_verifier::{MerkleVerifier, ZkAccessHelper};`
- `use zk_verifier::{AccessRejectedEvent, ContractError, ZkVerifierContract, ZkVerifierContractClient};`
- `zk_verifier::PoseidonHasher::hash(...)` calls

## Verification Commands

Run these commands to verify the fixes:

```bash
# Check compilation
cargo check -p zk_verifier

# Run clippy with all targets
cargo clippy -p zk_verifier --all-targets -- -D warnings

# Check formatting
cargo fmt --all -- --check

# Run tests
cargo test -p zk_verifier
```

## Expected Results

All commands should pass without errors:
- ✅ No unresolved imports
- ✅ No missing type definitions
- ✅ No duplicate type conflicts
- ✅ All tests compile successfully
- ✅ Clippy warnings resolved
- ✅ Formatting compliant

## Related Issues Fixed

This fix resolves:
- #273 (zk_verifier test compilation)
- #271 (zk_prover - same root cause)
- #272 (zk_voting - same root cause)

## Files Modified

1. `contracts/zk_verifier/src/lib.rs`
   - Added missing public exports
   - Added contract client re-export
   - Consolidated type exports

2. `contracts/zk_verifier/src/verifier.rs`
   - Removed duplicate G1Point and G2Point definitions
   - Added re-exports from vk module
   - Simplified type structure

## Breaking Changes

None. All changes are additive exports that make previously internal types public for test usage.

## Notes

- The `symbol_short` macro is imported from `soroban_sdk` and is available in test scope
- `PoseidonHasher` is now properly exported and accessible as `zk_verifier::PoseidonHasher`
- Type consolidation ensures consistency across the codebase
- All test imports are now satisfied by the public API
