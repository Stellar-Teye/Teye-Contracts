# Consent Management Guide

This document provides comprehensive guidance on the patient consent lifecycle implemented in the Stellar Teye platform, ensuring HIPAA compliance and patient control over health information.

## 🎯 Consent Overview

The Stellar Teye platform implements a robust consent management system that gives patients granular control over who can access their health information, for what purposes, and for how long. All consent operations are immutably recorded on the blockchain.

### Core Principles

1. **Patient Autonomy** - Patients have ultimate control over their data
2. **Granular Consent** - Specific permissions for different data types and purposes
3. **Time-Bound Access** - Consent expires automatically unless renewed
4. **Revocability** - Patients can revoke consent at any time
5. **Auditability** - All consent actions are logged and traceable

## 📋 Consent Data Structure

### Consent Record

**Location**: `contracts/common/src/consent.rs`

```rust
#[contracttype]
pub struct Consent {
    pub consent_id: String,           // Unique consent identifier
    pub patient_id: Address,          // Patient granting consent
    pub requester_id: Address,        // Provider/requester receiving consent
    pub permissions: Vec<String>,     // Specific permissions granted
    pub data_types: Vec<String>,     // Types of data accessible
    pub purpose: String,             // Purpose of data access
    pub granted_at: u64,            // When consent was granted
    pub expires_at: Option<u64>,     // When consent expires (None = indefinite)
    pub revoked_at: Option<u64>,     // When consent was revoked (if applicable)
    pub conditions: Vec<String>,      // Special conditions or limitations
    pub emergency_override: bool,     // Whether emergency override is permitted
    pub metadata: String,            // Additional context
}
```

### Permission Types

| Permission | Description | Use Case |
|------------|-------------|-----------|
| `read_basic` | Read basic patient information | Registration, scheduling |
| `read_medical` | Read medical history and records | Treatment, consultation |
| `read_prescriptions` | Read prescription information | Pharmacy, treatment |
| `write_medical` | Add/update medical records | Provider documentation |
| `share_research` | Share for research purposes | Research studies |
| `emergency_access` | Emergency access without consent | Emergency situations |
| `billing_access` | Access for billing purposes | Insurance, billing |

### Data Types

| Data Type | Description | Sensitivity Level |
|------------|-------------|------------------|
| `demographics` | Name, DOB, contact info | Medium |
| `medical_history` | Diagnosis, treatments, procedures | High |
| `prescriptions` | Medication information | High |
| `test_results` | Lab results, imaging | High |
| `insurance` | Insurance and billing info | Medium |
| `emergency_contacts` | Emergency contact information | Medium |

## 🔄 Consent Lifecycle

### 1. Consent Grant

**Process Flow**:
```
Patient Login → Select Provider → Specify Permissions → Set Duration → Review Terms → Confirm Consent → Blockchain Record → Provider Notification
```

**Implementation**:
```rust
pub fn grant_consent(
    env: &Env,
    patient_id: &Address,
    requester_id: &Address,
    permissions: Vec<String>,
    data_types: Vec<String>,
    purpose: String,
    duration_days: Option<u64>,
    conditions: Vec<String>,
) -> Result<String, ConsentError> {
    // Verify patient identity
    require_patient_authentication(env, patient_id)?;
    
    // Validate requester
    let requester = validate_provider(env, requester_id)?;
    
    // Validate permissions and data types
    validate_permissions(&permissions)?;
    validate_data_types(&data_types)?;
    
    // Create consent record
    let consent_id = generate_consent_id(env, patient_id, requester_id);
    let expires_at = duration_days.map(|d| env.ledger().timestamp() + (d * 24 * 60 * 60));
    
    let consent = Consent {
        consent_id: consent_id.clone(),
        patient_id: patient_id.clone(),
        requester_id: requester_id.clone(),
        permissions,
        data_types,
        purpose,
        granted_at: env.ledger().timestamp(),
        expires_at,
        revoked_at: None,
        conditions,
        emergency_override: false,
        metadata: "".to_string(),
    };
    
    // Store on blockchain
    let consent_key = generate_consent_key(env, &consent_id);
    env.storage().persistent().set(&consent_key, &consent);
    
    // Update patient consent index
    update_patient_consent_index(env, patient_id, &consent_id);
    
    // Log consent grant
    log_consent_event(env, patient_id, requester_id, "consent_granted", &consent_id);
    
    // Emit event
    env.events().publish(
        (symbol_short!("CONSENT"), symbol_short!("GRANTED")),
        (patient_id, requester_id, consent_id),
    );
    
    Ok(consent_id)
}
```

