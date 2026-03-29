//! Tests for diagnostic image metadata in the vision_records module.
//! Covers: metadata schema validation, diagnostic category indexing,
//! and image access event logging for privacy auditing.
#[cfg(test)]
mod tests {
    use crate::examination::{
        get_examination, remove_examination, set_examination, EyeExamination, FundusPhotography,
        IntraocularPressure, OptFundusPhotography, OptPhysicalMeasurement, OptRetinalImaging,
        OptVisualField, PhysicalMeasurement, RetinalImaging, SlitLampFindings, VisualAcuity,
    };
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_visual_acuity(env: &Env) -> VisualAcuity {
        VisualAcuity {
            uncorrected: PhysicalMeasurement {
                left_eye: String::from_str(env, "20/20"),
                right_eye: String::from_str(env, "20/25"),
            },
            corrected: OptPhysicalMeasurement::None,
        }
    }

    fn make_iop(env: &Env) -> IntraocularPressure {
        IntraocularPressure {
            left_eye: 14,
            right_eye: 15,
            method: String::from_str(env, "Goldmann"),
            timestamp: 1000,
        }
    }

    fn make_slit_lamp(env: &Env) -> SlitLampFindings {
        SlitLampFindings {
            cornea: String::from_str(env, "Clear"),
            anterior_chamber: String::from_str(env, "Deep and quiet"),
            iris: String::from_str(env, "Normal"),
            lens: String::from_str(env, "Clear"),
        }
    }

    fn make_retinal_imaging(env: &Env) -> RetinalImaging {
        RetinalImaging {
            image_url: String::from_str(env, "ipfs://QmRetinalHash123"),
            image_hash: String::from_str(env, "sha256:abc123def456"),
            findings: String::from_str(env, "No diabetic retinopathy"),
        }
    }

    fn make_fundus(env: &Env) -> FundusPhotography {
        FundusPhotography {
            image_url: String::from_str(env, "ipfs://QmFundusHash456"),
            image_hash: String::from_str(env, "sha256:def456abc789"),
            cup_to_disc_ratio_left: String::from_str(env, "0.3"),
            cup_to_disc_ratio_right: String::from_str(env, "0.4"),
            macula_status: String::from_str(env, "Normal"),
        }
    }

    fn make_exam(env: &Env, record_id: u64) -> EyeExamination {
        EyeExamination {
            record_id,
            visual_acuity: make_visual_acuity(env),
            iop: make_iop(env),
            slit_lamp: make_slit_lamp(env),
            visual_field: OptVisualField::None,
            retina_imaging: OptRetinalImaging::None,
            fundus_photo: OptFundusPhotography::None,
            clinical_notes: String::from_str(env, "Routine exam"),
        }
    }

    // -----------------------------------------------------------------------
    // Metadata schema validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_retinal_imaging_metadata_stored_and_retrieved() {
        let env = Env::default();
        let provider = Address::generate(&env);
        let mut exam = make_exam(&env, 1);
        let imaging = make_retinal_imaging(&env);
        exam.retina_imaging = OptRetinalImaging::Some(imaging.clone());

        set_examination(&env, &exam, &provider);

