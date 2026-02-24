#![allow(clippy::unwrap_used, clippy::expect_used, clippy::arithmetic_side_effects)]
mod common;

use common::setup_test_env;
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};
use vision_records::{Certification, License, Location, VerificationStatus};

fn create_test_license(env: &Env) -> License {
    License {
        number: String::from_str(env, "LIC123456"),
        issuing_authority: String::from_str(env, "State Board"),
        issued_date: 1000,
        expiry_date: 2000,
        license_type: String::from_str(env, "Optometry"),
    }
}

fn create_test_certification(env: &Env) -> Certification {
    Certification {
        name: String::from_str(env, "Board Certified"),
        issuer: String::from_str(env, "Certification Board"),
        issued_date: 1000,
        expiry_date: 2000,
        credential_id: String::from_str(env, "CERT123"),
    }
}

fn create_test_location(env: &Env) -> Location {
    Location {
        name: String::from_str(env, "Main Office"),
        address: String::from_str(env, "123 Main St"),
        city: String::from_str(env, "City"),
        state: String::from_str(env, "State"),
        zip: String::from_str(env, "12345"),
        country: String::from_str(env, "USA"),
    }
}

#[test]
fn test_register_provider() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let mut licenses = Vec::new(&ctx.env);
    licenses.push_back(create_test_license(&ctx.env));
    let mut specialties = Vec::new(&ctx.env);
    specialties.push_back(String::from_str(&ctx.env, "Optometry"));
    specialties.push_back(String::from_str(&ctx.env, "Contact Lenses"));
    let mut certifications = Vec::new(&ctx.env);
    certifications.push_back(create_test_certification(&ctx.env));
    let mut locations = Vec::new(&ctx.env);
    locations.push_back(create_test_location(&ctx.env));

    let provider_id = ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    assert_eq!(provider_id, 1);

    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(provider_data.address, provider);
    assert_eq!(provider_data.name, String::from_str(&ctx.env, "Dr. Smith"));
    assert_eq!(
        provider_data.verification_status,
        VerificationStatus::Pending
    );
    assert!(provider_data.is_active);
    assert_eq!(provider_data.licenses.len(), 1);
    assert_eq!(provider_data.specialties.len(), 2);
    assert_eq!(provider_data.certifications.len(), 1);
    assert_eq!(provider_data.locations.len(), 1);
}

#[test]
fn test_register_provider_unauthorized() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);
    let unauthorized = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    let result = ctx.client.try_register_provider(
        &unauthorized,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    assert!(result.is_err());
}

#[test]
fn test_register_provider_duplicate() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    let result = ctx.client.try_register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    assert!(result.is_err());
}

#[test]
fn test_verify_provider() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Verified);

    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(
        provider_data.verification_status,
        VerificationStatus::Verified
    );
    assert!(provider_data.verified_at.is_some());
    assert!(provider_data.verified_by.is_some());
    assert_eq!(provider_data.verified_by.unwrap(), ctx.admin);
}

#[test]
fn test_verify_provider_rejected() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Rejected);

    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(
        provider_data.verification_status,
        VerificationStatus::Rejected
    );
}

#[test]
fn test_verify_provider_suspended() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Verified);
    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Suspended);

    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(
        provider_data.verification_status,
        VerificationStatus::Suspended
    );
}

#[test]
fn test_verify_provider_unauthorized() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);
    let unauthorized = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    let result =
        ctx.client
            .try_verify_provider(&unauthorized, &provider, &VerificationStatus::Verified);

    assert!(result.is_err());
}

#[test]
fn test_verify_provider_not_found() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let result =
        ctx.client
            .try_verify_provider(&ctx.admin, &provider, &VerificationStatus::Verified);

    assert!(result.is_err());
}

#[test]
fn test_update_provider_by_admin() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    let new_name = Some(String::from_str(&ctx.env, "Dr. John Smith"));
    ctx.client
        .update_provider(&ctx.admin, &provider, &new_name, &None, &None, &None, &None);

    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(
        provider_data.name,
        String::from_str(&ctx.env, "Dr. John Smith")
    );
}

#[test]
fn test_update_provider_by_self() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    let new_name = Some(String::from_str(&ctx.env, "Dr. John Smith"));
    ctx.client
        .update_provider(&provider, &provider, &new_name, &None, &None, &None, &None);

    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(
        provider_data.name,
        String::from_str(&ctx.env, "Dr. John Smith")
    );
}

#[test]
fn test_update_provider_specialties() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let mut specialties = Vec::new(&ctx.env);
    specialties.push_back(String::from_str(&ctx.env, "Optometry"));
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    let mut new_specialties_vec = Vec::new(&ctx.env);
    new_specialties_vec.push_back(String::from_str(&ctx.env, "Contact Lenses"));
    new_specialties_vec.push_back(String::from_str(&ctx.env, "Pediatrics"));
    let new_specialties = Some(new_specialties_vec);

    ctx.client.update_provider(
        &ctx.admin,
        &provider,
        &None,
        &None,
        &new_specialties,
        &None,
        &None,
    );

    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(provider_data.specialties.len(), 2);
}

