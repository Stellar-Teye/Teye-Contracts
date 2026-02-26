# Rust Client SDK Guide

This guide demonstrates how to integrate Stellar Teye contracts into Rust applications using the Soroban SDK.

## Prerequisites

- **Rust**: Version 1.70.0 or higher
- **Soroban CLI**: Version 23.1.4 or higher
- **Target**: `wasm32v1-none` target installed

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
soroban-sdk = "25.0.0"
stellar-xdr = { version = "21.0.0", default-features = false }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[dev-dependencies]
soroban-sdk = { version = "25.0.0", features = ["testutils"] }
```

Install the required target:

```bash
rustup target add wasm32v1-none
```

## Contract-to-Contract Calls

### Basic Contract Integration

```rust
#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, String, BytesN};

// Import the Teye contract interface
soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/teye_contract.wasm");

#[contract]
pub struct HealthcareApp;

#[contractimpl]
impl HealthcareApp {
    /// Register a patient through the Teye contract
    pub fn register_patient(
        env: Env,
        teye_contract_id: Address,
        patient_data: PatientData,
    ) -> u64 {
        let teye_client = Client::new(&env, &teye_contract_id);
        
        teye_client.register_patient(
            &patient_data.public_key,
            &patient_data.name,
            &patient_data.date_of_birth,
            &patient_data.contact_info,
            &patient_data.emergency_contact,
        )
    }

    /// Add a vision record
    pub fn add_vision_record(
        env: Env,
        teye_contract_id: Address,
        record_data: VisionRecordData,
    ) -> u64 {
        let teye_client = Client::new(&env, &teye_contract_id);
        
        teye_client.add_vision_record(
            &record_data.patient_id,
            &record_data.provider_id,
            &record_data.record_type,
            &record_data.encrypted_data_hash,
            &record_data.metadata,
        )
    }

    /// Grant access to patient records
    pub fn grant_access(
        env: Env,
        teye_contract_id: Address,
        patient_id: Address,
        requester_id: Address,
        permissions: Vec<String>,
        duration: Option<u64>,
    ) -> bool {
        let teye_client = Client::new(&env, &teye_contract_id);
        
        let access_request = AccessRequest {
            patient_id,
            requester_id,
            permissions,
            expires_at: duration.map(|d| env.ledger().timestamp() + d),
            granted_at: env.ledger().timestamp(),
        };
        
        teye_client.grant_access(&patient_id, &requester_id, &access_request)
    }
}

// Data structures
#[derive(Clone)]
pub struct PatientData {
    pub public_key: Address,
    pub name: String,
    pub date_of_birth: String,
    pub contact_info: String,
    pub emergency_contact: String,
}

#[derive(Clone)]
pub struct VisionRecordData {
    pub patient_id: Address,
    pub provider_id: Address,
    pub record_type: String,
    pub encrypted_data_hash: BytesN<32>,
    pub metadata: String,
}

#[derive(Clone)]
pub struct AccessRequest {
    pub patient_id: Address,
    pub requester_id: Address,
    pub permissions: Vec<String>,
    pub expires_at: Option<u64>,
    pub granted_at: u64,
}
```

### Advanced Contract Integration

```rust
use soroban_sdk::{contract, contractimpl, Address, Env, String, BytesN, Vec, Map, Symbol};

#[contract]
pub struct AdvancedHealthcareApp;

#[contractimpl]
impl AdvancedHealthcareApp {
    /// Batch patient registration with validation
    pub fn batch_register_patients(
        env: Env,
        teye_contract_id: Address,
        patients: Vec<PatientData>,
    ) -> Vec<u64> {
        let teye_client = Client::new(&env, &teye_contract_id);
        let mut results = Vec::new(&env);
        
        for patient in patients.iter() {
            // Validate patient data
            if self.validate_patient_data(&patient) {
                let result = teye_client.register_patient(
                    &patient.public_key,
                    &patient.name,
                    &patient.date_of_birth,
                    &patient.contact_info,
                    &patient.emergency_contact,
                );
                results.push_back(result);
            } else {
                // Log validation failure
                env.events().publish(
                    Symbol::new(&env, "validation_failed"),
                    patient.public_key.clone(),
                );
            }
        }
        
        results
    }

