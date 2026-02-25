mod common;

use common::setup_test_env;
use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, String, Vec};
use vision_records::{
    AppointmentStatus, AppointmentType, Certification, License, Location, VerificationStatus,
};

type TestContext = common::TestContext;

fn create_test_provider(ctx: &TestContext) -> Address {
    let provider = Address::generate(&ctx.env);
    let name = String::from_str(&ctx.env, "Test Provider");

    let mut licenses = Vec::new(&ctx.env);
    licenses.push_back(License {
        number: String::from_str(&ctx.env, "LIC123456"),
        issuing_authority: String::from_str(&ctx.env, "State Board"),
        issued_date: 1000,
        expiry_date: 2000,
        license_type: String::from_str(&ctx.env, "Ophthalmology"),
    });

    let mut specialties = Vec::new(&ctx.env);
    specialties.push_back(String::from_str(&ctx.env, "General"));

    let mut certifications = Vec::new(&ctx.env);
    certifications.push_back(Certification {
        name: String::from_str(&ctx.env, "Board Certified"),
        issuer: String::from_str(&ctx.env, "Certification Board"),
        issued_date: 1000,
        expiry_date: 2000,
        credential_id: String::from_str(&ctx.env, "CERT123"),
    });

    let mut locations = Vec::new(&ctx.env);
    locations.push_back(Location {
        name: String::from_str(&ctx.env, "Main Office"),
        address: String::from_str(&ctx.env, "123 Main St"),
        city: String::from_str(&ctx.env, "City"),
        state: String::from_str(&ctx.env, "State"),
        zip: String::from_str(&ctx.env, "12345"),
        country: String::from_str(&ctx.env, "USA"),
    });

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &name,
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Verified);

    provider
}

#[test]
fn test_schedule_appointment() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400; // 1 day from now
    let duration = 30u32; // 30 minutes

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    assert!(appointment_id > 0);

    let appointment = ctx.client.get_appointment(&appointment_id);
    assert_eq!(appointment.patient, patient);
    assert_eq!(appointment.provider, provider);
    assert_eq!(appointment.appointment_type, AppointmentType::Examination);
    assert_eq!(appointment.status, AppointmentStatus::Scheduled);
    assert_eq!(appointment.scheduled_at, scheduled_at);
    assert_eq!(appointment.duration_minutes, duration);
}

#[test]
fn test_schedule_appointment_by_provider() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    // Provider can schedule for patient
    let appointment_id = ctx.client.schedule_appointment(
        &provider,
        &patient,
        &provider,
        &AppointmentType::Consultation,
        &scheduled_at,
        &duration,
        &None,
    );

    let appointment = ctx.client.get_appointment(&appointment_id);
    assert_eq!(appointment.patient, patient);
    assert_eq!(appointment.provider, provider);
}

#[test]
fn test_schedule_appointment_unauthorized() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);
    let unauthorized = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    let result = ctx.client.try_schedule_appointment(
        &unauthorized,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    assert!(result.is_err());
}

#[test]
fn test_schedule_appointment_past_time() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    // Set timestamp to a reasonable value to avoid underflow
    ctx.env.ledger().set_timestamp(100000);
    let past_time = ctx.env.ledger().timestamp() - 86400; // 1 day ago
    let duration = 30u32;

    let result = ctx.client.try_schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &past_time,
        &duration,
        &None,
    );

    assert!(result.is_err());
}

#[test]
fn test_schedule_appointment_invalid_duration() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;

    // Try with 0 duration
    let result = ctx.client.try_schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &0u32,
        &None,
    );
    assert!(result.is_err());

    // Try with duration > 8 hours (480 minutes)
    let result = ctx.client.try_schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &481u32,
        &None,
    );
    assert!(result.is_err());
}

#[test]
fn test_confirm_appointment() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Patient confirms
    ctx.client.confirm_appointment(&patient, &appointment_id);

    let appointment = ctx.client.get_appointment(&appointment_id);
    assert_eq!(appointment.status, AppointmentStatus::Confirmed);
}

#[test]
fn test_confirm_appointment_wrong_status() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Confirm once
    ctx.client.confirm_appointment(&patient, &appointment_id);

    // Try to confirm again (should fail)
    let result = ctx
        .client
        .try_confirm_appointment(&patient, &appointment_id);
    assert!(result.is_err());
}

