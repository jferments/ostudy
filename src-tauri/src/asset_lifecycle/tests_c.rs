    #[test]
    fn ordinary_multi_asset_failure_returns_prior_files_to_staging() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("promotion-compensation");
        let source = source_image(&paths, "source.png");
        database
            .with_connection(|connection| {
                let draft = begin_draft_in_connection(connection, "card", None)?;
                let first = stage_source_in_connection(
                    connection,
                    &paths,
                    &source,
                    &draft.id,
                    Default::default(),
                )?;
                let second = stage_source_in_connection(
                    connection,
                    &paths,
                    &source,
                    &draft.id,
                    Default::default(),
                )?;
                let mut ids = vec![first.id.clone(), second.id.clone()];
                ids.sort();
                let first_record = load_asset(connection, &ids[0])?;
                let second_record = load_asset(connection, &ids[1])?;
                let first_staged = paths.resolve_relative(
                    first_record
                        .staged_relative_path
                        .as_deref()
                        .expect("first staged path"),
                );
                let first_final = paths.resolve_relative(&first_record.relative_path);
                let second_final = paths.resolve_relative(&second_record.relative_path);
                fs::write(&second_final, png_bytes()).map_err(CommandError::io)?;

                let transaction = connection.transaction().map_err(CommandError::database)?;
                let error = finalize_assets_with_faults(
                    &transaction,
                    &paths,
                    &ids,
                    Some(&draft.id),
                    None,
                    Default::default(),
                )
                .expect_err("second final path should conflict");
                assert_eq!(error.code, "asset_path_conflict");
                drop(transaction);

                assert!(first_staged.exists());
                assert!(!first_final.exists());
                assert!(second_final.exists());
                Ok(())
            })
            .expect("ordinary failure should compensate prior renames");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

    #[test]
    fn document_startup_reconciliation_restores_referenced_mp3_asset() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("referenced-mp3");
        database
            .with_connection(|connection| {
                let now = unix_time_millis()?;
                let asset_id = generate_id(connection)?;
                let relative = AppPaths::image_relative_path(&asset_id, "mp3");
                let bytes = mp3_bytes();
                fs::write(paths.resolve_relative(&relative), &bytes).map_err(CommandError::io)?;
                connection
                    .execute(
                        "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size, relative_path,
                         created_at, lifecycle_state, ready_at, verified_at, updated_at,
                         lifecycle_reason
                     ) VALUES (?1, 'lecture.mp3', 'audio/mpeg', ?2, ?3, ?4,
                               'missing', ?4, ?4, ?4, 'legacy image-only verification failure')",
                        params![asset_id, bytes.len() as i64, relative, now],
                    )
                    .map_err(CommandError::database)?;
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Audio".to_string(),
                        description: None,
                    },
                )?;
                let note_id = generate_id(connection)?;
                connection
                    .execute(
                        "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text, duplicate_hash,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 3,
                               '{}', '{}', '[Audio]', 'Answer', '', ?3, ?3, 1)",
                        params![note_id, deck.id, now],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO note_assets (note_id, asset_id, side)
                     VALUES (?1, ?2, 'front')",
                        params![note_id, asset_id],
                    )
                    .map_err(CommandError::database)?;

                let cleanup = reconcile_in_connection(connection, &paths, now + 1)?;
                assert!(cleanup.is_empty());
                let (state, reason, verified_at): (String, String, Option<i64>) = connection
                    .query_row(
                        "SELECT lifecycle_state, lifecycle_reason, verified_at
                         FROM assets WHERE id = ?1",
                        [asset_id.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(state, "ready");
                assert!(reason.is_empty());
                assert_eq!(verified_at, Some(now + 1));
                assert!(paths.resolve_relative(&relative).exists());
                let missing_events: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM asset_reconciliation_events
                         WHERE asset_id = ?1 AND action = 'missing'",
                        [asset_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(missing_events, 0);
                Ok(())
            })
            .expect("referenced MP3 should be restored during startup reconciliation");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