    /// Get comprehensive patient summary
    pub fn get_patient_summary(
        env: Env,
        teye_contract_id: Address,
        patient_id: Address,
    ) -> PatientSummary {
        let teye_client = Client::new(&env, &teye_contract_id);
        
        // Get patient profile
        let profile = teye_client.get_patient_profile(&patient_id);
        
        // Get all records
        let records = teye_client.get_patient_records(&patient_id);
        
        // Get access permissions
        let access_list = teye_client.get_access_list(&patient_id);
        
        PatientSummary {
            profile,
            records,
            access_list,
            last_updated: env.ledger().timestamp(),
        }
    }

    /// Validate patient data before registration
    fn validate_patient_data(&self, patient: &PatientData) -> bool {
        // Basic validation logic
        !patient.name.is_empty()
            && !patient.date_of_birth.is_empty()
            && patient.contact_info.len() > 5
            && patient.emergency_contact.len() > 5
    }
}

#[derive(Clone)]
pub struct PatientSummary {
    pub profile: PatientProfile,
    pub records: Vec<VisionRecord>,
    pub access_list: Vec<AccessPermission>,
    pub last_updated: u64,
}

#[derive(Clone)]
pub struct PatientProfile {
    pub public_key: Address,
    pub name: String,
    pub date_of_birth: String,
    pub contact_info: String,
    pub emergency_contact: String,
}

#[derive(Clone)]
pub struct VisionRecord {
    pub id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub record_type: String,
    pub encrypted_data_hash: BytesN<32>,
    pub metadata: String,
    pub created_at: u64,
}

#[derive(Clone)]
pub struct AccessPermission {
    pub requester_id: Address,
    pub permissions: Vec<String>,
    pub granted_at: u64,
    pub expires_at: Option<u64>,
}
```

## ZK Prover SDK Integration

### Using the ZK Prover Library

```rust
use soroban_sdk::{Address, Env, BytesN};
use zk_prover::{generate_proof, circuit::AccessWitness};

#[contract]
pub struct ZKHealthcareApp;

#[contractimpl]
impl ZKHealthcareApp {
    /// Grant access with zero-knowledge proof
    pub fn grant_access_with_zk_proof(
        env: Env,
        teye_contract_id: Address,
        patient_id: Address,
        requester_id: Address,
        resource_id: BytesN<32>,
        witness_data: AccessWitness,
    ) -> bool {
        // Generate ZK proof
        let public_inputs = &[&BytesN::from_array(&env, &[1u8; 32])]; // Mock public input
        let access_request = generate_proof(
            &env,
            requester_id.clone(),
            resource_id,
            witness_data,
            public_inputs,
        );

        // Submit to ZK verifier contract
        let zk_verifier_client = zk_verifier::Client::new(&env, &teye_contract_id);
        
        zk_verifier_client.verify_access_request(&access_request)
    }

    /// Batch access verification with ZK proofs
    pub fn batch_verify_access(
        env: Env,
        teye_contract_id: Address,
        access_requests: Vec<AccessRequestData>,
    ) -> Vec<bool> {
        let zk_verifier_client = zk_verifier::Client::new(&env, &teye_contract_id);
        let mut results = Vec::new(&env);
        
        for request in access_requests.iter() {
            let result = zk_verifier_client.verify_access_request(&request.access_request);
            results.push_back(result);
        }
        
        results
    }
}

#[derive(Clone)]
pub struct AccessRequestData {
    pub access_request: zk_verifier::AccessRequest,
    pub witness: AccessWitness,
}
```

### Custom ZK Circuit Integration

```rust
use zk_prover::circuit::{AccessWitness, ZkAccessCircuit};