#[test]
fn test_cancel_appointment() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Patient cancels
    ctx.client.cancel_appointment(&patient, &appointment_id);

    let appointment = ctx.client.get_appointment(&appointment_id);
    assert_eq!(appointment.status, AppointmentStatus::Cancelled);
}

#[test]
fn test_cancel_appointment_already_cancelled() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Cancel once
    ctx.client.cancel_appointment(&patient, &appointment_id);

    // Try to cancel again (should fail)
    let result = ctx.client.try_cancel_appointment(&patient, &appointment_id);
    assert!(result.is_err());
}

#[test]
fn test_reschedule_appointment() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let new_scheduled_at = ctx.env.ledger().timestamp() + 172800; // 2 days from now
    let duration = 30u32;

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Patient reschedules
    ctx.client
        .reschedule_appointment(&patient, &appointment_id, &new_scheduled_at);

    let appointment = ctx.client.get_appointment(&appointment_id);
    assert_eq!(appointment.scheduled_at, new_scheduled_at);
    assert_eq!(appointment.status, AppointmentStatus::Rescheduled);
    assert_eq!(appointment.reminder_sent, false); // Reminder should be reset
}

#[test]
fn test_reschedule_appointment_past_time() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    // Set timestamp to a reasonable value to avoid underflow
    ctx.env.ledger().set_timestamp(100000);
    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let past_time = ctx.env.ledger().timestamp() - 86400;
    let duration = 30u32;

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Try to reschedule to past time (should fail)
    let result = ctx
        .client
        .try_reschedule_appointment(&patient, &appointment_id, &past_time);
    assert!(result.is_err());
}

#[test]
fn test_complete_appointment() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Provider completes
    ctx.client.complete_appointment(&provider, &appointment_id);

    let appointment = ctx.client.get_appointment(&appointment_id);
    assert_eq!(appointment.status, AppointmentStatus::Completed);
}

#[test]
fn test_complete_appointment_unauthorized() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Patient tries to complete (should fail - only provider can complete)
    let result = ctx
        .client
        .try_complete_appointment(&patient, &appointment_id);
    assert!(result.is_err());
}

#[test]
fn test_verify_appointment() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Admin verifies
    ctx.client.verify_appointment(&ctx.admin, &appointment_id);

    let appointment = ctx.client.get_appointment(&appointment_id);
    assert!(appointment.verified_at.is_some());
    assert_eq!(appointment.verified_by.unwrap(), ctx.admin);
}

#[test]
fn test_verify_appointment_unauthorized() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Patient tries to verify (should fail - only admin can verify)
    let result = ctx.client.try_verify_appointment(&patient, &appointment_id);
    assert!(result.is_err());
}

#[test]
fn test_get_patient_appointments() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at1 = ctx.env.ledger().timestamp() + 86400;
    let scheduled_at2 = ctx.env.ledger().timestamp() + 172800;
    let duration = 30u32;

    let appointment_id1 = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at1,
        &duration,
        &None,
    );

    let appointment_id2 = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Consultation,
        &scheduled_at2,
        &duration,
        &None,
    );

    let appointments = ctx.client.get_patient_appointments(&patient);
    assert!(appointments.len() >= 2);

    // Verify both appointments are present
    let mut found_id1 = false;
    let mut found_id2 = false;
    for i in 0..appointments.len() {
        let apt = appointments.get(i).unwrap();
        if apt.id == appointment_id1 {
            found_id1 = true;
        }
        if apt.id == appointment_id2 {
            found_id2 = true;
        }
    }
    assert!(found_id1);
    assert!(found_id2);
}

#[test]
fn test_get_provider_appointments() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient1 = Address::generate(&ctx.env);
    let patient2 = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    let appointment_id1 = ctx.client.schedule_appointment(
        &patient1,
        &patient1,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    let appointment_id2 = ctx.client.schedule_appointment(
        &patient2,
        &patient2,
        &provider,
        &AppointmentType::Consultation,
        &scheduled_at,
        &duration,
        &None,
    );

    let appointments = ctx.client.get_provider_appointments(&provider);
    assert!(appointments.len() >= 2);

    // Verify both appointments are present
    let mut found_id1 = false;
    let mut found_id2 = false;
    for i in 0..appointments.len() {
        let apt = appointments.get(i).unwrap();
        if apt.id == appointment_id1 {
            found_id1 = true;
        }
        if apt.id == appointment_id2 {
            found_id2 = true;
        }
    }
    assert!(found_id1);
    assert!(found_id2);
}

