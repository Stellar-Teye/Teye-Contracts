
#[cfg(test)]
mod migration_tests {
    use soroban_sdk::{
        testutils::Address as _, Address, Bytes, Env, Map, Symbol, Vec,
    };

    use common::{
        dry_run_migration, initialize_default_migrations, lazy_read, lazy_write,
        migrate_forward, migrate_rollback, register_migration,
        resolve_version_for_caller, set_canary, set_stored_version, stored_version,
        FieldTransform, Migration, MigrationError, SchemaVersion,
        CURRENT_VERSION,
    };

    // ──────────────────────────────────────────────────────────
    // Helpers
    // ──────────────────────────────────────────────────────────

    fn make_env() -> Env {
        Env::default()
    }

    /// Build a minimal v1 record with only the required v1 fields.
    fn v1_record(env: &Env) -> Map<Symbol, Bytes> {
        let mut m = Map::new(env);
        m.set(
            Symbol::new(env, "patient_id"),
            Bytes::from_slice(env, b"P001"),
        );
        m.set(
            Symbol::new(env, "exam_date"),
            Bytes::from_slice(env, b"2024-01-01"),
        );
        m.set(
            Symbol::new(env, "raw_notes"),
            Bytes::from_slice(env, b"Initial exam"),
        );
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
        m.set(Symbol::new(env, "exam_date"),  Bytes::from_slice(env, b"2024-06-01"));
        m.set(Symbol::new(env, "raw_notes"),  Bytes::from_slice(env, notes));
        m.set(Symbol::new(env, "iop_value"),  Bytes::from_slice(env, iop_raw));
        m
    }


    // ──────────────────────────────────────────────────────────
    // 1. Schema version is tracked and enforced
    // ──────────────────────────────────────────────────────────

    #[test]
    fn test_version_tracking_initialized_to_zero() {
        let env = make_env();
        // Before initialization, version is 0
        assert_eq!(stored_version(&env), 0);
    }

    #[test]
    fn test_initialize_sets_version_one() {
        let env = make_env();
        initialize_default_migrations(&env);
        assert_eq!(stored_version(&env), 1);
    }

    #[test]
    fn test_set_stored_version() {
        let env = make_env();
        set_stored_version(&env, 2);
        assert_eq!(stored_version(&env), 2);
    }

    #[test]
    fn test_version_too_new_rejected() {
        let env = make_env();
        initialize_default_migrations(&env);
        let mut record = v1_record(&env);
        // Attempt to migrate from a future version — should fail.
        let result = migrate_forward(&env, &mut record, 999, 1000);
        assert_eq!(result, Err(MigrationError::VersionTooNew));
    }

    // ──────────────────────────────────────────────────────────
    // 2. Forward migration (v1 → v2 → v3)
    // ──────────────────────────────────────────────────────────

    #[test]
    fn test_forward_v1_to_v2() {
        let env = make_env();
        initialize_default_migrations(&env);
        let mut record = v1_record(&env);

        let result = migrate_forward(&env, &mut record, 1, 2);
        assert_eq!(result, Ok(2));

        // iop_value should have been added with default "0"
        let iop = record.get(Symbol::new(&env, "iop_value")).unwrap();
        assert_eq!(iop, Bytes::from_slice(&env, b"0"));
    }

    #[test]
    fn test_forward_v1_to_v3_multistep() {
        let env = make_env();
        initialize_default_migrations(&env);
        let mut record = v1_record(&env);

        let result = migrate_forward(&env, &mut record, 1, 3);
        assert_eq!(result, Ok(3));

        // v2 field added
        assert!(record.contains_key(Symbol::new(&env, "iop_value")));
        // v3 field added
        assert!(record.contains_key(Symbol::new(&env, "ai_flag")));
        // v2→v3 renamed raw_notes to clinical_notes
        assert!(record.contains_key(Symbol::new(&env, "clinical_notes")));
        assert!(!record.contains_key(Symbol::new(&env, "raw_notes")));
    }

    #[test]
    fn test_forward_same_version_is_noop() {
        let env = make_env();
        initialize_default_migrations(&env);
        let mut record = v1_record(&env);

        let result = migrate_forward(&env, &mut record, 2, 2);
        assert_eq!(result, Ok(2));
    }

    // ──────────────────────────────────────────────────────────
    // 3. Lazy migration on read
    // ──────────────────────────────────────────────────────────

