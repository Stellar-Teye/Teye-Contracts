# Appointment Scheduling Integration

## Overview

The Appointment Scheduling system provides on-chain tracking and verification of healthcare appointments. This system enables:

- **Scheduling**: Create appointments between patients and providers
- **Status Management**: Track appointment lifecycle (Scheduled, Confirmed, Completed, Cancelled, etc.)
- **History Tracking**: Immutable audit trail of all appointment changes
- **Verification**: Admin verification of appointments
- **Reminders**: Automated reminder system for upcoming appointments
- **Modifications**: Support for rescheduling and status updates

## Appointment Types

The system supports the following appointment types:

- **Examination**: Standard eye examination
- **Consultation**: Consultation appointment
- **FollowUp**: Follow-up visit
- **Surgery**: Surgical procedure
- **Emergency**: Emergency appointment
- **Routine**: Routine checkup

## Appointment Statuses

Appointments progress through the following statuses:

- **Scheduled**: Initial appointment creation
- **Confirmed**: Appointment has been confirmed by patient or provider
- **Completed**: Appointment has been completed
- **Cancelled**: Appointment has been cancelled
- **NoShow**: Patient did not show up (future enhancement)
- **Rescheduled**: Appointment has been rescheduled to a new time

## Appointment Lifecycle

### 1. Scheduling an Appointment

An appointment can be scheduled by:
- The patient (for themselves)
- The provider (for their patients)
- An admin (with ManageUsers permission)

**Requirements:**
- Scheduled time must be in the future
- Duration must be between 1 minute and 8 hours (480 minutes)
- Patient and provider addresses must be valid

**Example:**
```rust
schedule_appointment(
    env,
    caller,              // Patient, provider, or admin
    patient,             // Patient address
    provider,            // Provider address
    AppointmentType::Examination,
    scheduled_at,       // Unix timestamp (must be future)
    duration_minutes,   // 1-480 minutes
    notes               // Optional notes
)
```

### 2. Confirming an Appointment

Appointments can be confirmed by patient, provider, or admin. Confirmation changes status from `Scheduled` to `Confirmed`.

```rust
confirm_appointment(env, caller, appointment_id)
```

### 3. Rescheduling an Appointment

Appointments can be rescheduled by patient, provider, or admin. The new scheduled time must be in the future.

```rust
reschedule_appointment(
    env,
    caller,
    appointment_id,
    new_scheduled_at    // New timestamp (must be future)
)
```

**Note:** Rescheduling resets the reminder flag, so a new reminder can be sent.

### 4. Cancelling an Appointment

Appointments can be cancelled by patient, provider, or admin. Once cancelled or completed, appointments cannot be modified.

```rust
cancel_appointment(env, caller, appointment_id)
```

### 5. Completing an Appointment

Only providers or admins can mark an appointment as completed. This is typically done after the appointment has occurred.

```rust
complete_appointment(env, caller, appointment_id)
```

### 6. Verifying an Appointment

Admins can verify appointments, which adds verification metadata to the appointment record.

```rust
verify_appointment(env, caller, appointment_id)
```

**Requirements:**
- Caller must have ManageUsers permission

## Reminder System

The system includes an automated reminder mechanism for upcoming appointments.

### Sending Reminders

```rust
send_appointment_reminders(env, reminder_window_seconds) -> u32
```

This function:
- Finds appointments scheduled within the reminder window
- That haven't had reminders sent yet
- That are in Scheduled or Confirmed status
- Marks them as reminder_sent = true
- Publishes reminder events
- Returns the count of reminders sent

**Example:**
```rust
// Send reminders for appointments in the next 24 hours
let reminder_count = send_appointment_reminders(env, 86400);
```

## Appointment History

Every appointment maintains an immutable history of all changes:

- **CREATED**: When appointment is first scheduled
- **CONFIRMED**: When appointment is confirmed
- **CANCELLED**: When appointment is cancelled
- **RESCHEDULED**: When appointment is rescheduled
- **COMPLETED**: When appointment is completed
- **VERIFIED**: When appointment is verified
- **REMINDER_SENT**: When reminder is sent

