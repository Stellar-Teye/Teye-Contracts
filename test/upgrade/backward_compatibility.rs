
#[cfg(test)]
mod backward_compat_tests {
    use soroban_sdk::{Bytes, Env, Map, Symbol};

    use common::{
        dry_run_migration, initialize_default_migrations,
        lazy_read, migrate_forward, migrate_rollback,
        read_record, write_record,
        set_stored_version, stored_version,
        global_version, bump_global_version,
        MigrationError, CURRENT_VERSION,
    };

    // ─────────────────────────────────────────────────────────
    // Helpers
    // ─────────────────────────────────────────────────────────

    fn make_env() -> Env {
        Env::default()
    }

    fn v1_record(env: &Env) -> Map<Symbol, Bytes> {
        let mut m = Map::new(env);
        m.set(Symbol::new(env, "patient_id"),  Bytes::from_slice(env, b"P999"));
        m.set(Symbol::new(env, "exam_date"),   Bytes::from_slice(env, b"2023-06-01"));
        m.set(Symbol::new(env, "raw_notes"),   Bytes::from_slice(env, b"Baseline exam"));
        m
    }

    fn v2_record(env: &Env) -> Map<Symbol, Bytes> {
        let mut m = v1_record(env);
        m.set(Symbol::new(env, "iop_value"), Bytes::from_slice(env, b"14"));
        m
    }

    fn v1_record_with(
        env:        &Env,
        patient_id: &[u8],
        iop_raw:    &[u8],
        notes:      &[u8],
    ) -> Map<Symbol, Bytes> {
        let mut m = Map::new(env);
        m.set(Symbol::new(env, "patient_id"), Bytes::from_slice(env, patient_id));
        m.set(Symbol::new(env, "exam_date"),  Bytes::from_slice(env, b"2023-09-15"));
        m.set(Symbol::new(env, "raw_notes"),  Bytes::from_slice(env, notes));
        m.set(Symbol::new(env, "iop_value"),  Bytes::from_slice(env, iop_raw));
        m
    }


    // ─────────────────────────────────────────────────────────
    // 1. v1 data is readable after upgrade
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_v1_data_readable_after_global_upgrade() {
        let env = make_env();
        initialize_default_migrations(&env);

        write_record(&env, 1, v1_record(&env), 1).unwrap();

        set_stored_version(&env, CURRENT_VERSION);

        let vr = read_record(&env, 1).unwrap().expect("record must exist");
        assert_eq!(vr.version, CURRENT_VERSION);
        assert!(vr.data.contains_key(Symbol::new(&env, "ai_flag")));
        assert!(vr.data.contains_key(Symbol::new(&env, "clinical_notes")));
    }

    #[test]
    fn test_v2_data_readable_after_global_upgrade_to_v3() {
        let env = make_env();
        initialize_default_migrations(&env);

        write_record(&env, 2, v2_record(&env), 2).unwrap();
        set_stored_version(&env, CURRENT_VERSION);

        let vr = read_record(&env, 2).unwrap().expect("record must exist");
        assert_eq!(vr.version, CURRENT_VERSION);
        // iop_value preserved
        let iop = vr.data.get(Symbol::new(&env, "iop_value")).unwrap();
        assert_eq!(iop, Bytes::from_slice(&env, b"14"));
    }

    // ─────────────────────────────────────────────────────────
    // 2. Zero-downtime: in-flight reads succeed during version bump
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_zero_downtime_upgrade_reads_succeed_mid_migration() {
        let env = make_env();
        initialize_default_migrations(&env);

        for id in 1u64..=5 {
            write_record(&env, id, v1_record(&env), 1).unwrap();
        }

        set_stored_version(&env, CURRENT_VERSION);

        for id in 1u64..=5 {
            let vr = read_record(&env, id)
                .expect("read must succeed")
                .expect("record must exist");
            assert_eq!(vr.version, CURRENT_VERSION,
                "Record {id} not migrated to current version");
        }
    }

