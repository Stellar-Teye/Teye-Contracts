# Emergency Access Protocol

## Overview

The Emergency Access Protocol provides a secure, time-limited mechanism for healthcare providers to access patient records during critical care situations. This protocol ensures that emergency access is:

- **Time-limited**: Maximum 24 hours duration
- **Attested**: Requires explicit attestation from the requester
- **Audited**: All actions are logged in an immutable audit trail
- **Notified**: Emergency contacts are automatically notified
- **Revocable**: Can be revoked by patient, requester, or system admin

## Emergency Conditions

Emergency access can only be granted for the following conditions:

- **LifeThreatening**: Patient's life is in immediate danger
- **Unconscious**: Patient is unconscious and cannot provide consent
- **SurgicalEmergency**: Emergency surgical procedure required
- **Masscasualties**: Mass casualty incident requiring rapid access

## Access Flow

### 1. Granting Emergency Access

A verified healthcare provider can request emergency access by:

1. Authenticating as the requester
2. Specifying the patient address
3. Selecting the emergency condition
4. Providing an attestation (required text explaining the emergency)
5. Setting duration (1 second to 24 hours)
6. Optionally providing emergency contact addresses

**Requirements:**
- Requester must be a verified provider OR have SystemAdmin permission
- Attestation cannot be empty
- Duration must be between 1 second and 24 hours (86400 seconds)

**Example:**
```rust
grant_emergency_access(
    env,
    provider_address,      // Verified provider
    patient_address,       // Patient whose records need access
    EmergencyCondition::LifeThreatening,
    "Patient unconscious, requires immediate vision assessment", // Attestation
    3600,                  // 1 hour duration
    vec![contact1, contact2] // Emergency contacts to notify
)
```

### 2. Using Emergency Access

Once granted, the requester can access patient records using:

```rust
access_record_via_emergency(
    env,
    requester_address,
    patient_address,
    Some(record_id)  // Optional: specific record ID, or None for all records
)
```

**Important:** This function:
- Verifies the emergency access is still active
- Checks that access hasn't expired
- Creates an audit entry
- Publishes an event

### 3. Revoking Emergency Access

Emergency access can be revoked by:
- The patient
- The original requester
- A SystemAdmin

```rust
revoke_emergency_access(
    env,
    revoker_address,
    access_id
)
```

### 4. Automatic Expiration

Emergency accesses automatically expire after their specified duration. The contract provides a function to clean up expired accesses:

```rust
expire_emergency_accesses(env) -> u32  // Returns count of expired accesses
```

## Audit Trail

Every emergency access action is logged in an immutable audit trail:

- **GRANTED**: When emergency access is first granted
- **REVOKED**: When emergency access is revoked
- **ACCESSED**: When records are accessed via emergency access
- **NOTIFIED**: When emergency contacts are notified

### Retrieving Audit Trail

```rust
get_emergency_audit_trail(env, access_id) -> Vec<EmergencyAuditEntry>
```

Each audit entry contains:
- `access_id`: The emergency access ID
- `actor`: Address of the actor performing the action
- `action`: Type of action (GRANTED, REVOKED, ACCESSED, NOTIFIED)
- `timestamp`: When the action occurred

## Events

The protocol emits the following events for monitoring and indexing:

### EmergencyAccessGrantedEvent
Published when emergency access is granted.

**Topics:** `("EMRG_GRT", patient, requester)`

**Data:**
- `access_id`: Unique identifier for the emergency access
- `patient`: Patient address
- `requester`: Requester address
- `condition`: Emergency condition type
- `expires_at`: Expiration timestamp
- `timestamp`: Event timestamp

### EmergencyAccessRevokedEvent
Published when emergency access is revoked.

**Topics:** `("EMRG_REV", patient, revoker)`

**Data:**
- `access_id`: Emergency access ID
- `patient`: Patient address
- `revoker`: Address that revoked the access
- `timestamp`: Event timestamp

### EmergencyContactNotifiedEvent
Published for each emergency contact notified.