### Retrieving History

```rust
get_appointment_history(env, appointment_id) -> Vec<AppointmentHistoryEntry>
```

Each history entry contains:
- `appointment_id`: The appointment ID
- `action`: Type of action performed
- `actor`: Address of the actor
- `timestamp`: When the action occurred
- `previous_status`: Previous status (if status changed)
- `new_status`: New status (if status changed)
- `notes`: Optional notes

## Events

The system emits the following events for monitoring and indexing:

### AppointmentScheduledEvent
Published when an appointment is created.

**Topics:** `("APPT_SCH", patient, provider)`

**Data:**
- `appointment_id`: Unique appointment identifier
- `patient`: Patient address
- `provider`: Provider address
- `appointment_type`: Type of appointment
- `scheduled_at`: Scheduled timestamp
- `timestamp`: Event timestamp

### AppointmentConfirmedEvent
Published when an appointment is confirmed.

**Topics:** `("APPT_CFM", patient, provider)`

**Data:**
- `appointment_id`: Appointment ID
- `patient`: Patient address
- `provider`: Provider address
- `confirmed_by`: Address that confirmed
- `timestamp`: Event timestamp

### AppointmentCancelledEvent
Published when an appointment is cancelled.

**Topics:** `("APPT_CNL", patient, provider)`

**Data:**
- `appointment_id`: Appointment ID
- `patient`: Patient address
- `provider`: Provider address
- `cancelled_by`: Address that cancelled
- `timestamp`: Event timestamp

### AppointmentRescheduledEvent
Published when an appointment is rescheduled.

**Topics:** `("APPT_RSCH", patient, provider)`

**Data:**
- `appointment_id`: Appointment ID
- `patient`: Patient address
- `provider`: Provider address
- `old_scheduled_at`: Previous scheduled time
- `new_scheduled_at`: New scheduled time
- `rescheduled_by`: Address that rescheduled
- `timestamp`: Event timestamp

### AppointmentCompletedEvent
Published when an appointment is completed.

**Topics:** `("APPT_CMP", patient, provider)`

**Data:**
- `appointment_id`: Appointment ID
- `patient`: Patient address
- `provider`: Provider address
- `completed_by`: Address that completed
- `timestamp`: Event timestamp

### AppointmentReminderEvent
Published when a reminder is sent.

**Topics:** `("APPT_RMD", patient, provider)`

**Data:**
- `appointment_id`: Appointment ID
- `patient`: Patient address
- `provider`: Provider address
- `scheduled_at`: Scheduled timestamp
- `timestamp`: Event timestamp

### AppointmentVerifiedEvent
Published when an appointment is verified.

**Topics:** `("APPT_VER", patient, provider)`

**Data:**
- `appointment_id`: Appointment ID
- `patient`: Patient address
- `provider`: Provider address
- `verifier`: Address that verified
- `timestamp`: Event timestamp

## Error Handling

The system defines the following error types:

- **AppointmentNotFound**: Appointment ID does not exist
- **AppointmentAlreadyExists**: Attempt to create duplicate appointment (future use)
- **InvalidAppointmentTime**: Scheduled time is in the past
- **AppointmentCannotBeModified**: Appointment cannot be modified in current state
- **InvalidAppointmentStatus**: Invalid status transition attempted
- **AppointmentNotVerified**: Appointment verification required but not present

All errors are logged and published as error events for monitoring.

## Authorization

### Scheduling
- Patient can schedule for themselves
- Provider can schedule for their patients
- Admin can schedule for any patient

### Confirming
- Patient can confirm their appointments
- Provider can confirm their appointments
- Admin can confirm any appointment

### Cancelling
- Patient can cancel their appointments
- Provider can cancel their appointments
- Admin can cancel any appointment

### Rescheduling
- Patient can reschedule their appointments
- Provider can reschedule their appointments
- Admin can reschedule any appointment

### Completing
- Only provider can complete their appointments
- Admin can complete any appointment

### Verifying
- Only admin (with ManageUsers permission) can verify appointments

## API Reference

### schedule_appointment

Schedules a new appointment.

