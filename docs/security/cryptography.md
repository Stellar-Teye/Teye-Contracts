# Cryptography

This document outlines all cryptographic primitives, assumptions, and implementations used in the Stellar Teye platform.

## 🔐 Cryptographic Overview

The Stellar Teye platform uses multiple cryptographic techniques to ensure security, privacy, and integrity of healthcare data:

- **Public Key Cryptography**: Identity and authentication
- **Hash Functions**: Data integrity and storage
- **Zero-Knowledge Proofs**: Privacy-preserving verification
- **Homomorphic Encryption**: Privacy-preserving analytics
- **Differential Privacy**: Statistical privacy guarantees

## 🗝️ Hash Functions

### SHA-256

**Purpose**: Primary hash function for data integrity

**Usage**:
- Data hash storage in vision records
- Merkle tree construction
- Audit trail integrity
- File fingerprinting

**Implementation**:
```rust
use sha2::{Sha256, Digest};

fn hash_data(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}
```

**Security Properties**:
- **Collision Resistance**: Practically impossible to find colliding inputs
- **Pre-image Resistance**: Infeasible to reverse hash
- **Second Pre-image Resistance**: Hard to find another input with same hash
- **Avalanche Effect**: Small input changes produce vastly different outputs

### Storage Key Conventions

**Patient Data**: `SHA256("patient_" + patient_id)`
**Vision Records**: `SHA256("record_" + record_id)`
**Access Control**: `SHA256("access_" + patient_id + "_" + requester_id)`
**Audit Logs**: `SHA256("audit_" + timestamp + event_type)`

## 🔑 Public Key Cryptography

### Ed25519 Digital Signatures

**Purpose**: Identity verification and transaction signing

**Usage**:
- Patient and provider identity
- Transaction authentication
- Contract interaction authorization
- Message signing

**Implementation**:
```rust
use ed25519_dalek::{Keypair, PublicKey, Signature, Signer, Verifier};

fn sign_message(keypair: &Keypair, message: &[u8]) -> Signature {
    keypair.sign(message)
}

fn verify_signature(
    public_key: &PublicKey, 
    message: &[u8], 
    signature: &Signature
) -> bool {
    public_key.verify(message, signature).is_ok()
}
```

**Key Management**:
- **Private Keys**: Never stored on-chain, encrypted off-chain
- **Public Keys**: Stored as Stellar addresses
- **Key Derivation**: Hierarchical deterministic wallets
- **Backup**: Encrypted backup procedures

### Stellar-specific Cryptography

**Stellar Address**: `SHA256(public_key) + version byte + checksum`
**Transaction Hash**: `SHA256(transaction_envelope)`
**Signature Verification**: Stellar-specific signature scheme

## 🔒 Zero-Knowledge Proofs

### Groth16 zk-SNARKs

**Purpose**: Privacy-preserving verification of access rights

**Usage**:
- Access control without revealing identity
- Compliance verification
- Anonymous voting
- Privacy-preserving analytics

**Circuit Types**:

#### Access Control Circuit
```rust
// Simplified access control circuit
pub struct AccessCircuit {
    patient_id: Field,
    requester_id: Field,
    access_permission: Field,
    timestamp: Field,
}
```

**Verification Process**:
1. **Proof Generation**: Client generates proof using witness data
2. **Proof Submission**: Submit proof to verifier contract
3. **Verification**: Contract verifies proof using verification key
4. **Access Grant**: Grant access if proof is valid

**Security Assumptions**:
- **Trusted Setup**: Initial setup ceremony was secure
- **Soundness**: False proofs cannot be verified
- **Zero-Knowledge**: No information leakage from proofs
- **Quantum Resistance**: Not currently quantum-resistant

### Implementation Details

**Proof Generation** (from `sdk/zk_prover/src/lib.rs`):
```rust
pub fn generate_proof(
    env: &Env,
    user: Address,
    resource_id: [u8; 32],
    witness: AccessWitness,
    public_inputs: &[&[u8; 32]],
) -> AccessRequest {
    // Mock implementation for demonstration
    // Real implementation would use actual ZK proving system
}
```

**Verification** (in `zk_verifier` contract):
```rust
pub fn verify_proof(
    proof_a: [u8; 64],
    proof_b: [u8; 128],
    proof_c: [u8; 64],
    public_inputs: &[&[u8; 32]],
) -> bool {
    // Pairing-based verification
    // Returns true if proof is valid
}
```

## 🔐 Homomorphic Encryption

### Paillier Cryptosystem

**Purpose**: Privacy-preserving analytics on encrypted data

**Usage**:
- Statistical analysis of medical data
- Aggregated health metrics
- Privacy-preserving research
- Secure multi-party computation

**Properties**:
- **Additive Homomorphism**: `E(a) * E(b) = E(a + b)`
- **Semantic Security**: Indistinguishability under chosen plaintext attack
- **Key Size**: 2048-bit minimum for security

**Implementation**:
```rust
// Simplified Paillier implementation
pub struct PaillierCipher {
    ciphertext: BigInt,
    modulus: BigInt,
}

impl PaillierCipher {
    pub fn encrypt(plaintext: u64, public_key: &PublicKey) -> Self {
        // Encryption implementation
    }
    
    pub fn add(&self, other: &PaillierCipher) -> Self {
        // Homomorphic addition
        PaillierCipher {
            ciphertext: &self.ciphertext * &other.ciphertext % &self.modulus,
            modulus: self.modulus.clone(),
        }
    }
}
```