#[test]
fn test_update_provider_locations() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let mut locations = Vec::new(&ctx.env);
    locations.push_back(create_test_location(&ctx.env));

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    let new_location = Location {
        name: String::from_str(&ctx.env, "Branch Office"),
        address: String::from_str(&ctx.env, "456 Oak Ave"),
        city: String::from_str(&ctx.env, "City2"),
        state: String::from_str(&ctx.env, "State2"),
        zip: String::from_str(&ctx.env, "67890"),
        country: String::from_str(&ctx.env, "USA"),
    };

    let mut new_locations_vec = Vec::new(&ctx.env);
    new_locations_vec.push_back(create_test_location(&ctx.env));
    new_locations_vec.push_back(new_location);
    let new_locations = Some(new_locations_vec);

    ctx.client.update_provider(
        &ctx.admin,
        &provider,
        &None,
        &None,
        &None,
        &None,
        &new_locations,
    );

    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(provider_data.locations.len(), 2);
}

#[test]
fn test_search_providers_by_specialty() {
    let ctx = setup_test_env();

    let provider1 = Address::generate(&ctx.env);
    let provider2 = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let mut specialties1 = Vec::new(&ctx.env);
    specialties1.push_back(String::from_str(&ctx.env, "Optometry"));
    let mut specialties2 = Vec::new(&ctx.env);
    specialties2.push_back(String::from_str(&ctx.env, "Ophthalmology"));
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider1,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses.clone(),
        &specialties1,
        &certifications.clone(),
        &locations.clone(),
    );

    ctx.client.register_provider(
        &ctx.admin,
        &provider2,
        &String::from_str(&ctx.env, "Dr. Jones"),
        &licenses,
        &specialties2,
        &certifications,
        &locations,
    );

    let optometry_providers = ctx
        .client
        .search_providers_by_specialty(&String::from_str(&ctx.env, "Optometry"));

    assert_eq!(optometry_providers.len(), 1);
    assert_eq!(optometry_providers.get(0).unwrap(), provider1);
}

#[test]
fn test_search_providers_by_status() {
    let ctx = setup_test_env();

    let provider1 = Address::generate(&ctx.env);
    let provider2 = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider1,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses.clone(),
        &specialties.clone(),
        &certifications.clone(),
        &locations.clone(),
    );

    ctx.client.register_provider(
        &ctx.admin,
        &provider2,
        &String::from_str(&ctx.env, "Dr. Jones"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    ctx.client
        .verify_provider(&ctx.admin, &provider1, &VerificationStatus::Verified);

    let verified_providers = ctx
        .client
        .search_providers_by_status(&VerificationStatus::Verified);
    assert_eq!(verified_providers.len(), 1);
    assert_eq!(verified_providers.get(0).unwrap(), provider1);

    let pending_providers = ctx
        .client
        .search_providers_by_status(&VerificationStatus::Pending);
    assert_eq!(pending_providers.len(), 1);
    assert_eq!(pending_providers.get(0).unwrap(), provider2);
}

#[test]
fn test_get_provider_count() {
    let ctx = setup_test_env();

    assert_eq!(ctx.client.get_provider_count(), 0);

    let provider1 = Address::generate(&ctx.env);
    let provider2 = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider1,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses.clone(),
        &specialties.clone(),
        &certifications.clone(),
        &locations.clone(),
    );

    assert_eq!(ctx.client.get_provider_count(), 1);

    ctx.client.register_provider(
        &ctx.admin,
        &provider2,
        &String::from_str(&ctx.env, "Dr. Jones"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    assert_eq!(ctx.client.get_provider_count(), 2);
}

#[test]
fn test_provider_multi_location() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);

    let location1 = create_test_location(&ctx.env);
    let location2 = Location {
        name: String::from_str(&ctx.env, "Branch Office"),
        address: String::from_str(&ctx.env, "456 Oak Ave"),
        city: String::from_str(&ctx.env, "City2"),
        state: String::from_str(&ctx.env, "State2"),
        zip: String::from_str(&ctx.env, "67890"),
        country: String::from_str(&ctx.env, "USA"),
    };

    let mut locations = Vec::new(&ctx.env);
    locations.push_back(location1);
    locations.push_back(location2);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(provider_data.locations.len(), 2);
}

#[test]
fn test_provider_certifications() {
    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    let cert1 = create_test_certification(&ctx.env);
    let cert2 = Certification {
        name: String::from_str(&ctx.env, "Advanced Certification"),
        issuer: String::from_str(&ctx.env, "Advanced Board"),
        issued_date: 1500,
        expiry_date: 2500,
        credential_id: String::from_str(&ctx.env, "CERT456"),
    };

    let mut certifications = Vec::new(&ctx.env);
    certifications.push_back(cert1);
    certifications.push_back(cert2);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    let provider_data = ctx.client.get_provider(&provider);
    assert_eq!(provider_data.certifications.len(), 2);
}

#[test]
fn test_provider_events() {
    use soroban_sdk::testutils::Events;

    let ctx = setup_test_env();
    let provider = Address::generate(&ctx.env);

    let licenses = Vec::new(&ctx.env);
    let specialties = Vec::new(&ctx.env);
    let certifications = Vec::new(&ctx.env);
    let locations = Vec::new(&ctx.env);

    ctx.client.register_provider(
        &ctx.admin,
        &provider,
        &String::from_str(&ctx.env, "Dr. Smith"),
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );

    let all_events = ctx.env.events().all();
    assert!(!all_events.is_empty());

    ctx.client
        .verify_provider(&ctx.admin, &provider, &VerificationStatus::Verified);

    let all_events2 = ctx.env.events().all();
    assert!(!all_events2.is_empty());

    ctx.client.update_provider(
        &ctx.admin,
        &provider,
        &Some(String::from_str(&ctx.env, "Dr. John Smith")),
        &None,
        &None,
        &None,
        &None,
    );

    let all_events3 = ctx.env.events().all();
    assert!(!all_events3.is_empty());
}
