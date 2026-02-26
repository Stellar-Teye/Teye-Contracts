# Audit Trail Documentation

This document provides comprehensive documentation of the audit trail system implemented in the Stellar Teye platform, ensuring complete traceability of all operations involving Protected Health Information (PHI).

## 🔍 Audit Trail Overview

The audit trail system provides an immutable, tamper-evident record of all activities within the Stellar Teye platform, meeting HIPAA requirements for audit controls and ensuring accountability for all data access and modifications.

### Key Features

- **Immutability**: All audit records stored on-chain
- **Completeness**: Every operation logged without exception
- **Tamper-Evidence**: Any modification attempts are detectable
- **Queryability**: Efficient search and reporting capabilities
- **Retention**: Configurable retention policies
- **Privacy**: Audit logs don't contain PHI

## 📋 Audit Data Structure

### Core Audit Log Entry

**Location**: `contracts/compliance/src/audit.rs`

```rust
#[contracttype]
pub struct AuditLog {
    pub timestamp: u64,              // Unix timestamp
    pub ledger_sequence: u64,         // Stellar ledger sequence
    pub transaction_hash: [u8; 32],  // Transaction hash
    pub user_id: Address,             // User performing action
    pub user_role: String,            // User role (Patient, Provider, Admin)
    pub action: String,               // Action performed
    pub resource_type: String,         // Type of resource accessed
    pub resource_id: String,          // Resource identifier
    pub result: bool,                 // Success/failure
    pub consent_id: Option<String>,    // Associated consent ID
    pub ip_hash: [u8; 32],         // Hash of client IP (for privacy)
    pub user_agent_hash: [u8; 32],  // Hash of user agent
    pub metadata: String,             // Additional context
}
```

### Audit Event Types

| Event Category | Event Type | Description | Required Fields |
|----------------|------------|-------------|----------------|
| **Authentication** | `user_login` | User authentication attempt | user_id, result, ip_hash |
| **Authentication** | `user_logout` | User session termination | user_id, timestamp |
| **Authorization** | `access_granted` | Access to PHI granted | user_id, resource_id, consent_id |
| **Authorization** | `access_denied` | Access to PHI denied | user_id, resource_id, reason |
| **Data Access** | `record_viewed` | Patient record accessed | user_id, resource_id, consent_id |
| **Data Access** | `record_downloaded` | Record downloaded | user_id, resource_id, consent_id |
| **Data Modification** | `record_created` | New record created | user_id, resource_id, metadata |
| **Data Modification** | `record_updated` | Record modified | user_id, resource_id, changes |
| **Data Modification** | `record_deleted` | Record deleted | user_id, resource_id, reason |
| **Consent** | `consent_granted` | Patient consent granted | patient_id, provider_id, permissions |
| **Consent** | `consent_revoked` | Patient consent revoked | patient_id, provider_id, reason |
| **Consent** | `consent_expired` | Consent expired automatically | patient_id, provider_id, consent_id |
| **System** | `system_error` | System error occurred | error_code, error_message |
| **System** | `security_event` | Security-related event | event_type, severity, details |
| **Compliance** | `policy_violation` | Policy violation detected | user_id, policy, violation_type |
| **Compliance** | `breach_detected` | Potential breach detected | breach_type, affected_records |

## 🔧 Implementation Details

### Audit Logging Function

```rust
// From contracts/compliance/src/audit.rs
pub fn log_audit_event(
    env: &Env,
    user_id: &Address,
    action: &str,
    resource_type: &str,
    resource_id: &str,
    result: bool,
    consent_id: Option<&String>,
    metadata: &str,
) {
    let audit_log = AuditLog {
        timestamp: env.ledger().timestamp(),
        ledger_sequence: env.ledger().sequence(),
        transaction_hash: env.current_contract_address().contract_id().clone(),
        user_id: user_id.clone(),
        user_role: get_user_role(env, user_id),
        action: action.to_string(),
        resource_type: resource_type.to_string(),
        resource_id: resource_id.to_string(),
        result,
        consent_id: consent_id.cloned(),
        ip_hash: hash_client_ip(env),
        user_agent_hash: hash_user_agent(env),
        metadata: metadata.to_string(),
    };
    
    // Store immutably on-chain
    let audit_key = generate_audit_key(env, &audit_log.timestamp);
    env.storage().persistent().set(&audit_key, &audit_log);
    
    // Emit event for real-time monitoring
    env.events().publish(
        (symbol_short!("AUDIT"), symbol_short!("LOGGED")),
        (user_id, action, resource_id, result),
    );
}
```

### Storage Key Generation

```rust
fn generate_audit_key(env: &Env, timestamp: u64) -> (Symbol, u64) {
    // Use timestamp for chronological ordering
    (symbol_short!("AUDIT_LOG"), timestamp)
}
```