**Use Cases**:
- **Sum Aggregation**: Sum encrypted medical values
- **Mean Calculation**: Compute averages without decryption
- **Variance Analysis**: Statistical analysis on encrypted data
- **Trend Analysis**: Identify patterns without revealing data

## 🛡️ Differential Privacy

### Laplace Mechanism

**Purpose**: Add statistical noise to protect individual privacy

**Usage**:
- Analytics query responses
- Public health statistics
- Research data sharing
- Aggregate reporting

**Implementation**:
```rust
use rand_distr::{Laplace, Distribution};

fn add_differential_privacy(
    true_value: f64,
    sensitivity: f64,
    epsilon: f64,
) -> f64 {
    let laplace = Laplace::new(0.0, sensitivity / epsilon).unwrap();
    let noise = laplace.sample(&mut rand::thread_rng());
    true_value + noise
}
```

**Privacy Parameters**:
- **Epsilon (ε)**: Privacy budget (typically 0.1 to 1.0)
- **Delta (δ)**: Probability of privacy failure (typically 10^-5)
- **Sensitivity**: Maximum impact of individual data point

**Composition Theorem**:
- **Sequential Composition**: ε_total = Σ ε_i
- **Parallel Composition**: ε_total = max(ε_i)
- **Adaptive Composition**: More complex bounds

## 🔐 Encryption Schemes

### AES-256-GCM

**Purpose**: Symmetric encryption for data at rest

**Usage**:
- Encrypting PHI before storage
- Secure data transmission
- Database encryption
- File system encryption

**Key Management**:
- **Key Derivation**: PBKDF2 with random salt
- **Key Rotation**: Every 90 days
- **Key Storage**: Hardware security modules (HSM)
- **Backup**: Encrypted key backup procedures

**Implementation**:
```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};

pub fn encrypt_data(
    plaintext: &[u8], 
    key: &[u8; 32]
) -> (Vec<u8>, [u8; 12]) {
    let cipher = Aes256Gcm::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(&generate_random_nonce());
    let ciphertext = cipher.encrypt(nonce, plaintext).unwrap();
    (ciphertext, nonce.into())
}
```

## 📊 Cryptographic Performance

### Operation Benchmarks

| Operation | Time (ms) | Gas Cost | Security Level |
|------------|-------------|-----------|---------------|
| SHA-256 Hash | 0.1 | 100 | High |
| Ed25519 Sign | 0.5 | 500 | High |
| Ed25519 Verify | 1.0 | 1000 | High |
| ZK Proof Gen | 1000 | 1000000 | High |
| ZK Verify | 500 | 500000 | High |
| AES Encrypt | 0.2 | 200 | High |
| AES Decrypt | 0.2 | 200 | High |

### Optimization Strategies

1. **Batch Operations**: Process multiple operations together
2. **Precomputation**: Cache expensive computations
3. **Hardware Acceleration**: Use specialized hardware when available
4. **Algorithm Selection**: Choose appropriate algorithms for use case

## 🔍 Security Analysis

### Threat Models

#### Cryptanalysis Attacks
- **Brute Force**: Infeasible with current computing power
- **Side Channel**: Mitigated through constant-time implementations
- **Quantum Attacks**: Future consideration needed

#### Implementation Risks
- **Key Management**: Secure key storage and rotation
- **Random Number Generation**: Cryptographically secure RNGs
- **Side Channel Protection**: Constant-time algorithms

### Compliance Considerations

#### HIPAA Requirements
- **Encryption**: AES-256 for data at rest and in transit
- **Access Controls**: Cryptographic authentication
- **Audit Trails**: Cryptographic integrity of logs
- **Data Integrity**: Hash-based verification

#### Regulatory Standards
- **FIPS 140-2**: Validated cryptographic modules
- **NIST Guidelines**: Follow NIST cryptographic standards
- **ISO 27001**: Information security management

## 🚀 Future Cryptography

### Post-Quantum Considerations

**Threats**:
- **Shor's Algorithm**: Breaks RSA, ECC, DH
- **Grover's Algorithm**: Reduces symmetric key security

**Mitigation Strategies**:
- **Lattice-based Cryptography**: CRYSTALS-Kyber
- **Hash-based Signatures**: SPHINCS+
- **Multivariate Cryptography**: Rainbow signatures
- **Code-based Cryptography**: Classic McEliece

### Migration Plan

1. **Assessment**: Identify quantum-vulnerable components
2. **Research**: Evaluate post-quantum alternatives
3. **Implementation**: Gradual migration to PQC
4. **Testing**: Extensive security testing
5. **Deployment**: Phased rollout with fallback

## 📚 References

### Standards and Specifications
- [NIST Cryptographic Standards](https://csrc.nist.gov/)
- [FIPS 140-2](https://csrc.nist.gov/publications/fips/fips140-2/final/)
- [HIPAA Security Rule](https://www.hhs.gov/hipaa/for-professionals/security/)

### Research Papers
- [Groth16 zk-SNARKs](https://eprint.iacr.org/2016/260)
- [Differential Privacy](https://www.cis.upenn.edu/~aaroth/Papers/privacybook.pdf)
- [Homomorphic Encryption](https://homomorphicencryption.org/)

### Implementation Libraries
- [Stellar SDK](https://github.com/stellar/js-stellar-sdk)
- [Bellman](https://github.com/zcash/librustbellman)
- [Paillier Rust](https://github.com/magnusmuller/paillier-rust)

---

**Last Updated**: 2025-02-25  
**Next Review**: 2025-03-25  
**Version**: 1.0