**Parameters:**
- `caller`: Address of the caller (must be patient, provider, or admin)
- `patient`: Address of the patient
- `provider`: Address of the provider
- `appointment_type`: Type of appointment
- `scheduled_at`: Scheduled timestamp (must be future)
- `duration_minutes`: Duration in minutes (1-480)
- `notes`: Optional notes string

**Returns:** `Result<u64, ContractError>` - Appointment ID on success

**Errors:**
- `Unauthorized`: Caller is not authorized
- `InvalidAppointmentTime`: Scheduled time is in the past
- `InvalidInput`: Duration is 0 or exceeds 480 minutes

### confirm_appointment

Confirms an appointment.

**Parameters:**
- `caller`: Address of the caller (patient, provider, or admin)
- `appointment_id`: Appointment ID to confirm

**Returns:** `Result<(), ContractError>`

**Errors:**
- `AppointmentNotFound`: Appointment ID does not exist
- `Unauthorized`: Caller is not authorized
- `AppointmentCannotBeModified`: Appointment cannot be confirmed in current state

### cancel_appointment

Cancels an appointment.

**Parameters:**
- `caller`: Address of the caller (patient, provider, or admin)
- `appointment_id`: Appointment ID to cancel

**Returns:** `Result<(), ContractError>`

**Errors:**
- `AppointmentNotFound`: Appointment ID does not exist
- `Unauthorized`: Caller is not authorized
- `AppointmentCannotBeModified`: Appointment cannot be cancelled (already cancelled or completed)

### reschedule_appointment

Reschedules an appointment to a new time.

**Parameters:**
- `caller`: Address of the caller (patient, provider, or admin)
- `appointment_id`: Appointment ID to reschedule
- `new_scheduled_at`: New scheduled timestamp (must be future)

**Returns:** `Result<(), ContractError>`

**Errors:**
- `AppointmentNotFound`: Appointment ID does not exist
- `Unauthorized`: Caller is not authorized
- `InvalidAppointmentTime`: New scheduled time is in the past
- `AppointmentCannotBeModified`: Appointment cannot be rescheduled (cancelled or completed)

### complete_appointment

Marks an appointment as completed.

**Parameters:**
- `caller`: Address of the caller (provider or admin)
- `appointment_id`: Appointment ID to complete

**Returns:** `Result<(), ContractError>`

**Errors:**
- `AppointmentNotFound`: Appointment ID does not exist
- `Unauthorized`: Caller is not the provider or admin
- `AppointmentCannotBeModified`: Appointment cannot be completed (already cancelled or completed)

### verify_appointment

Verifies an appointment.

**Parameters:**
- `caller`: Address of the caller (must be admin)
- `appointment_id`: Appointment ID to verify

**Returns:** `Result<(), ContractError>`

**Errors:**
- `Unauthorized`: Caller does not have ManageUsers permission
- `AppointmentNotFound`: Appointment ID does not exist

### send_appointment_reminders

Sends reminders for appointments within the reminder window.

**Parameters:**
- `reminder_window_seconds`: Time window in seconds (e.g., 86400 for 24 hours)

**Returns:** `Result<u32, ContractError>` - Number of reminders sent

### get_appointment

Retrieves an appointment by ID.

**Parameters:**
- `appointment_id`: Appointment ID

**Returns:** `Result<Appointment, ContractError>`

**Errors:**
- `AppointmentNotFound`: Appointment ID does not exist

### get_patient_appointments

Retrieves all appointments for a patient.

**Parameters:**
- `patient`: Patient address

**Returns:** `Vec<Appointment>` - All appointments for the patient

### get_provider_appointments

Retrieves all appointments for a provider.

**Parameters:**
- `provider`: Provider address

**Returns:** `Vec<Appointment>` - All appointments for the provider

### get_upcoming_patient_appointments

Retrieves upcoming appointments for a patient (scheduled in the future, status Scheduled or Confirmed).

**Parameters:**
- `patient`: Patient address

**Returns:** `Vec<Appointment>` - Upcoming appointments

### get_appointment_history

Retrieves the complete history for an appointment.

**Parameters:**
- `appointment_id`: Appointment ID