    #[test]
    fn test_lazy_read_migrates_to_current() {
        let env = make_env();
        initialize_default_migrations(&env);
        let mut record = v1_record(&env);

        let new_ver = lazy_read(&env, &mut record, 1).unwrap();
        assert_eq!(new_ver, CURRENT_VERSION);
        assert!(record.contains_key(Symbol::new(&env, "ai_flag")));
    }

    #[test]
    fn test_lazy_read_already_current_is_noop() {
        let env = make_env();
        initialize_default_migrations(&env);
        let mut record = v1_record(&env);
        // Pre-migrate to current
        migrate_forward(&env, &mut record, 1, CURRENT_VERSION).unwrap();

        let snapshot = record.clone();
        let new_ver = lazy_read(&env, &mut record, CURRENT_VERSION).unwrap();
        assert_eq!(new_ver, CURRENT_VERSION);
        // Data unchanged
        assert_eq!(record.len(), snapshot.len());
    }

    // ──────────────────────────────────────────────────────────
    // 4. Rollback — restores previous schema and data format
    // ──────────────────────────────────────────────────────────

    #[test]
    fn test_rollback_v3_to_v2() {
        let env = make_env();
        initialize_default_migrations(&env);
        let mut record = v1_record(&env);

        // Migrate forward to v3
        migrate_forward(&env, &mut record, 1, 3).unwrap();
        assert!(record.contains_key(Symbol::new(&env, "ai_flag")));
        assert!(record.contains_key(Symbol::new(&env, "clinical_notes")));

        // Roll back to v2
        let result = migrate_rollback(&env, &mut record, 3, 2);
        assert_eq!(result, Ok(2));

        // ai_flag and clinical_notes removed, raw_notes restored
        assert!(!record.contains_key(Symbol::new(&env, "ai_flag")));
        assert!(!record.contains_key(Symbol::new(&env, "clinical_notes")));
        assert!(record.contains_key(Symbol::new(&env, "raw_notes")));
    }

    #[test]
    fn test_rollback_multistep_v3_to_v1() {
        let env = make_env();
        initialize_default_migrations(&env);
        let mut record = v1_record(&env);

        migrate_forward(&env, &mut record, 1, 3).unwrap();

        let result = migrate_rollback(&env, &mut record, 3, 1);
        assert_eq!(result, Ok(1));

        // Back to v1 shape: no iop_value, no ai_flag, original raw_notes
        assert!(!record.contains_key(Symbol::new(&env, "iop_value")));
        assert!(!record.contains_key(Symbol::new(&env, "ai_flag")));
        assert!(record.contains_key(Symbol::new(&env, "raw_notes")));
    }

    #[test]
    fn test_rollback_below_minimum_fails() {
        let env = make_env();
        initialize_default_migrations(&env);
        let mut record = v1_record(&env);
        migrate_forward(&env, &mut record, 1, 3).unwrap();

        // Version 0 is below MINIMUM_SUPPORTED_VERSION
        let result = migrate_rollback(&env, &mut record, 3, 0);
        assert_eq!(result, Err(MigrationError::RollbackUnavailable));
    }

    // ──────────────────────────────────────────────────────────
    // 5. Pre-migration validation (dry-run)
    // ──────────────────────────────────────────────────────────

    #[test]
    fn test_dry_run_success() {
        let env = make_env();
        initialize_default_migrations(&env);
        let record = v1_record(&env);

        let result = dry_run_migration(&env, &record, 1, 3);
        assert_eq!(result, Ok(3));

        // Original record is UNCHANGED (dry-run should not mutate)
        assert!(!record.contains_key(Symbol::new(&env, "ai_flag")));
    }

    #[test]
    fn test_dry_run_catches_missing_path() {
        let env = make_env();
        // Do NOT initialize default migrations — registry is empty
        let record = v1_record(&env);

        let result = dry_run_migration(&env, &record, 1, 2);
        assert_eq!(result, Err(MigrationError::NoMigrationPath));
    }

    // ──────────────────────────────────────────────────────────
    // 6. Migration DSL — individual transforms
    // ──────────────────────────────────────────────────────────

