use rusqlite::{params, Connection};

use crate::{
    asset_lifecycle,
    data_repairs::{apply_required_repairs, verify_repair_ledger},
    error::CommandError,
    paths::AppPaths,
};

const PRE_CANONICAL_MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_decks.sql"),
    include_str!("../migrations/0002_cards_assets_settings_practice.sql"),
    include_str!("../migrations/0003_fsrs_reviews.sql"),
    include_str!("../migrations/0004_card_organization.sql"),
    include_str!("../migrations/0005_remove_legacy_duplicate_trigger.sql"),
    include_str!("../migrations/0006_card_types_and_templates.sql"),
    include_str!("../migrations/0007_rich_templates_single_cloze.sql"),
    include_str!("../migrations/0008_scheduling_controls.sql"),
    include_str!("../migrations/0009_operation_boundaries.sql"),
    include_str!("../migrations/0010_transactional_schedule_actions.sql"),
    include_str!("../migrations/0011_managed_asset_lifecycle.sql"),
    include_str!("../migrations/0012_versioned_data_repairs.sql"),
];

const CANONICAL_MIGRATION: &str = include_str!("../migrations/0013_canonical_data_model.sql");

fn pre_canonical_database() -> Connection {
    let connection = Connection::open_in_memory().expect("database should open");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("foreign keys should enable");
    for migration in PRE_CANONICAL_MIGRATIONS {
        connection
            .execute_batch(migration)
            .expect("historical migration should apply");
    }
    connection
        .execute_batch("PRAGMA user_version = 12;")
        .expect("historical schema version should be recorded");
    connection
}