#[contractimpl]
impl ZKHealthcareApp {
    /// Create custom access proof for specific data types
    pub fn create_custom_access_proof(
        env: Env,
        patient_id: Address,
        data_type: String,
        access_level: u32,
    ) -> zk_verifier::AccessRequest {
        let witness = AccessWitness {
            patient_id,
            data_type,
            access_level,
            timestamp: env.ledger().timestamp(),
        };

        let resource_id = BytesN::from_array(&env, &[2u8; 32]);
        let public_inputs = &[&BytesN::from_array(&env, &[1u8; 32])];
        
        generate_proof(&env, patient_id, resource_id, witness, public_inputs)
    }

    /// Verify medical data access with enhanced security
    pub fn verify_medical_access(
        env: Env,
        teye_contract_id: Address,
        access_request: zk_verifier::AccessRequest,
        required_clearance: u32,
    ) -> bool {
        let zk_verifier_client = zk_verifier::Client::new(&env, &teye_contract_id);
        
        // First verify the ZK proof
        let proof_valid = zk_verifier_client.verify_access_request(&access_request);
        
        if proof_valid {
            // Additional business logic validation
            self.validate_clearance_level(&access_request, required_clearance)
        } else {
            false
        }
    }

    fn validate_clearance_level(
        &self,
        access_request: &zk_verifier::AccessRequest,
        required_clearance: u32,
    ) -> bool {
        // Extract clearance level from the proof (implementation depends on your circuit)
        // This is a placeholder for your actual validation logic
        true
    }
}
```

## Test Harness Integration

### Soroban Test Environment

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, Address, String, BytesN};

    #[test]
    fn test_patient_registration() {
        let env = Env::default();
        let contract_id = Address::random(&env);
        
        // Mock patient data
        let patient_data = PatientData {
            public_key: Address::random(&env),
            name: String::from_str(&env, "John Doe"),
            date_of_birth: String::from_str(&env, "1990-01-01"),
            contact_info: String::from_str(&env, "john@example.com"),
            emergency_contact: String::from_str(&env, "Jane Doe"),
        };

        // Test registration
        let result = HealthcareApp::register_patient(env.clone(), contract_id, patient_data);
        assert!(result > 0);
    }

    #[test]
    fn test_vision_record_creation() {
        let env = Env::default();
        let contract_id = Address::random(&env);
        
        let record_data = VisionRecordData {
            patient_id: Address::random(&env),
            provider_id: Address::random(&env),
            record_type: String::from_str(&env, "exam"),
            encrypted_data_hash: BytesN::from_array(&env, &[1u8; 32]),
            metadata: String::from_str(&env, "Comprehensive eye exam"),
        };

        let result = HealthcareApp::add_vision_record(env.clone(), contract_id, record_data);
        assert!(result > 0);
    }

    #[test]
    fn test_access_grant_and_revoke() {
        let env = Env::default();
        let contract_id = Address::random(&env);
        
        let patient_id = Address::random(&env);
        let requester_id = Address::random(&env);
        let permissions = vec![String::from_str(&env, "read"), String::from_str(&env, "write")];

        // Test access grant
        let grant_result = HealthcareApp::grant_access(
            env.clone(),
            contract_id,
            patient_id.clone(),
            requester_id,
            permissions,
            Some(86400), // 24 hours
        );
        assert!(grant_result);
    }

    #[test]
    fn test_zk_proof_generation() {
        let env = Env::default();
        let contract_id = Address::random(&env);
        
        let witness = AccessWitness {
            patient_id: Address::random(&env),
            data_type: String::from_str(&env, "medical_record"),
            access_level: 2,
            timestamp: env.ledger().timestamp(),
        };

        let result = ZKHealthcareApp::grant_access_with_zk_proof(
            env.clone(),
            contract_id,
            Address::random(&env),
            Address::random(&env),
            BytesN::from_array(&env, &[3u8; 32]),
            witness,
        );
        
        assert!(result);
    }

    #[test]
    fn test_batch_operations() {
        let env = Env::default();
        let contract_id = Address::random(&env);
        
        let mut patients = Vec::new(&env);
        for i in 0..3 {
            patients.push_back(PatientData {
                public_key: Address::random(&env),
                name: String::from_str(&env, &format!("Patient {}", i)),
                date_of_birth: String::from_str(&env, "1990-01-01"),
                contact_info: String::from_str(&env, "patient@example.com"),
                emergency_contact: String::from_str(&env, "Emergency Contact"),
            });
        }

        let results = AdvancedHealthcareApp::batch_register_patients(env.clone(), contract_id, patients);
        assert_eq!(results.len(), 3);
    }
}
```