    #[test]
    fn test_rename_field_transform() {
        let env = make_env();
        initialize_default_migrations(&env);

        // Register a custom migration v3 → v4 for testing
        let migration = Migration {
            from_version: 3,
            to_version: 4,
            description: soroban_sdk::String::from_str(&env, "rename test"),
            forward: {
                let mut v = Vec::new(&env);
                v.push_back(FieldTransform::RenameField {
                    old_key: Symbol::new(&env, "patient_id"),
                    new_key: Symbol::new(&env, "pid"),
                });
                v
            },
            reverse: {
                let mut v = Vec::new(&env);
                v.push_back(FieldTransform::RenameField {
                    old_key: Symbol::new(&env, "pid"),
                    new_key: Symbol::new(&env, "patient_id"),
                });
                v
            },
        };

        register_migration(&env, migration).unwrap();

        let mut record = v1_record(&env);
        migrate_forward(&env, &mut record, 1, 3).unwrap(); // bring to v3

        // Now apply our custom step
        let result = migrate_forward(&env, &mut record, 3, 4);
        assert_eq!(result, Ok(4));
        assert!(record.contains_key(Symbol::new(&env, "pid")));
        assert!(!record.contains_key(Symbol::new(&env, "patient_id")));
    }

    #[test]
    fn test_add_field_transform_does_not_overwrite() {
        let env = make_env();
        initialize_default_migrations(&env);

        let mut record = v1_record(&env);
        // Manually set iop_value before migrating
        record.set(
            Symbol::new(&env, "iop_value"),
            Bytes::from_slice(&env, b"21"),
        );

        // Migrate v1→v2: AddField should NOT overwrite existing value
        migrate_forward(&env, &mut record, 1, 2).unwrap();
        let iop = record.get(Symbol::new(&env, "iop_value")).unwrap();
        assert_eq!(iop, Bytes::from_slice(&env, b"21"));
    }

    #[test]
    fn test_remove_field_transform() {
        let env = make_env();
        initialize_default_migrations(&env);

        // Register v3→v4 that drops clinical_notes
        let migration = Migration {
            from_version: 3,
            to_version: 4,
            description: soroban_sdk::String::from_str(&env, "remove clinical_notes"),
            forward: {
                let mut v = Vec::new(&env);
                v.push_back(FieldTransform::RemoveField {
                    key: Symbol::new(&env, "clinical_notes"),
                });
                v
            },
            reverse: {
                let mut v = Vec::new(&env);
                // Reverse: restore with a sentinel value
                v.push_back(FieldTransform::AddField {
                    key: Symbol::new(&env, "clinical_notes"),
                    default_value: Bytes::from_slice(&env, b""),
                });
                v
            },
        };
        register_migration(&env, migration).unwrap();

        let mut record = v1_record(&env);
        migrate_forward(&env, &mut record, 1, 3).unwrap();
        assert!(record.contains_key(Symbol::new(&env, "clinical_notes")));

        migrate_forward(&env, &mut record, 3, 4).unwrap();
        assert!(!record.contains_key(Symbol::new(&env, "clinical_notes")));
    }

    #[test]
    fn test_copy_field_transform_leaves_source_intact() {
        let env = make_env();
        initialize_default_migrations(&env);

        // Register v3→v4 that copies patient_id → pid (non-destructive)
        let migration = Migration {
            from_version: 3,
            to_version: 4,
            description: soroban_sdk::String::from_str(&env, "copy patient_id to pid"),
            forward: {
                let mut v = Vec::new(&env);
                v.push_back(FieldTransform::CopyField {
                    source_key: Symbol::new(&env, "patient_id"),
                    dest_key:   Symbol::new(&env, "pid"),
                });
                v
            },
            reverse: {
                let mut v = Vec::new(&env);
                v.push_back(FieldTransform::RemoveField {
                    key: Symbol::new(&env, "pid"),
                });
                v
            },
        };
        register_migration(&env, migration).unwrap();

        let mut record = v1_record(&env);
        migrate_forward(&env, &mut record, 1, 3).unwrap();
        migrate_forward(&env, &mut record, 3, 4).unwrap();

        // Both source and dest must exist with identical values
        let src = record.get(Symbol::new(&env, "patient_id")).unwrap();
        let dst = record.get(Symbol::new(&env, "pid")).unwrap();
        assert_eq!(src, dst);
    }

