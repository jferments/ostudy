    #[test]
    fn corrupt_referenced_file_is_marked_missing_without_blocking_startup() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("corrupt-reference");
        database
            .with_connection(|connection| {
                let now = unix_time_millis()?;
                let asset_id = generate_id(connection)?;
                let relative = AppPaths::image_relative_path(&asset_id, "png");
                let corrupt = vec![b'x'; png_bytes().len()];
                fs::write(paths.resolve_relative(&relative), &corrupt).map_err(CommandError::io)?;
                connection
                    .execute(
                        "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size, relative_path,
                         created_at, lifecycle_state, ready_at, verified_at, updated_at
                     ) VALUES (?1, 'corrupt.png', 'image/png', ?2, ?3, ?4,
                               'ready', ?4, ?4, ?4)",
                        params![asset_id, corrupt.len() as i64, relative, now],
                    )
                    .map_err(CommandError::database)?;
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Corrupt".to_string(),
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
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1,
                               '{}', '{}', 'Q', 'A', '', ?3, ?3, 1)",
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

                let first = reconcile_in_connection(connection, &paths, now + 1)?;
                assert!(first.is_empty());
                let state: String = connection
                    .query_row(
                        "SELECT lifecycle_state FROM assets WHERE id = ?1",
                        [asset_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(state, "missing");
                assert!(paths.resolve_relative(&relative).exists());
                let event_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM asset_reconciliation_events
                     WHERE asset_id = ?1 AND action = 'missing'",
                        [asset_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(event_count, 1);

                let second = reconcile_in_connection(connection, &paths, now + 2)?;
                assert!(second.is_empty());
                let repeated_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM asset_reconciliation_events
                     WHERE asset_id = ?1 AND action = 'missing'",
                        [asset_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(repeated_count, event_count);
                assert!(paths.resolve_relative(&relative).exists());
                Ok(())
            })
            .expect("corrupt reference should not abort reconciliation");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }
