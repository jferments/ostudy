    fn temp_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "ostudy-{label}-{}-{}",
            std::process::id(),
            unix_time_millis().expect("clock")
        ));
        AppPaths::new(root).expect("paths should initialize")
    }

    fn png_bytes() -> Vec<u8> {
        vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0]
    }

    fn mp3_bytes() -> Vec<u8> {
        let mut mp3 = vec![0_u8; 417 * 2];
        for offset in [0, 417] {
            mp3[offset..offset + 4].copy_from_slice(&[0xff, 0xfb, 0x90, 0x00]);
        }
        mp3
    }

    fn source_image(paths: &AppPaths, name: &str) -> PathBuf {
        let path = paths.app_data_dir().join(name);
        fs::write(&path, png_bytes()).expect("source should write");
        path
    }

    #[test]
    fn staged_ingest_cleans_copy_sync_and_database_failures() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("ingest-failures");
        let source = source_image(&paths, "source.png");
        database
            .with_connection(|connection| {
                let draft = begin_draft_in_connection(connection, "card", None)?;
                for faults in [
                    IngestFaults {
                        fail_after_copy: true,
                        ..Default::default()
                    },
                    IngestFaults {
                        fail_after_sync: true,
                        ..Default::default()
                    },
                    IngestFaults {
                        fail_before_insert: true,
                        ..Default::default()
                    },
                ] {
                    let error =
                        stage_source_in_connection(connection, &paths, &source, &draft.id, faults)
                            .expect_err("fault should fail");
                    assert!(error.code.starts_with("asset_test_"));
                }
                let count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(count, 0);
                assert_eq!(fs::read_dir(paths.asset_staging_dir()).unwrap().count(), 0);
                Ok(())
            })
            .expect("failure cleanup should be durable");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

    #[test]
    fn post_rename_failure_is_reconciled_without_losing_a_reference() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("post-rename");
        let source = source_image(&paths, "source.png");
        database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Assets".to_string(),
                        description: None,
                    },
                )?;
                let draft = begin_draft_in_connection(connection, "card", None)?;
                let asset = stage_source_in_connection(
                    connection,
                    &paths,
                    &source,
                    &draft.id,
                    IngestFaults::default(),
                )?;
                let transaction = connection.transaction().map_err(CommandError::database)?;
                let content = validate_content(
                    json!({"type":"doc","content":[{"type":"image","attrs":{"assetId":asset.id,"src":format!("study-asset://{}", asset.id)}}]}),
                    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Answer"}]}]}),
                    String::new(),
                    "Answer".to_string(),
                )?;
                let note_id = generate_id(&transaction)?;
                let card_id = generate_id(&transaction)?;
                transaction.execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text, duplicate_hash,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, '', 1, 1, 1)",
                    params![note_id, deck.id, content.front_doc_json, content.back_doc_json, content.front_text, content.back_text],
                ).map_err(CommandError::database)?;
                transaction.execute(
                    "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'forward', 0, 1, 1)",
                    params![card_id, note_id, deck.id],
                ).map_err(CommandError::database)?;
                insert_new_schedule(&transaction, &card_id, 1)?;
                transaction.execute(
                    "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'front')",
                    params![note_id, asset.id],
                ).map_err(CommandError::database)?;
                let error = finalize_assets_with_faults(
                    &transaction,
                    &paths,
                    std::slice::from_ref(&asset.id),
                    Some(&draft.id),
                    Some(&note_id),
                    PromotionFaults { fail_after_rename: true },
                ).expect_err("post-rename crash should be injected");
                assert_eq!(error.code, "asset_test_post_rename_crash");
                drop(transaction);

                // Recreate the durable reference to model a commit that completed
                // while the final lifecycle update was interrupted.
                connection.execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text, duplicate_hash,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, '', 2, 2, 1)",
                    params![note_id, deck.id, content.front_doc_json, content.back_doc_json, content.front_text, content.back_text],
                ).map_err(CommandError::database)?;
                connection.execute(
                    "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'front')",
                    params![note_id, asset.id],
                ).map_err(CommandError::database)?;
                let cleanup = reconcile_in_connection(connection, &paths, unix_time_millis()?)?;
                remove_paths_best_effort(&cleanup);
                let state: String = connection.query_row(
                    "SELECT lifecycle_state FROM assets WHERE id = ?1",
                    [asset.id.as_str()],
                    |row| row.get(0),
                ).map_err(CommandError::database)?;
                assert_eq!(state, "ready");
                assert!(paths.resolve_relative(&AppPaths::image_relative_path(&asset.id, "png")).exists());
                Ok(())
            })
            .expect("reconciliation should restore the final file");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