    // ─────────────────────────────────────────────────────────
    // 3. Rollback restores exact v1 data shape
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_rollback_restores_v1_shape_exactly() {
        let env = make_env();
        initialize_default_migrations(&env);

        let original = v1_record(&env);
        let mut record = original.clone();

        migrate_forward(&env, &mut record, 1, 3).unwrap();

        migrate_rollback(&env, &mut record, 3, 1).unwrap();

        assert_eq!(
            record.get(Symbol::new(&env, "patient_id")),
            original.get(Symbol::new(&env, "patient_id"))
        );
        assert_eq!(
            record.get(Symbol::new(&env, "exam_date")),
            original.get(Symbol::new(&env, "exam_date"))
        );
        assert_eq!(
            record.get(Symbol::new(&env, "raw_notes")),
            original.get(Symbol::new(&env, "raw_notes"))
        );

        assert!(!record.contains_key(Symbol::new(&env, "iop_value")));
        assert!(!record.contains_key(Symbol::new(&env, "ai_flag")));
        assert!(!record.contains_key(Symbol::new(&env, "clinical_notes")));
    }

    // ─────────────────────────────────────────────────────────
    // 4. Dry-run validation catches incompatible changes
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_dry_run_catches_incompatible_record() {
        let env = make_env();
        initialize_default_migrations(&env);

        // Record missing required exam_date field
        let mut bad_record = Map::new(&env);
        bad_record.set(
            Symbol::new(&env, "patient_id"),
            Bytes::from_slice(&env, b"P_BAD"),
        );

        let result = dry_run_migration(&env, &bad_record, 1, 2);
        assert_eq!(result, Err(MigrationError::ValidationFailed));
    }

    // ─────────────────────────────────────────────────────────
    // 5. Global version bump helper
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_bump_global_version_increments() {
        let env = make_env();
        set_stored_version(&env, 1);

        bump_global_version(&env);
        assert_eq!(global_version(&env), 2);

        bump_global_version(&env);
        assert_eq!(global_version(&env), 3);

        // Cannot exceed CURRENT_VERSION
        bump_global_version(&env);
        assert_eq!(global_version(&env), CURRENT_VERSION); // still 3
    }

    // ─────────────────────────────────────────────────────────
    // 6. Integration: write v1 → read back as v3 → rollback
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_full_lifecycle_write_upgrade_rollback() {
        let env = make_env();
        initialize_default_migrations(&env);

        // Step 1: write a v1 record
        write_record(&env, 42, v1_record(&env), 1).unwrap();

        // Step 2: upgrade the global schema
        set_stored_version(&env, 3);

        // Step 3: read back (lazy migrate to v3)
        let vr = read_record(&env, 42).unwrap().unwrap();
        assert_eq!(vr.version, 3);

        // Step 4: write back to persist the upgraded data
        write_record(&env, 42, vr.data.clone(), vr.version).unwrap();

        // Step 5: rollback to v1
        let mut data = vr.data;
        migrate_rollback(&env, &mut data, 3, 1).unwrap();

        // Confirm v1 shape
        assert!(data.contains_key(Symbol::new(&env, "raw_notes")));
        assert!(!data.contains_key(Symbol::new(&env, "ai_flag")));
    }

    // ─────────────────────────────────────────────────────────
    // 7. Backward-compatible deserialisation fallback
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_backward_compatibility_deserialization() {
        let env = make_env();
        initialize_default_migrations(&env);

        let owner   = b"bob";
        let balance = b"500";
        let notes   = b"initial checkup";
        write_record(&env, 10, v1_record_with(&env, owner, balance, notes), 1).unwrap();

        set_stored_version(&env, CURRENT_VERSION);
        let vr = read_record(&env, 10)
            .expect("read must not error")
            .expect("record must exist");

        assert_eq!(vr.version, CURRENT_VERSION);

        // ── owner equivalent ─────────────────────────────────────────────────
        assert_eq!(
            vr.data.get(Symbol::new(&env, "patient_id")).unwrap(),
            Bytes::from_slice(&env, owner),
            "patient_id (stub: owner) must be preserved",
        );

        // ── balance equivalent ───────────────────────────────────────────────
        assert_eq!(
            vr.data.get(Symbol::new(&env, "iop_value")).unwrap(),
            Bytes::from_slice(&env, balance),
            "iop_value (stub: balance) must be preserved",
        );
        assert_eq!(
            vr.data.get(Symbol::new(&env, "ai_flag")).unwrap(),
            Bytes::from_slice(&env, b"false"),
            "ai_flag must default to b"false" when introduced by migration",
        );
        assert_eq!(
            vr.data.get(Symbol::new(&env, "clinical_notes")).unwrap(),
            Bytes::from_slice(&env, notes),
        );
    }