### Integration Testing with Mock Contracts

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use soroban_sdk::{Env, testutils::{Accounts, LedgerInfo}};

    fn setup_test_env() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        
        // Setup test accounts
        let admin = Address::random(&env);
        let patient = Address::random(&env);
        let provider = Address::random(&env);
        
        // Fund accounts
        env.budget().reset_unlimited();
        
        (env, admin)
    }

    #[test]
    fn test_full_patient_workflow() {
        let (env, admin) = setup_test_env();
        let contract_id = Address::random(&env);
        
        // Step 1: Register patient
        let patient_data = PatientData {
            public_key: Address::random(&env),
            name: String::from_str(&env, "Test Patient"),
            date_of_birth: String::from_str(&env, "1985-05-15"),
            contact_info: String::from_str(&env, "test@patient.com"),
            emergency_contact: String::from_str(&env, "Emergency Contact"),
        };

        let patient_id = HealthcareApp::register_patient(env.clone(), contract_id, patient_data);
        assert!(patient_id > 0);

        // Step 2: Add vision record
        let record_data = VisionRecordData {
            patient_id: Address::random(&env),
            provider_id: Address::random(&env),
            record_type: String::from_str(&env, "prescription"),
            encrypted_data_hash: BytesN::from_array(&env, &[4u8; 32]),
            metadata: String::from_str(&env, "Eye prescription"),
        };

        let record_id = HealthcareApp::add_vision_record(env.clone(), contract_id, record_data);
        assert!(record_id > 0);

        // Step 3: Grant access
        let permissions = vec![String::from_str(&env, "read")];
        let access_granted = HealthcareApp::grant_access(
            env.clone(),
            contract_id,
            Address::random(&env),
            Address::random(&env),
            permissions,
            Some(3600), // 1 hour
        );
        assert!(access_granted);
    }
}
```

## Cross-Contract Invocation Patterns

### Safe Contract Calls

```rust
#[contractimpl]
impl HealthcareApp {
    /// Safe contract call with error handling
    pub fn safe_contract_call(
        env: Env,
        target_contract: Address,
        method: &str,
        args: Vec<soroban_sdk::Val>,
    ) -> Result<soroban_sdk::Val, ContractError> {
        // Validate contract address
        if target_contract.is_none() {
            return Err(ContractError::InvalidAddress);
        }

        // Check if contract exists
        let contract_client = soroban_sdk::ContractClient::new(&env, &target_contract);
        
        // Implement safe call pattern
        match self.try_contract_call(&env, &contract_client, method, args) {
            Ok(result) => Ok(result),
            Err(e) => Err(ContractError::CallFailed(e)),
        }
    }

    fn try_contract_call(
        &self,
        env: &Env,
        client: &soroban_sdk::ContractClient,
        method: &str,
        args: Vec<soroban_sdk::Val>,
    ) -> Result<soroban_sdk::Val, String> {
        // Implementation depends on your specific needs
        // This is a placeholder for safe call logic
        Ok(env.clone().into())
    }
}

#[derive(Clone, Debug)]
pub enum ContractError {
    InvalidAddress,
    CallFailed(String),
    InsufficientPermissions,
    ContractNotFound,
}
```

## Best Practices

1. **Security**: Always validate contract addresses before making calls
2. **Error Handling**: Implement comprehensive error handling for contract calls
3. **Testing**: Write thorough unit and integration tests
4. **Gas Optimization**: Minimize storage operations and optimize data structures
5. **Security Audits**: Regularly audit contract interaction code
6. **Documentation**: Document all contract interfaces and data structures

## References

- [Soroban SDK Documentation](https://soroban.stellar.org/docs/)
- [Rust Example Code](../../../example/rust/)
- [ZK Prover SDK](../../sdk/zk_prover/)
- [Contract API Documentation](../../api/)