### 2. Consent Verification

**Access Control Check**:
```rust
pub fn verify_consent(
    env: &Env,
    patient_id: &Address,
    requester_id: &Address,
    required_permission: &str,
    data_type: &str,
) -> Result<ConsentVerification, ConsentError> {
    // Get active consents
    let active_consents = get_active_consents(env, patient_id, requester_id);
    
    for consent in active_consents {
        // Check if consent covers required permission
        if !consent.permissions.contains(&required_permission.to_string()) {
            continue;
        }
        
        // Check if consent covers data type
        if !consent.data_types.contains(&data_type.to_string()) {
            continue;
        }
        
        // Check if consent has expired
        if let Some(expires_at) = consent.expires_at {
            if env.ledger().timestamp() > expires_at {
                // Mark as expired
                mark_consent_expired(env, &consent.consent_id);
                continue;
            }
        }
        
        // Check if consent has been revoked
        if consent.revoked_at.is_some() {
            continue;
        }
        
        return Ok(ConsentVerification {
            valid: true,
            consent_id: consent.consent_id.clone(),
            expires_at: consent.expires_at,
            conditions: consent.conditions.clone(),
        });
    }
    
    Err(ConsentError::NoValidConsent)
}
```

### 3. Consent Revocation

**Revocation Process**:
```rust
pub fn revoke_consent(
    env: &Env,
    patient_id: &Address,
    consent_id: &str,
    reason: Option<String>,
) -> Result<(), ConsentError> {
    // Verify patient identity
    require_patient_authentication(env, patient_id)?;
    
    // Get consent record
    let consent_key = generate_consent_key(env, consent_id);
    let mut consent = env.storage().persistent().get(&consent_key)
        .ok_or(ConsentError::ConsentNotFound)?;
    
    // Verify patient owns the consent
    if consent.patient_id != *patient_id {
        return Err(ConsentError::UnauthorizedAccess);
    }
    
    // Check if already revoked
    if consent.revoked_at.is_some() {
        return Err(ConsentError::AlreadyRevoked);
    }
    
    // Revoke consent
    consent.revoked_at = Some(env.ledger().timestamp());
    env.storage().persistent().set(&consent_key, &consent);
    
    // Log revocation
    log_consent_event(env, patient_id, &consent.requester_id, "consent_revoked", consent_id);
    
    // Emit event
    env.events().publish(
        (symbol_short!("CONSENT"), symbol_short!("REVOKED")),
        (patient_id, consent.requester_id, consent_id),
    );
    
    Ok(())
}
```

### 4. Emergency Override

**Emergency Access**:
```rust
pub fn emergency_access(
    env: &Env,
    patient_id: &Address,
    requester_id: &Address,
    required_permission: &str,
    justification: String,
) -> Result<EmergencyAccess, ConsentError> {
    // Verify requester credentials
    require_emergency_credentials(env, requester_id)?;
    
    // Check if emergency access is permitted
    let patient_profile = get_patient_profile(env, patient_id)?;
    if !patient_profile.emergency_access_enabled {
        return Err(ConsentError::EmergencyAccessDisabled);
    }
    
    // Create emergency access record
    let emergency_access = EmergencyAccess {
        patient_id: patient_id.clone(),
        requester_id: requester_id.clone(),
        permission: required_permission.to_string(),
        justification,
        accessed_at: env.ledger().timestamp(),
        post_approval_required: true,
        approved_at: None,
    };
    
    // Store emergency access
    let access_key = generate_emergency_access_key(env, patient_id);
    env.storage().persistent().set(&access_key, &emergency_access);
    
    // Log emergency access
    log_consent_event(env, patient_id, requester_id, "emergency_access", "");
    
    Ok(emergency_access)
}
```

## 🔧 Consent Management Interface

### Patient Consent Dashboard

**Features**:
- View all active consents
- Grant new consent
- Revoke existing consent
- Modify consent terms
- View consent history
- Emergency access settings

**API Endpoints**:
```rust
// Get patient's active consents
pub fn get_patient_consents(
    env: &Env,
    patient_id: &Address,
) -> Vec<Consent> {
    get_active_consents_for_patient(env, patient_id)
}

// Get consent details
pub fn get_consent_details(
    env: &Env,
    consent_id: &str,
) -> Consent {
    let consent_key = generate_consent_key(env, consent_id);
    env.storage().persistent().get(&consent_key)
        .unwrap_or_default()
}

// Check consent status
pub fn check_consent_status(
    env: &Env,
    consent_id: &str,
) -> ConsentStatus {
    let consent = get_consent_details(env, consent_id);
    
    if consent.revoked_at.is_some() {
        ConsentStatus::Revoked
    } else if let Some(expires_at) = consent.expires_at {
        if env.ledger().timestamp() > expires_at {
            ConsentStatus::Expired
        } else {
            ConsentStatus::Active
        }
    } else {
        ConsentStatus::Active
    }
}
```

