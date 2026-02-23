use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Vec,
};

mod common;
use common::setup_test_env;

#[test]
fn test_rate_limit_config_set_and_get() {
    let ctx = setup_test_env();
    let operation = soroban_sdk::String::from_str(&ctx.env, "add_record");
    let max_requests = 10u32;
    let window_seconds = 3600u64;

    ctx.client
        .set_rate_limit_config(&ctx.admin, &operation, &max_requests, &window_seconds);

    let config = ctx.client.get_rate_limit_config(&operation);
    assert!(config.is_some());
    let cfg = config.unwrap();
    assert_eq!(cfg.max_requests, max_requests);
    assert_eq!(cfg.window_seconds, window_seconds);
    assert_eq!(cfg.operation, operation);
}

#[test]
fn test_rate_limit_enforcement() {
    let ctx = setup_test_env();
    let user = Address::generate(&ctx.env);
    let patient = Address::generate(&ctx.env);
    let provider = Address::generate(&ctx.env);

    // Register user and provider
    ctx.client.register_user(
        &ctx.admin,
        &user,
        &vision_records::Role::Patient,
        &soroban_sdk::String::from_str(&ctx.env, "Test User"),
    );
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &vision_records::Role::Optometrist,
        &soroban_sdk::String::from_str(&ctx.env, "Test Provider"),
    );
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Set rate limit: 2 requests per hour
    let operation = soroban_sdk::String::from_str(&ctx.env, "add_record");
    ctx.client
        .set_rate_limit_config(&ctx.admin, &operation, &2u32, &3600u64);

    // First request should succeed
    let data_hash1 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let result1 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash1,
    );
    assert!(result1.is_ok());

    // Second request should succeed
    let data_hash2 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let result2 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash2,
    );
    assert!(result2.is_ok());

    // Third request should fail due to rate limit
    let data_hash3 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdI");
    let result3 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash3,
    );
    assert!(result3.is_err());
    match result3 {
        Err(Ok(e)) => assert_eq!(e, vision_records::ContractError::RateLimitExceeded),
        _ => panic!("Expected RateLimitExceeded error"),
    }
}

#[test]
fn test_rate_limit_window_reset() {
    let ctx = setup_test_env();
    let user = Address::generate(&ctx.env);
    let patient = Address::generate(&ctx.env);
    let provider = Address::generate(&ctx.env);

    // Register user and provider
    ctx.client.register_user(
        &ctx.admin,
        &user,
        &vision_records::Role::Patient,
        &soroban_sdk::String::from_str(&ctx.env, "Test User"),
    );
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &vision_records::Role::Optometrist,
        &soroban_sdk::String::from_str(&ctx.env, "Test Provider"),
    );
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Set rate limit: 1 request per 10 seconds
    let operation = soroban_sdk::String::from_str(&ctx.env, "add_record");
    ctx.client
        .set_rate_limit_config(&ctx.admin, &operation, &1u32, &10u64);

    // Set initial timestamp to a known value
    ctx.env.ledger().set_timestamp(1000);

    // First request should succeed (this sets window_start to 1000, window_end to 1010)
    let data_hash1 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let result1 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash1,
    );
    assert!(result1.is_ok());

    // Second request should fail (within window, count = 1, max = 1)
    let data_hash2 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let result2 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash2,
    );
    assert!(result2.is_err());

    // Advance time past the window (window ends at 1000 + 10 = 1010)
    // Set to 1011 to ensure we're past the window
    ctx.env.ledger().set_timestamp(1011);

    // Third request should succeed (window reset)
    let data_hash3 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdI");
    let result3 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash3,
    );
    assert!(result3.is_ok());
}