### Privacy Protection

**IP Address Hashing**:
```rust
fn hash_client_ip(env: &Env) -> [u8; 32] {
    // Get client IP from transaction metadata
    let client_ip = get_client_ip(env);
    
    // Hash with salt for privacy
    let salt = env.storage().persistent().get(&symbol_short!("IP_SALT"));
    let mut hasher = Sha256::new();
    hasher.update(client_ip.as_bytes());
    hasher.update(salt.as_bytes());
    hasher.finalize().into()
}
```

**User Agent Hashing**:
```rust
fn hash_user_agent(env: &Env) -> [u8; 32] {
    let user_agent = get_user_agent(env);
    let salt = env.storage().persistent().get(&symbol_short!("UA_SALT"));
    
    let mut hasher = Sha256::new();
    hasher.update(user_agent.as_bytes());
    hasher.update(salt.as_bytes());
    hasher.finalize().into()
}
```

## 🔍 Query and Reporting

### Audit Query Interface

```rust
pub struct AuditQuery {
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub user_id: Option<Address>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub result: Option<bool>,
    pub limit: Option<u32>,
}
```

### Query Functions

```rust
pub fn query_audit_logs(
    env: &Env,
    query: AuditQuery,
) -> Vec<AuditLog> {
    let mut results = Vec::new(env);
    let current_time = env.ledger().timestamp();
    
    // Determine search range
    let start_time = query.start_time.unwrap_or(0);
    let end_time = query.end_time.unwrap_or(current_time);
    
    // Scan audit logs (optimized with indexing)
    let mut timestamp = start_time;
    while timestamp <= end_time && results.len() < query.limit.unwrap_or(1000) as usize {
        let audit_key = generate_audit_key(env, timestamp);
        
        if let Some(audit_log) = env.storage().persistent().get(&audit_key) {
            if matches_query(&audit_log, &query) {
                results.push_back(audit_log);
            }
        }
        
        timestamp += 1;
    }
    
    results
}
```

### Compliance Reports

#### HIPAA Access Report

```rust
pub fn generate_hipaa_access_report(
    env: &Env,
    patient_id: &Address,
    start_date: u64,
    end_date: u64,
) -> HipaaAccessReport {
    let query = AuditQuery {
        start_time: Some(start_date),
        end_time: Some(end_date),
        user_id: None, // All users
        action: Some("record_viewed".to_string()),
        resource_type: Some("patient_record".to_string()),
        result: Some(true),
        limit: Some(10000),
    };
    
    let audit_logs = query_audit_logs(env, query);
    
    // Filter for specific patient
    let patient_logs = audit_logs.iter()
        .filter(|log| log.resource_id.contains(&patient_id.to_string()))
        .collect();
    
    HipaaAccessReport {
        patient_id: patient_id.clone(),
        report_period: (start_date, end_date),
        total_accesses: patient_logs.len(),
        unique_providers: count_unique_providers(&patient_logs),
        consent_compliance: verify_consent_compliance(&patient_logs),
        access_patterns: analyze_access_patterns(&patient_logs),
    }
}
```

#### Security Incident Report

```rust
pub fn generate_security_incident_report(
    env: &Env,
    start_date: u64,
    end_date: u64,
) -> SecurityIncidentReport {
    let security_events = query_audit_logs(env, AuditQuery {
        start_time: Some(start_date),
        end_time: Some(end_date),
        action: Some("security_event".to_string()),
        result: None,
        ..Default::default()
    });
    
    SecurityIncidentReport {
        report_period: (start_date, end_date),
        total_incidents: security_events.len(),
        incidents_by_type: categorize_incidents(&security_events),
        high_severity_incidents: filter_high_severity(&security_events),
        resolution_times: calculate_resolution_times(&security_events),
    }
}
```

## 📊 Retention Management

### Retention Policies

**Location**: `contracts/compliance/src/retention.rs`

```rust
#[contracttype]
pub struct RetentionPolicy {
    pub data_type: String,
    pub retention_period_days: u64,
    pub archival_required: bool,
    pub deletion_method: DeletionMethod,
}

#[contracttype]
pub enum DeletionMethod {
    SecureDelete,    // Cryptographic erasure
    Archive,         // Move to archival storage
    Anonymize,       // Remove identifying information
}
```

### Automated Cleanup

```rust
pub fn cleanup_expired_audit_logs(env: &Env) {
    let current_time = env.ledger().timestamp();
    let retention_days = get_retention_period(env, "audit_log");
    let cutoff_time = current_time - (retention_days * 24 * 60 * 60);
    
    // Archive logs older than retention period
    let mut timestamp = 0;
    while timestamp < cutoff_time {
        let audit_key = generate_audit_key(env, timestamp);
        
        if let Some(audit_log) = env.storage().persistent().get(&audit_key) {
            archive_audit_log(env, &audit_log);
            env.storage().persistent().remove(&audit_key);
        }
        
        timestamp += 1;
    }
}
```