    #[test]
    fn test_change_type_transform_requires_field_present() {
        let env = make_env();
        initialize_default_migrations(&env);

        // Register v3→v4 with a ChangeType transform on iop_value
        let migration = Migration {
            from_version: 3,
            to_version: 4,
            description: soroban_sdk::String::from_str(&env, "change iop_value type"),
            forward: {
                let mut v = Vec::new(&env);
                v.push_back(FieldTransform::ChangeType {
                    key:            Symbol::new(&env, "iop_value"),
                    transform_name: Symbol::new(&env, "to_u32"),
                });
                v
            },
            reverse: Vec::new(&env),
        };
        register_migration(&env, migration).unwrap();

        // Record migrated to v3 has iop_value — ChangeType should succeed
        let mut record = v1_record(&env);
        migrate_forward(&env, &mut record, 1, 3).unwrap();
        let result = migrate_forward(&env, &mut record, 3, 4);
        assert_eq!(result, Ok(4));
    }

    #[test]
    fn test_change_type_transform_fails_when_field_absent() {
        let env = make_env();
        initialize_default_migrations(&env);

        // Register v3→v4 with ChangeType on a field that does NOT exist
        let migration = Migration {
            from_version: 3,
            to_version: 4,
            description: soroban_sdk::String::from_str(&env, "change missing field type"),
            forward: {
                let mut v = Vec::new(&env);
                v.push_back(FieldTransform::ChangeType {
                    key:            Symbol::new(&env, "nonexistent"),
                    transform_name: Symbol::new(&env, "to_u32"),
                });
                v
            },
            reverse: Vec::new(&env),
        };
        register_migration(&env, migration).unwrap();

        let mut record = v1_record(&env);
        migrate_forward(&env, &mut record, 1, 3).unwrap();
        let result = migrate_forward(&env, &mut record, 3, 4);
        assert_eq!(result, Err(MigrationError::TransformFailed));
    }

    #[test]
    fn test_lazy_write_migrates_to_current_version() {
        let env = make_env();
        initialize_default_migrations(&env);

        let mut record = v1_record(&env);
        // lazy_write must behave identically to lazy_read: upgrades in place
        let new_ver = lazy_write(&env, &mut record, 1).unwrap();
        assert_eq!(new_ver, CURRENT_VERSION);
        assert!(record.contains_key(Symbol::new(&env, "ai_flag")));
        assert!(record.contains_key(Symbol::new(&env, "clinical_notes")));
    }

    #[test]
    fn test_rollback_no_path_fails() {
        let env = make_env();
        // Only register v1→v2, leave v2→v3 absent
        let m = Migration {
            from_version: 1,
            to_version: 2,
            description: soroban_sdk::String::from_str(&env, "partial registry"),
            forward: {
                let mut v = Vec::new(&env);
                v.push_back(FieldTransform::AddField {
                    key:           Symbol::new(&env, "iop_value"),
                    default_value: Bytes::from_slice(&env, b"0"),
                });
                v
            },
            reverse: {
                let mut v = Vec::new(&env);
                v.push_back(FieldTransform::RemoveField {
                    key: Symbol::new(&env, "iop_value"),
                });
                v
            },
        };
        register_migration(&env, m).unwrap();

        // Manually construct a v2-shaped record
        let mut record = v1_record(&env);
        record.set(Symbol::new(&env, "iop_value"), Bytes::from_slice(&env, b"14"));

        // Rolling back from v2 to v1 should succeed (path exists)
        let result = migrate_rollback(&env, &mut record, 2, 1);
        assert_eq!(result, Ok(1));

        // Rolling back from v3 to v2 should fail (no v2→v3 step registered)
        let mut record2 = v1_record(&env);
        record2.set(Symbol::new(&env, "iop_value"),    Bytes::from_slice(&env, b"14"));
        record2.set(Symbol::new(&env, "clinical_notes"), Bytes::from_slice(&env, b"notes"));
        record2.set(Symbol::new(&env, "ai_flag"),      Bytes::from_slice(&env, b"false"));
        let result2 = migrate_rollback(&env, &mut record2, 3, 2);
        assert_eq!(result2, Err(MigrationError::RollbackUnavailable));
    }

    // ──────────────────────────────────────────────────────────
    // 7. Canary deployments
    // ──────────────────────────────────────────────────────────