**Topics:** `("EMRG_NOT", patient, contact)`

**Data:**
- `access_id`: Emergency access ID
- `patient`: Patient address
- `contact`: Contact address that was notified
- `timestamp`: Event timestamp

### EmergencyAccessUsedEvent
Published when records are accessed via emergency access.

**Topics:** `("EMRG_USE", patient, requester)`

**Data:**
- `access_id`: Emergency access ID
- `patient`: Patient address
- `requester`: Requester address
- `record_id`: Optional record ID that was accessed
- `timestamp`: Event timestamp

## Error Handling

The protocol defines the following error types:

- **EmergencyAccessNotFound**: Emergency access ID does not exist
- **EmergencyAccessExpired**: Emergency access has expired
- **EmergencyAccessRevoked**: Emergency access has been revoked
- **InvalidEmergencyCondition**: Invalid emergency condition specified
- **InvalidAttestation**: Attestation is missing or invalid
- **EmergencyAccessDenied**: Emergency access denied (not active or unauthorized)

All errors are logged and published as error events for monitoring.

## Security Considerations

### Authorization

1. **Granting Access:**
   - Only verified providers or SystemAdmins can grant emergency access
   - Provider verification status is checked

2. **Using Access:**
   - Only the original requester can use the granted emergency access
   - Access must be active and not expired

3. **Revoking Access:**
   - Patient can always revoke their own emergency access
   - Original requester can revoke their own granted access
   - SystemAdmins can revoke any emergency access

### Time Limits

- Maximum emergency access duration: 24 hours
- Access automatically expires after the specified duration
- Expired accesses cannot be used

### Audit Requirements

- All actions are logged in an immutable audit trail
- Audit entries cannot be deleted or modified
- Audit trail is limited to 1000 entries per access ID

## Best Practices

1. **Attestation Quality:**
   - Provide clear, specific attestations explaining the emergency
   - Include relevant medical context
   - Attestations are stored on-chain and are permanent

2. **Duration Selection:**
   - Use the minimum duration necessary for the emergency
   - Consider that longer durations increase risk exposure
   - Default to shorter durations when uncertain

3. **Emergency Contacts:**
   - Provide emergency contacts who should be notified
   - Contacts receive notification events
   - Use trusted addresses for emergency contacts

4. **Monitoring:**
   - Monitor emergency access events for unusual patterns
   - Review audit trails regularly
   - Set up alerts for emergency access grants

5. **Revocation:**
   - Revoke emergency access as soon as it's no longer needed
   - Patients should review and revoke unexpected emergency accesses
   - System admins should monitor and revoke suspicious accesses

## API Reference

### grant_emergency_access

Grants emergency access to a patient's records.

**Parameters:**
- `caller`: Address of the requester (must be verified provider or SystemAdmin)
- `patient`: Address of the patient
- `condition`: Emergency condition type
- `attestation`: Required text explaining the emergency
- `duration_seconds`: Duration in seconds (1 to 86400)
- `emergency_contacts`: Vector of contact addresses to notify

**Returns:** `Result<u64, ContractError>` - Emergency access ID on success

**Errors:**
- `InvalidAttestation`: Attestation is empty
- `InvalidInput`: Duration is 0 or exceeds 24 hours
- `Unauthorized`: Caller is not a verified provider or SystemAdmin

### revoke_emergency_access

Revokes an active emergency access grant.

**Parameters:**
- `caller`: Address of the revoker (patient, requester, or SystemAdmin)
- `access_id`: Emergency access ID to revoke

**Returns:** `Result<(), ContractError>`

**Errors:**
- `EmergencyAccessNotFound`: Access ID does not exist
- `EmergencyAccessExpired`: Access has already expired
- `EmergencyAccessRevoked`: Access has already been revoked
- `Unauthorized`: Caller is not authorized to revoke

### check_emergency_access

Checks if emergency access is currently active.

**Parameters:**
- `patient`: Patient address
- `requester`: Requester address