#[test]
fn test_get_upcoming_patient_appointments() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    // Set timestamp to a reasonable value to avoid underflow
    ctx.env.ledger().set_timestamp(100000);
    let future_time = ctx.env.ledger().timestamp() + 86400;
    let past_time = ctx.env.ledger().timestamp() - 86400;
    let duration = 30u32;

    // Schedule future appointment
    let future_appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &future_time,
        &duration,
        &None,
    );

    // Try to schedule past appointment (should fail)
    let result = ctx.client.try_schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Consultation,
        &past_time,
        &duration,
        &None,
    );
    assert!(result.is_err()); // Past appointments should be rejected

    let upcoming = ctx.client.get_patient_upcoming(&patient);
    assert_eq!(upcoming.len(), 1); // Only the future appointment should be in upcoming

    // Verify future appointment is in upcoming list
    let mut found_future = false;
    for i in 0..upcoming.len() {
        let apt = upcoming.get(i).unwrap();
        if apt.id == future_appointment_id {
            found_future = true;
            assert!(apt.scheduled_at > ctx.env.ledger().timestamp());
        }
    }
    assert!(found_future);
}

#[test]
fn test_appointment_history() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Confirm
    ctx.client.confirm_appointment(&patient, &appointment_id);

    // Complete
    ctx.client.complete_appointment(&provider, &appointment_id);

    // Get history
    let history = ctx.client.get_appointment_history(&appointment_id);
    assert!(history.len() >= 3); // CREATED, CONFIRMED, COMPLETED

    // Verify history entries
    let created_str = String::from_str(&ctx.env, "CREATED");
    let confirmed_str = String::from_str(&ctx.env, "CONFIRMED");
    let completed_str = String::from_str(&ctx.env, "COMPLETED");

    let mut found_created = false;
    let mut found_confirmed = false;
    let mut found_completed = false;

    for i in 0..history.len() {
        let entry = history.get(i).unwrap();
        if entry.action == created_str {
            found_created = true;
        }
        if entry.action == confirmed_str {
            found_confirmed = true;
        }
        if entry.action == completed_str {
            found_completed = true;
        }
    }

    assert!(found_created);
    assert!(found_confirmed);
    assert!(found_completed);
}

#[test]
fn test_send_appointment_reminders() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    // Set current time
    ctx.env.ledger().set_timestamp(1000);

    // Schedule appointment 1 hour from now (within reminder window)
    let scheduled_at = 1000 + 3600; // 1 hour from now
    let duration = 30u32;
    let reminder_window = 7200u64; // 2 hour window

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &None,
    );

    // Send reminders
    let reminder_count = ctx.client.send_appointment_reminders(&reminder_window);
    assert!(reminder_count >= 1);

    // Verify reminder was sent
    let appointment = ctx.client.get_appointment(&appointment_id);
    assert_eq!(appointment.reminder_sent, true);
}

#[test]
fn test_appointment_with_notes() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;
    let notes = Some(String::from_str(
        &ctx.env,
        "Patient needs special accommodation",
    ));

    let appointment_id = ctx.client.schedule_appointment(
        &patient,
        &patient,
        &provider,
        &AppointmentType::Examination,
        &scheduled_at,
        &duration,
        &notes,
    );

    let appointment = ctx.client.get_appointment(&appointment_id);
    assert!(appointment.notes.is_some());
    assert_eq!(
        appointment.notes.unwrap(),
        String::from_str(&ctx.env, "Patient needs special accommodation")
    );
}

#[test]
fn test_appointment_different_types() {
    let ctx = setup_test_env();
    let provider = create_test_provider(&ctx);
    let patient = Address::generate(&ctx.env);

    let scheduled_at = ctx.env.ledger().timestamp() + 86400;
    let duration = 30u32;

    // Test all appointment types
    let types = [
        AppointmentType::Examination,
        AppointmentType::Consultation,
        AppointmentType::FollowUp,
        AppointmentType::Surgery,
        AppointmentType::Emergency,
        AppointmentType::Routine,
    ];

    for apt_type in types.iter() {
        let appointment_id = ctx.client.schedule_appointment(
            &patient,
            &patient,
            &provider,
            apt_type,
            &scheduled_at,
            &duration,
            &None,
        );

        let appointment = ctx.client.get_appointment(&appointment_id);
        assert_eq!(appointment.appointment_type, *apt_type);

        // Clean up
        ctx.client.cancel_appointment(&patient, &appointment_id);
    }
}