    #[test]
    fn test_canary_zero_percent_always_stable() {
        let env = make_env();
        initialize_default_migrations(&env);
        set_stored_version(&env, 2);

        // 0% canary = everyone gets stable (v2)
        set_canary(&env, 0, 3).unwrap();

        for _ in 0..10 {
            let caller = Address::generate(&env);
            let ver = resolve_version_for_caller(&env, &caller);
            assert_eq!(ver, 2);
        }
    }

    #[test]
    fn test_canary_hundred_percent_all_new() {
        let env = make_env();
        initialize_default_migrations(&env);
        set_stored_version(&env, 2);

        // 100% canary = everyone gets new version (v3)
        set_canary(&env, 100, 3).unwrap();

        for _ in 0..10 {
            let caller = Address::generate(&env);
            let ver = resolve_version_for_caller(&env, &caller);
            assert_eq!(ver, 3);
        }
    }

    #[test]
    fn test_canary_invalid_percentage_rejected() {
        let env = make_env();
        let result = set_canary(&env, 101, 3);
        assert_eq!(result, Err(MigrationError::InvalidCanaryPercentage));
    }

    // ──────────────────────────────────────────────────────────
    // 8. Double-registration rejected
    // ──────────────────────────────────────────────────────────

    #[test]
    fn test_duplicate_migration_rejected() {
        let env = make_env();
        initialize_default_migrations(&env); // registers v1→v2, v2→v3

        // Try to register v1→v2 again
        let dup = Migration {
            from_version: 1,
            to_version: 2,
            description: soroban_sdk::String::from_str(&env, "duplicate"),
            forward: Vec::new(&env),
            reverse: Vec::new(&env),
        };
        let result = register_migration(&env, dup);
        assert_eq!(result, Err(MigrationError::AlreadyMigrated));
    }

    // ──────────────────────────────────────────────────────────
    // 9. Field-value preservation across migration steps
    // ──────────────────────────────────────────────────────────

    #[test]
    fn test_state_migration_correctness() {
        let env = make_env();
        initialize_default_migrations(&env);

        // Construct the "old state" — a v1 record with concrete, named values.
        let old_patient_id = b"alice";
        let old_iop        = b"1000";
        let old_notes      = b"baseline";
        let mut record = v1_record_with(&env, old_patient_id, old_iop, old_notes);

        // Apply the full v1 → CURRENT_VERSION migration chain.
        let new_ver = migrate_forward(&env, &mut record, 1, CURRENT_VERSION).unwrap();
        assert_eq!(new_ver, CURRENT_VERSION);

        // ── owner equivalent: patient_id must be unchanged ──────────────────
        assert_eq!(
            record.get(Symbol::new(&env, "patient_id")).unwrap(),
            Bytes::from_slice(&env, old_patient_id),
            "patient_id (stub: owner) must be preserved across migration",
        );

        assert_eq!(
            record.get(Symbol::new(&env, "iop_value")).unwrap(),
            Bytes::from_slice(&env, old_iop),
            "iop_value (stub: balance) must be preserved across migration",
        );

        assert_eq!(
            record.get(Symbol::new(&env, "ai_flag")).unwrap(),
            Bytes::from_slice(&env, b"false"),
            "ai_flag (stub: is_frozen) must default to b\"false\" after migration",
        );

        // ── notes content preserved under the renamed key ────────────────────
        assert_eq!(
            record.get(Symbol::new(&env, "clinical_notes")).unwrap(),
            Bytes::from_slice(&env, old_notes),
            "notes content must survive the raw_notes → clinical_notes rename",
        );
    }

    #[test]
    fn test_v1_to_v2_preserves_existing_fields() {
        let env = make_env();
        initialize_default_migrations(&env);

        let mut record = v1_record(&env);
        migrate_forward(&env, &mut record, 1, 2).unwrap();

        // All original v1 fields must be unchanged
        assert_eq!(
            record.get(Symbol::new(&env, "patient_id")).unwrap(),
            Bytes::from_slice(&env, b"P001"),
        );
        assert_eq!(
            record.get(Symbol::new(&env, "exam_date")).unwrap(),
            Bytes::from_slice(&env, b"2024-01-01"),
        );
        assert_eq!(
            record.get(Symbol::new(&env, "raw_notes")).unwrap(),
            Bytes::from_slice(&env, b"Initial exam"),
        );
        // Newly added field must carry the correct default
        assert_eq!(
            record.get(Symbol::new(&env, "iop_value")).unwrap(),
            Bytes::from_slice(&env, b"0"),
        );
    }

