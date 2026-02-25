# Post-Quantum ZK Migration Research

As quantum computing capabilities advance, the security of traditional elliptic-curve cryptography (like the BN254 alt-bn128 curve currently used by the `zk_verifier` Soroban smart contract) will be threatened by Shor's algorithm. This document analyzes post-quantum zero-knowledge (ZK) alternatives and outlines a strategy for migrating the Stellar-Teye ecosystem to quantum-resistant proofs.

## Comparison of ZK Technologies

| Feature | Pre-Quantum SNARK (BN254) | STARKs (e.g. FRI-based) | Post-Quantum SNARKs | Lattice-based ZK |
| :--- | :--- | :--- | :--- | :--- |
| **Security Assumption** | Elliptic Curve pairings / DLOG (Vulnerable to Quantum) | Collision-resistant Hash Functions (Quantum Resistant) | Hash Functions or Lattices | Learning With Errors (LWE) / SIS (Quantum Resistant) |
| **Trusted Setup** | Required (Toxic waste problem) | Transparent (No setup required) | Varies (Some transparent) | Varies |
| **Proof Size** | Very Small (~200-300 bytes) | Large (10-100+ KB) | Medium-Large | Medium (Can be optimized) |
| **Prover Time** | Moderate to Fast | Very Fast | Slower (currently) | Fast (Lattice operations are efficient linear algebra) |
| **Verification Cost** | Constant / Low (Pairing operations dominate) | Poly-logarithmic (Scales well, but base cost of multi-hashing is higher than SNARKs) | Variable | Low-Medium (Depends on protocol) |
| **Soroban Feasibility** | **Excellent:** Supported currently, very low instruction budget footprint. | **Challenging:** Verifying large hash paths across 100KB proofs exceeds the current 100KB cross-contract call payload limit and may breach the CPU instruction budget without native host precompiles (e.g., `keccak256` or `poseidon`). | **Developing:** Wasm efficiency is improving, but proof sizes and compute cost still push limits. | **Promising but Immature:** Very fast operations but standards and tooling (like Circom) are not widely adopted yet for general-purpose circuits. |

## Recommended Migration Strategy

A sudden transition to a post-quantum proof system is risky due to differing proof sizes breaking `AccessRequest` structs and increased Soroban execution budgets. The following phased migration path is recommended:

### Phase 1: Research & Dual-Protocol Support
1. **Host-Function Advocacy**: Campaign for the Stellar network to introduce native precompiles for STARK-friendly hashes (e.g. Blake3) or FRI verification directly, minimizing CPU budget constraints for larger proofs.
2. **Dual-Verifier Implementation**: Update the `ZkVerifierContract` to support *both* BN254 SNARKs and a selected PQ alternative (likely a STARK or a lattice-based SNARK). The `AccessRequest` structure will migrate from hardcoded `Proof` structs to a generic `enum ProofType { Groth16(BN254Proof), STARK(StarkProof) }`.

### Phase 2: Dual Verification & Gradual Upgrade
3. **SDK Update**: The `sdk/zk_prover` will be updated to allow dApps to generate the new proof types.
4. **Soft-Deprecation**: Emit events warning legacy indexers and off-chain clients that BN254 verification is deprecated. Encourage ecosystem participants to upgrade circuit toolchains.
5. **Circuit Upgrade**: Ensure all `identity`, `zk_voting`, and `vision_records` circuit logic is ported (from Circom to Cairo/Winterfell for STARKs, for instance).

### Phase 3: Final Quantum-Safe Enclavement
6. **Hard-Deprecation**: Disallow `ProofType::Groth16` in the `ZkVerifierContract` entirely.
7. **Cleanup**: Remove legacy BN254 parsing and validation logic from the contract to save Wasm space and minimize attack surface.

## Action Items
To prepare the current BN254-based codebase for this migration, `// TODO: post-quantum migration` annotations have been added to:
- `contracts/zk_verifier/src/lib.rs`
- `contracts/zk_verifier/src/verifier.rs`