### Provider Consent View

**Features**:
- View consents granted by patients
- Check access rights for specific patients
- Request additional consent
- View consent expiration dates
- Audit consent usage

**API Endpoints**:
```rust
// Check provider's access rights
pub fn check_access_rights(
    env: &Env,
    provider_id: &Address,
    patient_id: &Address,
    permission: &str,
    data_type: &str,
) -> Result<AccessRights, ConsentError> {
    verify_consent(env, patient_id, provider_id, permission, data_type)
        .map(|verification| AccessRights {
            has_access: verification.valid,
            consent_id: verification.consent_id,
            expires_at: verification.expires_at,
            conditions: verification.conditions,
        })
}

// Request consent from patient
pub fn request_consent(
    env: &Env,
    provider_id: &Address,
    patient_id: &Address,
    permissions: Vec<String>,
    data_types: Vec<String>,
    purpose: String,
    message: String,
) -> Result<String, ConsentError> {
    // Create consent request
    let request_id = generate_request_id(env, provider_id, patient_id);
    let consent_request = ConsentRequest {
        request_id: request_id.clone(),
        provider_id: provider_id.clone(),
        patient_id: patient_id.clone(),
        permissions,
        data_types,
        purpose,
        message,
        created_at: env.ledger().timestamp(),
        status: RequestStatus::Pending,
    };
    
    // Store request
    let request_key = generate_request_key(env, &request_id);
    env.storage().persistent().set(&request_key, &consent_request);
    
    // Notify patient
    notify_patient_of_consent_request(env, patient_id, &request_id);
    
    Ok(request_id)
}
```

## 📊 Consent Analytics and Reporting

### Consent Usage Analytics

**Metrics Tracked**:
- Consent grant/revocation rates
- Time-to-expiration statistics
- Permission usage patterns
- Emergency access frequency
- Provider consent requests

**Analytics Functions**:
```rust
pub struct ConsentAnalytics {
    pub total_consents: u64,
    pub active_consents: u64,
    pub expired_consents: u64,
    pub revoked_consents: u64,
    pub emergency_accesses: u64,
    pub average_consent_duration_days: f64,
    pub most_common_permissions: Vec<(String, u64)>,
    pub consent_by_purpose: Vec<(String, u64)>,
}

pub fn generate_consent_analytics(
    env: &Env,
    start_date: u64,
    end_date: u64,
) -> ConsentAnalytics {
    // Analyze consent data within date range
    let consents = query_consents_in_range(env, start_date, end_date);
    
    ConsentAnalytics {
        total_consents: consents.len() as u64,
        active_consents: count_active_consents(&consents),
        expired_consents: count_expired_consents(&consents),
        revoked_consents: count_revoked_consents(&consents),
        emergency_accesses: count_emergency_accesses(&consents),
        average_consent_duration_days: calculate_average_duration(&consents),
        most_common_permissions: analyze_permission_usage(&consents),
        consent_by_purpose: analyze_purpose_distribution(&consents),
    }
}
```

### Compliance Reports

**HIPAA Consent Compliance Report**:
```rust
pub fn generate_consent_compliance_report(
    env: &Env,
    patient_id: &Address,
    report_period: (u64, u64),
) -> ConsentComplianceReport {
    let consents = get_patient_consents_in_period(env, patient_id, report_period);
    let access_logs = get_access_logs_for_patient(env, patient_id, report_period);
    
    ConsentComplianceReport {
        patient_id: patient_id.clone(),
        report_period,
        total_consents_granted: consents.len(),
        total_access_attempts: access_logs.len(),
        authorized_access: count_authorized_access(&access_logs, &consents),
        unauthorized_access: count_unauthorized_access(&access_logs, &consents),
        consent_compliance_rate: calculate_compliance_rate(&access_logs, &consents),
        emergency_access_incidents: count_emergency_access(&access_logs),
        recommendations: generate_compliance_recommendations(&access_logs, &consents),
    }
}
```

## 🛡️ Security and Privacy

### Consent Data Protection

**Security Measures**:
- **Immutable Storage**: All consent records stored on-chain
- **Digital Signatures**: Consent actions signed by patients
- **Access Controls**: Only patients can modify their consent
- **Audit Logging**: All consent operations logged