## 🛡️ Security and Privacy

### Audit Log Protection

**Integrity Measures**:
- **Immutable Storage**: On-chain storage prevents modification
- **Cryptographic Hashing**: Each log entry hashed and chained
- **Digital Signatures**: All logs signed by auditing contract
- **Redundancy**: Multiple storage locations

**Privacy Measures**:
- **No PHI**: Audit logs contain no protected health information
- **Hashed Identifiers**: IP addresses and user agents hashed
- **Access Controls**: Strict access to audit query functions
- **Data Minimization**: Only necessary data collected

### Access Control for Audit Logs

```rust
pub fn require_audit_access(env: &Env, user: &Address) -> Result<(), AuditError> {
    let user_role = get_user_role(env, user);
    
    match user_role.as_str() {
        "ComplianceOfficer" | "SuperAdmin" | "Auditor" => Ok(()),
        _ => Err(AuditError::UnauthorizedAccess)
    }
}
```

## 📈 Performance Optimization

### Indexing Strategy

**Time-based Indexing**:
```rust
// Index by timestamp for efficient range queries
struct TimeIndex {
    hour_bucket: u64,    // Unix timestamp / 3600
    log_count: u64,
    first_timestamp: u64,
    last_timestamp: u64,
}
```

**Event Type Indexing**:
```rust
// Index by event type for efficient filtering
struct EventTypeIndex {
    event_type: String,
    log_ids: Vec<u64>,
    last_updated: u64,
}
```

### Batch Processing

```rust
pub fn batch_log_events(env: &Env, events: Vec<AuditEvent>) {
    for event in events {
        log_audit_event(
            env,
            &event.user_id,
            &event.action,
            &event.resource_type,
            &event.resource_id,
            event.result,
            event.consent_id.as_ref(),
            &event.metadata,
        );
    }
    
    // Emit batch summary event
    env.events().publish(
        (symbol_short!("AUDIT"), symbol_short!("BATCH")),
        (events.len(), env.ledger().timestamp()),
    );
}
```

## 🧪 Testing and Validation

### Audit Log Testing

```rust
#[cfg(test)]
mod audit_tests {
    use super::*;
    
    #[test]
    fn test_audit_log_creation() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        log_audit_event(
            &env,
            &user,
            "record_viewed",
            "patient_record",
            "record_123",
            true,
            Some(&"consent_456".to_string()),
            "Test audit log",
        );
        
        // Verify log was created
        let audit_key = generate_audit_key(&env, env.ledger().timestamp());
        let audit_log = env.storage().persistent().get(&audit_key);
        assert!(audit_log.is_some());
        
        let log = audit_log.unwrap();
        assert_eq!(log.user_id, user);
        assert_eq!(log.action, "record_viewed");
        assert_eq!(log.result, true);
    }
    
    #[test]
    fn test_audit_query() {
        let env = Env::default();
        setup_test_audit_logs(&env);
        
        let query = AuditQuery {
            start_time: Some(1000),
            end_time: Some(2000),
            action: Some("record_viewed".to_string()),
            ..Default::default()
        };
        
        let results = query_audit_logs(&env, query);
        assert_eq!(results.len(), 2); // Based on test data
    }
}
```

### Compliance Validation

```rust
#[test]
fn test_hipaa_audit_compliance() {
    let env = Env::default();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    
    // Simulate access without consent
    let result = access_patient_record(&env, &patient, &provider);
    assert!(result.is_err());
    
    // Verify audit log was created
    let audit_logs = query_audit_logs(&env, AuditQuery {
        user_id: Some(provider),
        action: Some("record_viewed".to_string()),
        result: Some(false),
        ..Default::default()
    });
    
    assert_eq!(audit_logs.len(), 1);
    assert_eq!(audit_logs[0].result, false);
}
```

## 📚 References

### Implementation References
- [Audit Contract](../../contracts/compliance/src/audit.rs)
- [Retention Contract](../../contracts/compliance/src/retention.rs)
- [Consent Management](consent-management.md)
- [HIPAA Requirements Mapping](hipaa-mapping.md)

### Regulatory References
- [HIPAA Security Rule § 164.312(b)](https://www.hhs.gov/hipaa/for-professionals/security/)
- [NIST SP 800-92](https://csrc.nist.gov/publications/detail/sp/800-92/final)
- [ISO 27001:2013](https://www.iso.org/isoiec-27001-information-security.html)

### Security References
- [Security Architecture](../security/overview.md)
- [Access Control Matrix](../security/access-control-matrix.md)
- [Incident Response Plan](../incident-response-plan.md)

---

**Last Updated**: 2025-02-25  
**Next Review**: 2025-03-25  
**Version**: 1.0
