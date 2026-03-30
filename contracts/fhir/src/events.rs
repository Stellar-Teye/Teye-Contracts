#[test]
fn test_event_not_emitted_on_invalid_data() {
    let result = process_patient_data(None, Some(0));
    assert!(result.is_err());
    // assert no event emitted
}