**Privacy Measures**:
- **Minimal Data Collection**: Only necessary consent information
- **Patient Control**: Patients control all consent data
- **Data Encryption**: Sensitive consent data encrypted
- **Access Logging**: All consent access logged

### Consent Validation

**Input Validation**:
```rust
fn validate_consent_input(
    permissions: &Vec<String>,
    data_types: &Vec<String>,
    purpose: &str,
    duration_days: Option<u64>,
) -> Result<(), ConsentError> {
    // Validate permissions
    for permission in permissions {
        if !is_valid_permission(permission) {
            return Err(ConsentError::InvalidPermission);
        }
    }
    
    // Validate data types
    for data_type in data_types {
        if !is_valid_data_type(data_type) {
            return Err(ConsentError::InvalidDataType);
        }
    }
    
    // Validate purpose
    if purpose.is_empty() || purpose.len() > 500 {
        return Err(ConsentError::InvalidPurpose);
    }
    
    // Validate duration
    if let Some(days) = duration_days {
        if days > 365 * 5 { // 5 year maximum
            return Err(ConsentError::InvalidDuration);
        }
    }
    
    Ok(())
}
```

## 🧪 Testing and Validation

### Consent Testing

```rust
#[cfg(test)]
mod consent_tests {
    use super::*;
    
    #[test]
    fn test_consent_grant_and_verification() {
        let env = Env::default();
        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        
        // Grant consent
        let consent_id = grant_consent(
            &env,
            &patient,
            &provider,
            vec!["read_medical".to_string()],
            vec!["medical_history".to_string()],
            "Treatment".to_string(),
            Some(30), // 30 days
            vec![],
        ).unwrap();
        
        // Verify consent
        let verification = verify_consent(
            &env,
            &patient,
            &provider,
            "read_medical",
            "medical_history",
        ).unwrap();
        
        assert!(verification.valid);
        assert_eq!(verification.consent_id, consent_id);
    }
    
    #[test]
    fn test_consent_expiration() {
        let env = Env::default();
        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        
        // Grant consent with 1 day expiration
        let consent_id = grant_consent(
            &env,
            &patient,
            &provider,
            vec!["read_medical".to_string()],
            vec!["medical_history".to_string()],
            "Treatment".to_string(),
            Some(1),
            vec![],
        ).unwrap();
        
        // Fast-forward time past expiration
        env.ledger().set_timestamp(env.ledger().timestamp() + 24 * 60 * 60 + 1);
        
        // Verify consent should be expired
        let result = verify_consent(
            &env,
            &patient,
            &provider,
            "read_medical",
            "medical_history",
        );
        
        assert!(result.is_err());
        assert!(matches!(result.err(), Some(ConsentError::NoValidConsent)));
    }
    
    #[test]
    fn test_consent_revocation() {
        let env = Env::default();
        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        
        // Grant consent
        let consent_id = grant_consent(
            &env,
            &patient,
            &provider,
            vec!["read_medical".to_string()],
            vec!["medical_history".to_string()],
            "Treatment".to_string(),
            Some(30),
            vec![],
        ).unwrap();
        
        // Revoke consent
        revoke_consent(&env, &patient, &consent_id, Some("Patient request".to_string())).unwrap();
        
        // Verify consent is revoked
        let status = check_consent_status(&env, &consent_id);
        assert!(matches!(status, ConsentStatus::Revoked));
        
        // Verify access is denied
        let result = verify_consent(
            &env,
            &patient,
            &provider,
            "read_medical",
            "medical_history",
        );
        
        assert!(result.is_err());
    }
}
```

## 📚 References

### Implementation References
- [Consent Contract](../../contracts/common/src/consent.rs)
- [Vision Records Consent](../../contracts/vision_records/src/consent.rs)
- [Audit Trail Documentation](audit-trail.md)
- [HIPAA Requirements Mapping](hipaa-mapping.md)

### Regulatory References
- [HIPAA Privacy Rule § 164.508](https://www.hhs.gov/hipaa/for-professionals/privacy/)
- [HIPAA Security Rule § 164.312](https://www.hhs.gov/hipaa/for-professionals/security/)
- [HITECH Act Requirements](https://www.hhs.gov/hipaa/for-professionals/special-topics/)

### User Experience References
- [Patient Portal Guide](../onboarding/patient-portal.md)
- [Provider Portal Guide](../onboarding/provider-portal.md)
- [API Documentation](../api/consent.md)

---

**Last Updated**: 2025-02-25  
**Next Review**: 2025-03-25  
**Version**: 1.0
