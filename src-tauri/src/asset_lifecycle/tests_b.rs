    #[test]
    fn cancel_and_sync_remove_only_unreferenced_draft_assets() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("draft-cleanup");
        let source = source_image(&paths, "source.png");
        database
            .with_connection(|connection| {
                let draft = begin_draft_in_connection(connection, "card", None)?;
                let kept = stage_source_in_connection(
                    connection,
                    &paths,
                    &source,
                    &draft.id,
                    Default::default(),
                )?;
                let removed = stage_source_in_connection(
                    connection,
                    &paths,
                    &source,
                    &draft.id,
                    Default::default(),
                )?;
                let kept_path = paths.resolve_relative(&AppPaths::staged_asset_relative_path(
                    &draft.id, &kept.id, "png",
                ));
                let removed_path = paths.resolve_relative(&AppPaths::staged_asset_relative_path(
                    &draft.id,
                    &removed.id,
                    "png",
                ));
                let retained = BTreeSet::from([kept.id.clone()]);
                let transaction = connection.transaction().map_err(CommandError::database)?;
                let cleanup = retire_unused_draft_assets(
                    &transaction,
                    &paths,
                    &draft.id,
                    &retained,
                    unix_time_millis()?,
                    "test sync",
                )?;
                transaction.commit().map_err(CommandError::database)?;
                remove_paths_best_effort(&cleanup);
                assert!(kept_path.exists());
                assert!(!removed_path.exists());

                let cleanup =
                    abandon_draft_in_connection(connection, &paths, &draft.id, "test cancel")?;
                remove_paths_best_effort(&cleanup);
                assert!(!kept_path.exists());
                Ok(())
            })
            .expect("draft cleanup should succeed");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

    #[test]
    fn repeated_reconciliation_is_idempotent_and_protects_shared_references() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("reconcile-idempotent");
        database
            .with_connection(|connection| {
                let now = unix_time_millis()?;
                let asset_id = generate_id(connection)?;
                let relative = AppPaths::image_relative_path(&asset_id, "png");
                fs::write(paths.resolve_relative(&relative), png_bytes()).map_err(CommandError::io)?;
                connection.execute(
                    "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size, relative_path, created_at,
                         lifecycle_state, ready_at, verified_at, updated_at
                     ) VALUES (?1, 'shared.png', 'image/png', ?2, ?3, ?4, 'ready', ?4, ?4, ?4)",
                    params![asset_id, png_bytes().len() as i64, relative, now],
                ).map_err(CommandError::database)?;
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput { name: "Shared".to_string(), description: None },
                )?;
                for ordinal in 0..2 {
                    let note_id = generate_id(connection)?;
                    connection.execute(
                        "INSERT INTO notes (
                             id, deck_id, note_type, content_format, content_schema_version,
                             front_doc, back_doc, front_text, back_text, duplicate_hash,
                             created_at, updated_at, revision
                         ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, '{}', '{}', 'Q', 'A', '', ?3, ?3, 1)",
                        params![note_id, deck.id, now + ordinal],
                    ).map_err(CommandError::database)?;
                    connection.execute(
                        "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'front')",
                        params![note_id, asset_id],
                    ).map_err(CommandError::database)?;
                }
                let first = reconcile_in_connection(connection, &paths, now + 1)?;
                let event_count: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM asset_reconciliation_events",
                    [],
                    |row| row.get(0),
                ).map_err(CommandError::database)?;
                let second = reconcile_in_connection(connection, &paths, now + 2)?;
                let repeated_count: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM asset_reconciliation_events",
                    [],
                    |row| row.get(0),
                ).map_err(CommandError::database)?;
                assert!(first.is_empty());
                assert!(second.is_empty());
                assert_eq!(event_count, repeated_count);
                assert!(paths.resolve_relative(&relative).exists());
                Ok(())
            })
            .expect("shared referenced asset must remain ready");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

    #[test]
    fn unknown_and_unreferenced_files_are_quarantined_conservatively() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("orphan-quarantine");
        let orphan = paths.images_dir().join("unknown.png");
        fs::write(&orphan, png_bytes()).expect("orphan should write");
        database
            .with_connection(|connection| {
                reconcile_in_connection(connection, &paths, unix_time_millis()?)?;
                assert!(!orphan.exists());
                assert!(fs::read_dir(paths.asset_quarantine_dir()).unwrap().count() >= 1);
                let logged: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM asset_reconciliation_events WHERE action = 'orphan_quarantined'",
                    [],
                    |row| row.get(0),
                ).map_err(CommandError::database)?;
                assert_eq!(logged, 1);
                Ok(())
            })
            .expect("orphan should be quarantined");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