**Returns:** `Option<EmergencyAccess>` - Active emergency access if found

### access_record_via_emergency

Uses emergency access to read patient records.

**Parameters:**
- `caller`: Address of the requester (must match original requester)
- `patient`: Patient address
- `record_id`: Optional specific record ID, or None for all records

**Returns:** `Result<(), ContractError>`

**Errors:**
- `EmergencyAccessDenied`: No active emergency access found
- `EmergencyAccessExpired`: Emergency access has expired

### get_emergency_access

Retrieves emergency access information by ID.

**Parameters:**
- `access_id`: Emergency access ID

**Returns:** `Result<EmergencyAccess, ContractError>`

**Errors:**
- `EmergencyAccessNotFound`: Access ID does not exist

### get_patient_emergency_accesses

Retrieves all active emergency accesses for a patient.

**Parameters:**
- `patient`: Patient address

**Returns:** `Vec<EmergencyAccess>` - Vector of active emergency accesses

### get_emergency_audit_trail

Retrieves the complete audit trail for an emergency access.

**Parameters:**
- `access_id`: Emergency access ID

**Returns:** `Result<Vec<EmergencyAuditEntry>, ContractError>`

**Errors:**
- `EmergencyAccessNotFound`: Access ID does not exist

### expire_emergency_accesses

Expires emergency accesses that have passed their expiration time.

**Returns:** `u32` - Number of accesses expired

## Compliance

The Emergency Access Protocol is designed to comply with healthcare regulations including:

- **HIPAA**: Emergency access provisions allow access when necessary for treatment
- **GDPR**: Time-limited access with audit trails
- **Medical Ethics**: Requires attestation and justification

## Example Scenarios

### Scenario 1: Unconscious Patient

A patient arrives unconscious at an emergency room. The ER doctor needs immediate access to vision records:

```rust
// ER doctor (verified provider) grants emergency access
let access_id = contract.grant_emergency_access(
    env,
    er_doctor,
    unconscious_patient,
    EmergencyCondition::Unconscious,
    "Patient unconscious, requires immediate vision assessment for head trauma evaluation",
    3600, // 1 hour
    vec![patient_family_member]
)?;

// Access records
contract.access_record_via_emergency(env, er_doctor, unconscious_patient, None)?;

// Revoke when no longer needed
contract.revoke_emergency_access(env, er_doctor, access_id)?;
```

### Scenario 2: Surgical Emergency

A patient requires emergency eye surgery. The surgeon needs access to recent vision records:

```rust
let access_id = contract.grant_emergency_access(
    env,
    surgeon,
    patient,
    EmergencyCondition::SurgicalEmergency,
    "Emergency retinal detachment surgery, need recent vision records",
    7200, // 2 hours
    vec![patient_spouse]
)?;
```

### Scenario 3: Patient Revokes Unexpected Access

A patient notices an unexpected emergency access and revokes it:

```rust
// Get all emergency accesses
let accesses = contract.get_patient_emergency_accesses(env, patient);

// Review audit trail
for access in accesses.iter() {
    let audit = contract.get_emergency_audit_trail(env, access.id)?;
    // Review entries...
}

// Revoke suspicious access
contract.revoke_emergency_access(env, patient, suspicious_access_id)?;
```

## Monitoring and Alerts

Set up monitoring for:

1. **Emergency Access Grants**: Alert on all grants
2. **Unusual Patterns**: Multiple grants from same provider
3. **Long Durations**: Grants exceeding 12 hours
4. **Revocations**: Track revocation patterns
5. **Expired Accesses**: Monitor cleanup operations

## Future Enhancements

Potential improvements to the protocol:

1. **Multi-signature Requirements**: Require multiple providers to approve
2. **Conditional Extensions**: Allow extensions with re-attestation
3. **Granular Permissions**: Different access levels for emergency access
4. **Integration with Consent**: Link to patient consent preferences
5. **Automated Notifications**: Enhanced notification mechanisms
