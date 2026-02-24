use super::*;
use soroban_sdk::testutils::Address as _;

#[test]
fn test_prescription_workflow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    // Register doctor
    client.register_user(
        &admin,
        &doctor,
        &Role::Optometrist,
        &String::from_str(&env, "Dr. Eye"),
    );

    let left_eye = PrescriptionData {
        sphere: String::from_str(&env, "-2.50"),
        cylinder: String::from_str(&env, "-1.25"),
        axis: String::from_str(&env, "180"),
        add: String::from_str(&env, "0.00"),
        pd: String::from_str(&env, "62"),
    };

    let right_eye = PrescriptionData {
        sphere: String::from_str(&env, "-2.75"),
        cylinder: String::from_str(&env, "-1.00"),
        axis: String::from_str(&env, "175"),
        add: String::from_str(&env, "0.00"),
        pd: String::from_str(&env, "62"),
    };

    let rx_id = client.add_prescription(
        &patient,
        &doctor,
        &LensType::Glasses,
        &left_eye,
        &right_eye,
        &OptionalContactLensData::None,
        &31536000, // 1 year
        &String::from_str(&env, "metadata_hash"),
    );

    assert_eq!(rx_id, 1);

    let rx = client.get_prescription(&rx_id);
    assert_eq!(rx.patient, patient);
    assert_eq!(rx.provider, doctor);
    assert!(!rx.verified);

    // Verify prescription
    let pharmacist = Address::generate(&env);
    client.register_user(
        &admin,
        &pharmacist,
        &Role::Admin,
        &String::from_str(&env, "Pharmacist"),
    );

    assert!(client.verify_prescription(&rx_id, &pharmacist));

    let updated_rx = client.get_prescription(&rx_id);
    assert!(updated_rx.verified);

    // Check history
    let history = client.get_prescription_history(&patient);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap(), rx_id);
}

#[test]
fn test_contact_lens_workflow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    client.register_user(
        &admin,
        &doctor,
        &Role::Optometrist,
        &String::from_str(&env, "Dr. Contact"),
    );

    let eye_data = PrescriptionData {
        sphere: String::from_str(&env, "-3.00"),
        cylinder: String::from_str(&env, "0.00"),
        axis: String::from_str(&env, "0"),
        add: String::from_str(&env, "0.00"),
        pd: String::from_str(&env, "60"),
    };

    let contact_data = ContactLensData {
        base_curve: String::from_str(&env, "8.6"),
        diameter: String::from_str(&env, "14.2"),
        brand: String::from_str(&env, "Acuvue"),
    };

    let rx_id = client.add_prescription(
        &patient,
        &doctor,
        &LensType::ContactLens,
        &eye_data,
        &eye_data,
        &OptionalContactLensData::Some(contact_data),
        &15768000, // 6 months
        &String::from_str(&env, "contact_hash"),
    );

    let rx = client.get_prescription(&rx_id);
    assert_eq!(rx.lens_type, LensType::ContactLens);
    assert!(matches!(rx.contact_data, OptionalContactLensData::Some(_)));
}