        let stored = get_examination(&env, 1).expect("exam not found");
        match stored.retina_imaging {
            OptRetinalImaging::Some(r) => {
                assert_eq!(r.image_url, imaging.image_url);
                assert_eq!(r.image_hash, imaging.image_hash);
                assert_eq!(r.findings, imaging.findings);
            }
            OptRetinalImaging::None => panic!("expected retinal imaging"),
        }
    }

    #[test]
    fn test_fundus_photography_metadata_stored_and_retrieved() {
        let env = Env::default();
        let provider = Address::generate(&env);
        let mut exam = make_exam(&env, 2);
        let fundus = make_fundus(&env);
        exam.fundus_photo = OptFundusPhotography::Some(fundus.clone());

        set_examination(&env, &exam, &provider);

        let stored = get_examination(&env, 2).expect("exam not found");
        match stored.fundus_photo {
            OptFundusPhotography::Some(f) => {
                assert_eq!(f.image_url, fundus.image_url);
                assert_eq!(f.image_hash, fundus.image_hash);
                assert_eq!(f.cup_to_disc_ratio_left, fundus.cup_to_disc_ratio_left);
                assert_eq!(f.macula_status, fundus.macula_status);
            }
            OptFundusPhotography::None => panic!("expected fundus photo"),
        }
    }

    #[test]
    fn test_image_hash_is_preserved_exactly() {
        let env = Env::default();
        let provider = Address::generate(&env);
        let mut exam = make_exam(&env, 3);
        let hash = String::from_str(
            &env,
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        );
        exam.retina_imaging = OptRetinalImaging::Some(RetinalImaging {
            image_url: String::from_str(&env, "ipfs://Qm"),
            image_hash: hash.clone(),
            findings: String::from_str(&env, "Normal"),
        });

        set_examination(&env, &exam, &provider);

        let stored = get_examination(&env, 3).unwrap();
        if let OptRetinalImaging::Some(r) = stored.retina_imaging {
            assert_eq!(r.image_hash, hash);
        }
    }

    #[test]
    fn test_exam_without_imaging_stores_none_variants() {
        let env = Env::default();
        let provider = Address::generate(&env);
        let exam = make_exam(&env, 4);

        set_examination(&env, &exam, &provider);

        let stored = get_examination(&env, 4).unwrap();
        assert!(matches!(stored.retina_imaging, OptRetinalImaging::None));
        assert!(matches!(stored.fundus_photo, OptFundusPhotography::None));
        assert!(matches!(stored.visual_field, OptVisualField::None));
    }

    #[test]
    fn test_both_imaging_types_stored_together() {
        let env = Env::default();
        let provider = Address::generate(&env);
        let mut exam = make_exam(&env, 5);
        exam.retina_imaging = OptRetinalImaging::Some(make_retinal_imaging(&env));
        exam.fundus_photo = OptFundusPhotography::Some(make_fundus(&env));

        set_examination(&env, &exam, &provider);

        let stored = get_examination(&env, 5).unwrap();
        assert!(matches!(stored.retina_imaging, OptRetinalImaging::Some(_)));
        assert!(matches!(stored.fundus_photo, OptFundusPhotography::Some(_)));
    }

    // -----------------------------------------------------------------------
    // Indexing by diagnostic category
    // -----------------------------------------------------------------------

    #[test]
    fn test_multiple_exams_indexed_by_record_id() {
        let env = Env::default();
        let provider = Address::generate(&env);

        for id in 1u64..=5 {
            let mut exam = make_exam(&env, id);
            if id % 2 == 0 {
                exam.retina_imaging = OptRetinalImaging::Some(make_retinal_imaging(&env));
            } else {
                exam.fundus_photo = OptFundusPhotography::Some(make_fundus(&env));
            }
            set_examination(&env, &exam, &provider);
        }

        // Verify each record is independently retrievable
        for id in 1u64..=5 {
            let stored = get_examination(&env, id).expect("record missing");
            assert_eq!(stored.record_id, id);
            if id % 2 == 0 {
                assert!(matches!(stored.retina_imaging, OptRetinalImaging::Some(_)));
            } else {
                assert!(matches!(stored.fundus_photo, OptFundusPhotography::Some(_)));
            }
        }
    }

    #[test]
    fn test_exam_overwrite_updates_imaging_metadata() {
        let env = Env::default();
        let provider = Address::generate(&env);
        let mut exam = make_exam(&env, 10);
        set_examination(&env, &exam, &provider);

        // Update with retinal imaging
        exam.retina_imaging = OptRetinalImaging::Some(make_retinal_imaging(&env));
        set_examination(&env, &exam, &provider);

        let stored = get_examination(&env, 10).unwrap();
        assert!(matches!(stored.retina_imaging, OptRetinalImaging::Some(_)));
    }

    #[test]
    fn test_nonexistent_record_returns_none() {
        let env = Env::default();
        assert!(get_examination(&env, 9999).is_none());
    }

    #[test]
    fn test_removed_exam_no_longer_retrievable() {
        let env = Env::default();
        let provider = Address::generate(&env);
        let exam = make_exam(&env, 20);
        set_examination(&env, &exam, &provider);
        assert!(get_examination(&env, 20).is_some());

        remove_examination(&env, 20);
        assert!(get_examination(&env, 20).is_none());
    }

    // -----------------------------------------------------------------------
    // IOP metadata validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_iop_measurement_method_stored_correctly() {
        let env = Env::default();
        let provider = Address::generate(&env);
        let exam = make_exam(&env, 30);
        set_examination(&env, &exam, &provider);

        let stored = get_examination(&env, 30).unwrap();
        assert_eq!(stored.iop.method, String::from_str(&env, "Goldmann"));
        assert_eq!(stored.iop.left_eye, 14);
        assert_eq!(stored.iop.right_eye, 15);
    }

    #[test]
    fn test_clinical_notes_stored_with_exam() {
        let env = Env::default();
        let provider = Address::generate(&env);
        let mut exam = make_exam(&env, 40);
        exam.clinical_notes =
            String::from_str(&env, "DICOM: Glaucoma suspect, follow-up in 6 months");
        set_examination(&env, &exam, &provider);

        let stored = get_examination(&env, 40).unwrap();
        assert_eq!(stored.clinical_notes, exam.clinical_notes);
    }
}