    #[test]
    fn test_v2_to_v3_preserves_existing_fields() {
        let env = make_env();
        initialize_default_migrations(&env);

        // Start from v1 and migrate to v2 first, with a real iop reading
        let mut record = v1_record(&env);
        record.set(
            Symbol::new(&env, "iop_value"),
            Bytes::from_slice(&env, b"18"),
        );
        migrate_forward(&env, &mut record, 1, 2).unwrap();

        // Now step to v3
        migrate_forward(&env, &mut record, 2, 3).unwrap();

        // patient_id, exam_date untouched
        assert_eq!(
            record.get(Symbol::new(&env, "patient_id")).unwrap(),
            Bytes::from_slice(&env, b"P001"),
        );
        assert_eq!(
            record.get(Symbol::new(&env, "exam_date")).unwrap(),
            Bytes::from_slice(&env, b"2024-01-01"),
        );
        // iop_value preserved (was explicitly set to "18" before v1→v2)
        assert_eq!(
            record.get(Symbol::new(&env, "iop_value")).unwrap(),
            Bytes::from_slice(&env, b"18"),
        );
        // raw_notes renamed to clinical_notes; value preserved
        assert_eq!(
            record.get(Symbol::new(&env, "clinical_notes")).unwrap(),
            Bytes::from_slice(&env, b"Initial exam"),
        );
        assert!(!record.contains_key(Symbol::new(&env, "raw_notes")));
        // New ai_flag carries the correct default
        assert_eq!(
            record.get(Symbol::new(&env, "ai_flag")).unwrap(),
            Bytes::from_slice(&env, b"false"),
        );
    }

    #[test]
    fn test_full_v1_to_v3_preserves_all_original_values() {
        let env = make_env();
        initialize_default_migrations(&env);

        let mut record = v1_record(&env);
        migrate_forward(&env, &mut record, 1, CURRENT_VERSION).unwrap();

        // Identity fields preserved
        assert_eq!(
            record.get(Symbol::new(&env, "patient_id")).unwrap(),
            Bytes::from_slice(&env, b"P001"),
            "patient_id must survive the full migration chain",
        );
        assert_eq!(
            record.get(Symbol::new(&env, "exam_date")).unwrap(),
            Bytes::from_slice(&env, b"2024-01-01"),
            "exam_date must survive the full migration chain",
        );
        // Notes content preserved under its new key name
        assert_eq!(
            record.get(Symbol::new(&env, "clinical_notes")).unwrap(),
            Bytes::from_slice(&env, b"Initial exam"),
            "notes content must survive the rename from raw_notes to clinical_notes",
        );
        // Default values on newly introduced fields are correct
        assert_eq!(
            record.get(Symbol::new(&env, "iop_value")).unwrap(),
            Bytes::from_slice(&env, b"0"),
            "iop_value default must be \"0\"",
        );
        assert_eq!(
            record.get(Symbol::new(&env, "ai_flag")).unwrap(),
            Bytes::from_slice(&env, b"false"),
            "ai_flag default must be \"false\"",
        );
    }

    #[test]
    fn test_rollback_v3_to_v1_restores_original_values() {
        let env = make_env();
        initialize_default_migrations(&env);

        // Upgrade then roll back; confirm round-trip fidelity
        let mut record = v1_record(&env);
        migrate_forward(&env, &mut record, 1, CURRENT_VERSION).unwrap();
        migrate_rollback(&env, &mut record, CURRENT_VERSION, 1).unwrap();

        assert_eq!(
            record.get(Symbol::new(&env, "patient_id")).unwrap(),
            Bytes::from_slice(&env, b"P001"),
        );
        assert_eq!(
            record.get(Symbol::new(&env, "exam_date")).unwrap(),
            Bytes::from_slice(&env, b"2024-01-01"),
        );
        assert_eq!(
            record.get(Symbol::new(&env, "raw_notes")).unwrap(),
            Bytes::from_slice(&env, b"Initial exam"),
            "raw_notes value must be restored after rolling back the rename",
        );
        // Introduced fields must be gone after rollback
        assert!(!record.contains_key(Symbol::new(&env, "iop_value")));
        assert!(!record.contains_key(Symbol::new(&env, "ai_flag")));
        assert!(!record.contains_key(Symbol::new(&env, "clinical_notes")));
    }
}