**Returns:** `Result<Vec<AppointmentHistoryEntry>, ContractError>`

**Errors:**
- `AppointmentNotFound`: Appointment ID does not exist

## Example Workflows

### Workflow 1: Standard Appointment Flow

```rust
// 1. Patient schedules appointment
let appointment_id = contract.schedule_appointment(
    env,
    patient,
    patient,
    provider,
    AppointmentType::Examination,
    scheduled_at,  // 1 week from now
    30,            // 30 minutes
    None
)?;

// 2. Patient confirms
contract.confirm_appointment(env, patient, appointment_id)?;

// 3. Provider completes after appointment
contract.complete_appointment(env, provider, appointment_id)?;

// 4. Admin verifies
contract.verify_appointment(env, admin, appointment_id)?;
```

### Workflow 2: Rescheduling

```rust
// Schedule appointment
let appointment_id = contract.schedule_appointment(...)?;

// Patient needs to reschedule
let new_time = current_time + 172800; // 2 days later
contract.reschedule_appointment(env, patient, appointment_id, new_time)?;

// Appointment status is now Rescheduled
let appointment = contract.get_appointment(env, appointment_id)?;
assert_eq!(appointment.status, AppointmentStatus::Rescheduled);
```

### Workflow 3: Reminder System

```rust
// Schedule multiple appointments
contract.schedule_appointment(env, patient1, patient1, provider, ...)?;
contract.schedule_appointment(env, patient2, patient2, provider, ...)?;

// Send reminders for appointments in next 24 hours
let reminder_count = contract.send_appointment_reminders(env, 86400)?;
// Returns number of reminders sent

// Check which appointments got reminders
let appointments = contract.get_patient_appointments(env, patient1);
for i in 0..appointments.len() {
    let apt = appointments.get(i).unwrap();
    if apt.reminder_sent {
        // Reminder was sent for this appointment
    }
}
```

### Workflow 4: Appointment History

```rust
let appointment_id = contract.schedule_appointment(...)?;
contract.confirm_appointment(env, patient, appointment_id)?;
contract.complete_appointment(env, provider, appointment_id)?;

// Get full history
let history = contract.get_appointment_history(env, appointment_id)?;

// History contains:
// - CREATED entry (when scheduled)
// - CONFIRMED entry (when confirmed)
// - COMPLETED entry (when completed)
```

## Best Practices

1. **Scheduling:**
   - Schedule appointments well in advance
   - Use appropriate appointment types
   - Include notes for special requirements

2. **Reminders:**
   - Set reminder window based on appointment type
   - Send reminders 24-48 hours before appointment
   - Monitor reminder events for delivery confirmation

3. **Status Management:**
   - Confirm appointments promptly
   - Complete appointments after they occur
   - Cancel appointments as soon as possible if needed

4. **History:**
   - Review appointment history regularly
   - Use history for audit and compliance
   - Track patterns in appointment changes

5. **Verification:**
   - Verify important appointments
   - Use verification for compliance tracking
   - Maintain verification records

## Integration with Records

Appointments can be linked to vision records:

1. **After Completion:**
   - Create a vision record for the completed appointment
   - Link the record to the appointment ID in notes
   - Use appointment history for record context

2. **Before Appointment:**
   - Review patient's previous records
   - Prepare based on appointment type
   - Access records via normal access controls

## Future Enhancements

Potential improvements to the appointment system:

1. **Recurring Appointments**: Support for recurring appointment series
2. **Waitlist Management**: Queue system for appointment slots
3. **Availability Checking**: Real-time provider availability
4. **Automated Reminders**: Scheduled reminder jobs
5. **Appointment Slots**: Time slot management
6. **Integration with Calendar**: External calendar sync
7. **No-Show Tracking**: Automatic no-show detection
8. **Cancellation Policies**: Time-based cancellation rules

## Compliance

The Appointment Scheduling system is designed to comply with:

- **HIPAA**: Appointment data is stored securely on-chain
- **GDPR**: Patient data access controls
- **Medical Ethics**: Proper appointment management
- **Audit Requirements**: Complete history tracking