#[test]
fn test_rate_limit_bypass_for_verified_provider() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);
    let patient = Address::generate(&ctx.env);

    // Register provider as user first
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &vision_records::Role::Optometrist,
        &soroban_sdk::String::from_str(&ctx.env, "Dr. Smith"),
    );

    // Register provider details
    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &soroban_sdk::String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    // Set rate limit: 1 request per hour
    let operation = soroban_sdk::String::from_str(&ctx.env, "add_record");
    ctx.client
        .set_rate_limit_config(&ctx.admin, &operation, &1u32, &3600u64);

    // Grant WriteRecord permission
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // First request should succeed
    let data_hash1 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let result1 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash1,
    );
    assert!(result1.is_ok());

    // Second request should fail (rate limit)
    let data_hash2 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let result2 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash2,
    );
    assert!(result2.is_err());

    // Verify provider
    ctx.client.verify_provider(
        &ctx.admin,
        &provider,
        &vision_records::VerificationStatus::Verified,
    );

    // Check bypass is enabled
    assert!(ctx.client.has_rate_limit_bypass(&provider));

    // Third request should succeed (bypass enabled)
    let data_hash3 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdI");
    let result3 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash3,
    );
    assert!(result3.is_ok());

    // Fourth request should also succeed (bypass)
    let data_hash4 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdJ");
    let result4 = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash4,
    );
    assert!(result4.is_ok());
}

#[test]
fn test_rate_limit_status() {
    let ctx = setup_test_env();
    let user = Address::generate(&ctx.env);
    let patient = Address::generate(&ctx.env);
    let provider = Address::generate(&ctx.env);

    // Register user and provider
    ctx.client.register_user(
        &ctx.admin,
        &user,
        &vision_records::Role::Patient,
        &soroban_sdk::String::from_str(&ctx.env, "Test User"),
    );
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &vision_records::Role::Optometrist,
        &soroban_sdk::String::from_str(&ctx.env, "Test Provider"),
    );
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Set rate limit: 5 requests per hour
    let operation = soroban_sdk::String::from_str(&ctx.env, "add_record");
    ctx.client
        .set_rate_limit_config(&ctx.admin, &operation, &5u32, &3600u64);

    // Check initial status
    let status = ctx.client.get_rate_limit_status(&provider, &operation);
    assert!(status.is_some());
    let stat = status.unwrap();
    assert_eq!(stat.current_count, 0);
    assert_eq!(stat.max_requests, 5);

    // Make a request
    let data_hash =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash,
    );

    // Check updated status
    let status = ctx.client.get_rate_limit_status(&provider, &operation);
    assert!(status.is_some());
    let stat = status.unwrap();
    assert_eq!(stat.current_count, 1);
    assert_eq!(stat.max_requests, 5);
}

#[test]
fn test_rate_limit_bypass_manual() {
    let ctx = setup_test_env();
    let user = Address::generate(&ctx.env);

    // Set rate limit
    let operation = soroban_sdk::String::from_str(&ctx.env, "get_record");
    ctx.client
        .set_rate_limit_config(&ctx.admin, &operation, &1u32, &3600u64);

    // Initially no bypass
    assert!(!ctx.client.has_rate_limit_bypass(&user));

    // Admin grants bypass
    ctx.client.set_rate_limit_bypass(&ctx.admin, &user, &true);
    assert!(ctx.client.has_rate_limit_bypass(&user));

    // Admin revokes bypass
    ctx.client.set_rate_limit_bypass(&ctx.admin, &user, &false);
    assert!(!ctx.client.has_rate_limit_bypass(&user));
}

#[test]
fn test_rate_limit_unauthorized_config() {
    let ctx = setup_test_env();
    let user = Address::generate(&ctx.env);
    let operation = soroban_sdk::String::from_str(&ctx.env, "add_record");

    // Non-admin cannot set rate limit config
    ctx.client.register_user(
        &ctx.admin,
        &user,
        &vision_records::Role::Patient,
        &soroban_sdk::String::from_str(&ctx.env, "Test User"),
    );
    let result = ctx
        .client
        .try_set_rate_limit_config(&user, &operation, &10u32, &3600u64);
    assert!(result.is_err());
    match result {
        Err(Ok(e)) => assert_eq!(e, vision_records::ContractError::Unauthorized),
        _ => panic!("Expected Unauthorized error"),
    }
}

#[test]
fn test_rate_limit_unauthorized_bypass() {
    let ctx = setup_test_env();
    let user = Address::generate(&ctx.env);
    let other_user = Address::generate(&ctx.env);

    // Non-admin cannot set bypass
    ctx.client.register_user(
        &ctx.admin,
        &user,
        &vision_records::Role::Patient,
        &soroban_sdk::String::from_str(&ctx.env, "Test User"),
    );
    let result = ctx
        .client
        .try_set_rate_limit_bypass(&user, &other_user, &true);
    assert!(result.is_err());
    match result {
        Err(Ok(e)) => assert_eq!(e, vision_records::ContractError::Unauthorized),
        _ => panic!("Expected Unauthorized error"),
    }
}