    #[test]
    fn test_backward_compatibility_v2_record_field_values() {
        let env = make_env();
        initialize_default_migrations(&env);

        let owner   = b"bob";
        let balance = b"500";
        let notes   = b"follow-up";

        let mut rec = v1_record_with(&env, owner, balance, notes);
        write_record(&env, 11, rec.clone(), 2).unwrap();

        set_stored_version(&env, CURRENT_VERSION);

        let vr = read_record(&env, 11)
            .unwrap()
            .expect("record must exist");

        assert_eq!(vr.version, CURRENT_VERSION);
        assert_eq!(
            vr.data.get(Symbol::new(&env, "patient_id")).unwrap(),
            Bytes::from_slice(&env, owner),
        );
        assert_eq!(
            vr.data.get(Symbol::new(&env, "iop_value")).unwrap(),
            Bytes::from_slice(&env, balance),
        );
        assert_eq!(
            vr.data.get(Symbol::new(&env, "ai_flag")).unwrap(),
            Bytes::from_slice(&env, b"false"),
        );
    }

    // ─────────────────────────────────────────────────────────
    // 8. Mixed-version record pool
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_mixed_version_pool_all_readable_after_upgrade() {
        let env = make_env();
        initialize_default_migrations(&env);
        write_record(&env, 20, v1_record_with(&env, b"carol", b"12", b"v1 exam"), 1).unwrap();
        write_record(&env, 21, v1_record_with(&env, b"dave", b"16", b"v2 exam"), 2).unwrap();
        set_stored_version(&env, CURRENT_VERSION);
        let vr20 = read_record(&env, 20).unwrap().expect("record 20 must exist");
        assert_eq!(vr20.version, CURRENT_VERSION);
        assert_eq!(
            vr20.data.get(Symbol::new(&env, "patient_id")).unwrap(),
            Bytes::from_slice(&env, b"carol"),
        );
        assert_eq!(
            vr20.data.get(Symbol::new(&env, "iop_value")).unwrap(),
            Bytes::from_slice(&env, b"12"),
        );

        let vr21 = read_record(&env, 21).unwrap().expect("record 21 must exist");
        assert_eq!(vr21.version, CURRENT_VERSION);
        assert_eq!(
            vr21.data.get(Symbol::new(&env, "patient_id")).unwrap(),
            Bytes::from_slice(&env, b"dave"),
        );
        assert_eq!(
            vr21.data.get(Symbol::new(&env, "iop_value")).unwrap(),
            Bytes::from_slice(&env, b"16"),
        );
    }

    // ─────────────────────────────────────────────────────────
    // 9. Storage-layer completeness
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_read_record_returns_none_for_missing_id() {
        let env = make_env();
        initialize_default_migrations(&env);
        let result = read_record(&env, 999).expect("read must not error");
        assert!(result.is_none(), "non-existent record must return None");
    }

    #[test]
    fn test_delete_record_makes_record_unreadable() {
        use common::delete_record;

        let env = make_env();
        initialize_default_migrations(&env);

        write_record(&env, 30, v1_record(&env), 1).unwrap();
        assert!(read_record(&env, 30).unwrap().is_some());
        delete_record(&env, 30);
        let after = read_record(&env, 30).expect("read after delete must not error");
        assert!(after.is_none(), "deleted record must return None");
    }

    #[test]
    fn test_overwrite_record_replaces_values() {
        let env = make_env();
        initialize_default_migrations(&env);
        write_record(&env, 40, v1_record_with(&env, b"eve", b"10", b"first"), 1).unwrap();
        write_record(&env, 40, v1_record_with(&env, b"eve", b"22", b"revised"), 1).unwrap();

        set_stored_version(&env, CURRENT_VERSION);

        let vr = read_record(&env, 40).unwrap().expect("record must exist");
        assert_eq!(
            vr.data.get(Symbol::new(&env, "iop_value")).unwrap(),
            Bytes::from_slice(&env, b"22"),
            "overwritten iop_value must be the second value",
        );
        assert_eq!(
            vr.data.get(Symbol::new(&env, "clinical_notes")).unwrap(),
            Bytes::from_slice(&env, b"revised"),
            "overwritten notes must be the second value",
        );
    }

}