#[test]
fn versioned_repairs_migrate_legacy_rows_before_the_schema_is_narrowed() {
    let mut connection = pre_canonical_database();
    let deck_id = "1".repeat(32);
    let image_note_id = "2".repeat(32);
    let image_card_id = "3".repeat(32);
    let cloze_note_id = "4".repeat(32);
    let cloze_card_one = "5".repeat(32);
    let cloze_card_two = "6".repeat(32);
    let template_id = "7".repeat(32);
    let field_id = "8".repeat(32);
    let rule_id = "9".repeat(32);

    connection
        .execute(
            "INSERT INTO decks (
                 id, name, description, sort_order, created_at, updated_at, revision
             ) VALUES (?1, 'Legacy data', '', 0, 10, 10, 1)",
            [deck_id.as_str()],
        )
        .expect("deck should insert");
    connection
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text, duplicate_hash,
                 reverse_enabled, cloze_source, cloze_mode,
                 created_at, updated_at, revision
             ) VALUES (
                 ?1, ?2, 'image_occlusion', 'tiptap-json', 1,
                 ?3, ?4, 'Image question', 'Image answer', '',
                 0, '', 'one_at_a_time', 10, 10, 1
             )",
            params![
                image_note_id,
                deck_id,
                r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Image question"}]}]}"#,
                r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Image answer"}]}]}"#,
            ],
        )
        .expect("legacy image note should insert");
    connection
        .execute(
            "INSERT INTO cards (
                 id, note_id, deck_id, direction, template_ordinal,
                 created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'image_occlusion', 0, 10, 10)",
            params![image_card_id, image_note_id, deck_id],
        )
        .expect("legacy image card should insert");

    connection
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text, duplicate_hash,
                 reverse_enabled, cloze_source, cloze_mode,
                 created_at, updated_at, revision
             ) VALUES (
                 ?1, ?2, 'cloze', 'tiptap-json', 1,
                 ?3, ?4, 'The heart pumps blood.', 'heart · blood', '',
                 0, 'The {{c1::heart}} pumps {{c2::blood}}.', 'one_at_a_time',
                 10, 10, 1
             )",
            params![
                cloze_note_id,
                deck_id,
                r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"The heart pumps blood."}]}]}"#,
                r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"heart · blood"}]}]}"#,
            ],
        )
        .expect("legacy cloze note should insert");
    for (card_id, ordinal) in [(&cloze_card_one, 1_i64), (&cloze_card_two, 2_i64)] {
        connection
            .execute(
                "INSERT INTO cards (
                     id, note_id, deck_id, direction, template_ordinal,
                     created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 'cloze', ?4, 10, 10)",
                params![card_id, cloze_note_id, deck_id, ordinal],
            )
            .expect("legacy cloze card should insert");
    }

    connection
        .execute(
            "INSERT INTO card_templates (
                 id, name, normalized_name, description,
                 created_at, updated_at, revision
             ) VALUES (?1, 'Legacy template', 'legacy template', '', 10, 10, 1)",
            [template_id.as_str()],
        )
        .expect("template should insert");
    connection
        .execute(
            "INSERT INTO card_template_fields (
                 id, template_id, name, normalized_name, description, position
             ) VALUES (?1, ?2, 'Field', 'field', '', 0)",
            params![field_id, template_id],
        )
        .expect("field should insert");
    connection
        .execute(
            "INSERT INTO card_template_rules (
                 id, template_id, name, front_pattern, back_pattern,
                 front_doc, back_doc, position
             ) VALUES (?1, ?2, 'Card', '{{Field}}', 'Answer: {{Field}}', NULL, NULL, 0)",
            params![rule_id, template_id],
        )
        .expect("legacy template rule should insert");

    apply_required_repairs(&mut connection).expect("repairs should succeed");
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("foreign keys should disable for the physical rebuild");
    connection
        .execute_batch(CANONICAL_MIGRATION)
        .expect("canonical migration should apply");
    let foreign_key_violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("foreign-key validation should run");
    assert_eq!(foreign_key_violations, 0);
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("foreign keys should be restored after the physical rebuild");
    verify_repair_ledger(&connection).expect("repair ledger should verify");

    let image_state: (String, String, i64, Option<i64>) = connection
        .query_row(
            "SELECT n.note_type, c.direction, c.template_ordinal, c.deleted_at
             FROM notes n JOIN cards c ON c.note_id = n.id
             WHERE n.id = ?1",
            [image_note_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("image note should remain");
    assert_eq!(
        image_state,
        ("basic".to_string(), "forward".to_string(), 0, None)
    );

    let cloze_cards: Vec<(String, i64, Option<i64>)> = {
        let mut statement = connection
            .prepare(
                "SELECT direction, template_ordinal, deleted_at
                 FROM cards WHERE note_id = ?1
                 ORDER BY CASE WHEN deleted_at IS NULL THEN 0 ELSE 1 END, id",
            )
            .expect("cloze query");
        statement
            .query_map([cloze_note_id.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .expect("cloze rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("cloze rows should collect")
    };
    assert_eq!(cloze_cards.len(), 2);
    assert_eq!(cloze_cards[0], ("forward".to_string(), 0, None));
    assert_eq!(cloze_cards[1].0, "forward");
    assert!(cloze_cards[1].1 >= 1_000_000);
    assert!(cloze_cards[1].2.is_some());

    let repaired: (String, String, String) = connection
        .query_row(
            "SELECT n.duplicate_hash, r.front_doc, r.back_doc
             FROM notes n, card_template_rules r
             WHERE n.id = ?1 AND r.id = ?2",
            params![image_note_id, rule_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("repaired values should exist");
    assert!(!repaired.0.is_empty());
    for document in [&repaired.1, &repaired.2] {
        assert!(document.contains(&field_id));
        assert!(!document.contains("{{"));
    }

    for column in ["front_pattern", "back_pattern"] {
        let exists: i64 = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM pragma_table_info('card_template_rules') WHERE name = ?1
                 )",
                [column],
                |row| row.get(0),
            )
            .expect("column query should work");
        assert_eq!(exists, 0, "legacy column {column} should be removed");
    }

    let invalid_note = connection.execute(
        "INSERT INTO notes (
             id, deck_id, note_type, content_format, content_schema_version,
             front_doc, back_doc, front_text, back_text, duplicate_hash,
             reverse_enabled, cloze_source, cloze_mode,
             created_at, updated_at, revision
         ) VALUES (
             ?1, ?2, 'image_occlusion', 'tiptap-json', 1,
             '{}', '{}', '', '', 'x', 0, '', 'one_at_a_time', 20, 20, 1
         )",
        params!["a".repeat(32), deck_id],
    );
    assert!(invalid_note.is_err());
    let invalid_rule = connection.execute(
        "INSERT INTO card_template_rules (
             id, template_id, name, front_doc, back_doc, position
         ) VALUES (?1, ?2, 'Invalid', NULL, NULL, 1)",
        params!["b".repeat(32), template_id],
    );
    assert!(invalid_rule.is_err());

    let before = connection.total_changes();
    apply_required_repairs(&mut connection).expect("second repair pass should succeed");
    verify_repair_ledger(&connection).expect("ledger should still verify");
    assert_eq!(before, connection.total_changes());
    let repair_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM data_repairs", [], |row| row.get(0))
        .expect("repair count");
    assert_eq!(repair_count, 2);
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity check");
    assert_eq!(integrity, "ok");
}

#[test]
fn canonical_database_reopen_does_not_mutate_application_rows() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-no-op-startup-{}-{}",
        std::process::id(),
        crate::util::unix_time_millis().expect("clock")
    ));
    std::fs::create_dir_all(&root).expect("temp directory");
    let database_path = root.join("library.sqlite3");

    {
        let database =
            crate::database::Database::open(&database_path).expect("database should open");
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO decks (
                             id, name, description, sort_order,
                             created_at, updated_at, revision
                         ) VALUES (?1, 'Canonical', '', 0, 111, 222, 7)",
                        ["c".repeat(32)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("fixture should write");
    }

    let before = std::fs::metadata(&database_path)
        .expect("database metadata")
        .len();
    let database = crate::database::Database::open(&database_path).expect("database should reopen");
    database
        .with_connection(|connection| {
            assert_eq!(connection.total_changes(), 0);
            let deck: (i64, i64) = connection
                .query_row(
                    "SELECT updated_at, revision FROM decks WHERE id = ?1",
                    ["c".repeat(32)],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(deck, (222, 7));
            let repair_rows: i64 = connection
                .query_row("SELECT COUNT(*) FROM data_repairs", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(repair_rows, 3);
            Ok(())
        })
        .expect("canonical startup should be read-only");
    drop(database);
    let after = std::fs::metadata(&database_path)
        .expect("database metadata")
        .len();
    assert_eq!(before, after);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn startup_returns_an_interrupted_import_rename_to_its_active_draft() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-import-reconcile-{}-{}",
        std::process::id(),
        crate::util::unix_time_millis().expect("clock")
    ));
    let paths = AppPaths::new(root).expect("paths should initialize");
    let database =
        crate::database::Database::open(paths.database_path()).expect("database should open");
    let draft_id = "d".repeat(32);
    let asset_id = "e".repeat(32);
    let relative_path = AppPaths::image_relative_path(&asset_id, "png");
    let staged_relative_path = AppPaths::staged_asset_relative_path(&draft_id, &asset_id, "png");
    let final_path = paths.resolve_relative(&relative_path);
    std::fs::write(
        &final_path,
        [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0],
    )
    .expect("interrupted final file should exist");

    database
        .with_connection(|connection| {
            let now = crate::util::unix_time_millis()?;
            connection
                .execute(
                    "INSERT INTO asset_drafts (
                         id, purpose, owner_id, created_at, updated_at, expires_at
                     ) VALUES (?1, 'import', ?2, ?3, ?3, ?4)",
                    params![draft_id, "f".repeat(32), now, now + 60_000],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size,
                         relative_path, created_at, lifecycle_state,
                         draft_id, staged_relative_path, updated_at
                     ) VALUES (
                         ?1, 'diagram.png', 'image/png', 12,
                         ?2, ?3, 'staged', ?4, ?5, ?3
                     )",
                    params![asset_id, relative_path, now, draft_id, staged_relative_path,],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .expect("crash fixture should write");

    asset_lifecycle::reconcile_startup(&database, &paths)
        .expect("startup reconciliation should recover the draft");

    assert!(!final_path.exists());
    assert!(paths.resolve_relative(&staged_relative_path).is_file());
    database
        .with_connection(|connection| {
            let state: (String, Option<String>, Option<String>) = connection
                .query_row(
                    "SELECT lifecycle_state, draft_id, staged_relative_path
                     FROM assets WHERE id = ?1",
                    [asset_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(state.0, "staged");
            assert_eq!(state.1.as_deref(), Some(draft_id.as_str()));
            assert_eq!(state.2.as_deref(), Some(staged_relative_path.as_str()));
            let recoveries: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM asset_reconciliation_events
                     WHERE asset_id = ?1 AND action = 'restored'",
                    [asset_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(recoveries, 1);
            Ok(())
        })
        .expect("recovered state should persist");
    let _ = std::fs::remove_dir_all(paths.app_data_dir());
}