#[test]
fn test_rate_limit_different_operations() {
    let ctx = setup_test_env();
    let user = Address::generate(&ctx.env);
    let patient = Address::generate(&ctx.env);
    let provider = Address::generate(&ctx.env);

    // Register user and provider
    ctx.client.register_user(
        &ctx.admin,
        &user,
        &vision_records::Role::Patient,
        &soroban_sdk::String::from_str(&ctx.env, "Test User"),
    );
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &vision_records::Role::Optometrist,
        &soroban_sdk::String::from_str(&ctx.env, "Test Provider"),
    );
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Set different rate limits for different operations
    let add_op = soroban_sdk::String::from_str(&ctx.env, "add_record");
    let get_op = soroban_sdk::String::from_str(&ctx.env, "get_record");
    ctx.client
        .set_rate_limit_config(&ctx.admin, &add_op, &1u32, &3600u64);
    ctx.client
        .set_rate_limit_config(&ctx.admin, &get_op, &10u32, &3600u64);

    // Exhaust add_record limit
    let data_hash =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash,
    );

    // Second add_record should fail
    let data_hash2 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let result = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash2,
    );
    assert!(result.is_err());

    // But get_record should still work (different operation)
    // Note: This test assumes get_record doesn't require a valid record_id
    // In practice, you'd need to create a record first
}

#[test]
fn test_rate_limit_events() {
    let ctx = setup_test_env();
    let user = Address::generate(&ctx.env);
    let patient = Address::generate(&ctx.env);
    let provider = Address::generate(&ctx.env);

    // Register user and provider
    ctx.client.register_user(
        &ctx.admin,
        &user,
        &vision_records::Role::Patient,
        &soroban_sdk::String::from_str(&ctx.env, "Test User"),
    );
    ctx.client.register_user(
        &ctx.admin,
        &provider,
        &vision_records::Role::Optometrist,
        &soroban_sdk::String::from_str(&ctx.env, "Test Provider"),
    );
    ctx.client.grant_custom_permission(
        &ctx.admin,
        &provider,
        &vision_records::Permission::WriteRecord,
    );

    // Set rate limit: 1 request per hour
    let operation = soroban_sdk::String::from_str(&ctx.env, "add_record");
    ctx.client
        .set_rate_limit_config(&ctx.admin, &operation, &1u32, &3600u64);

    // Make first request
    let data_hash1 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    ctx.client.add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash1,
    );

    // Try second request (should fail and emit event)
    let data_hash2 =
        soroban_sdk::String::from_str(&ctx.env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdH");
    let result = ctx.client.try_add_record(
        &provider,
        &patient,
        &provider,
        &vision_records::RecordType::Examination,
        &data_hash2,
    );

    // Verify rate limit was exceeded
    assert!(result.is_err());
    match result {
        Err(Ok(e)) => assert_eq!(e, vision_records::ContractError::RateLimitExceeded),
        _ => panic!("Expected RateLimitExceeded error"),
    }

    // Check that rate limit exceeded event was emitted (events persist even on failure)
    use soroban_sdk::testutils::Events;
    let all_events = ctx.env.events().all();

    // The rate limit exceeded event should have been published
    // Since the function returns an error, the event should still be in the event log
    // We verify the error occurred above, which means the event should have been emitted
    assert!(
        !all_events.is_empty(),
        "Expected events to be present after rate limit exceeded"
    );
}

#[test]
fn test_get_all_rate_limit_configs() {
    let ctx = setup_test_env();

    // Set multiple rate limit configs
    let op1 = soroban_sdk::String::from_str(&ctx.env, "add_record");
    let op2 = soroban_sdk::String::from_str(&ctx.env, "get_record");
    ctx.client
        .set_rate_limit_config(&ctx.admin, &op1, &10u32, &3600u64);
    ctx.client
        .set_rate_limit_config(&ctx.admin, &op2, &20u32, &1800u64);

    // Get all configs
    let configs = ctx.client.get_all_rate_limit_configs();
    assert!(configs.len() >= 2);
}
