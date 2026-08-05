use super::*;
use serde_json::json;

fn temp_paths(label: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-studydeck-extra-{label}-{}-{}",
        std::process::id(),
        unix_time_millis().expect("clock")
    ));
    AppPaths::new(root).expect("paths")
}

fn paragraph(text: &str) -> Value {
    json!({
        "type": "doc",
        "content": [{"type":"paragraph","content":[{"type":"text","text":text}]}]
    })
}

fn basic_package(front_text: &str) -> PackageData {
    let deck_id = "1".repeat(32);
    let note_id = "2".repeat(32);
    let card_id = "3".repeat(32);
    PackageData {
        version: CURRENT_PACKAGE_VERSION,
        deck: PackageDeck {
            source_id: deck_id.clone(),
            name: "Imported deck".to_string(),
            description: "".to_string(),
            sort_order: 0,
            created_at: 1,
            updated_at: 1,
            revision: 1,
        },
        notes: vec![PackageNote {
            source_id: note_id.clone(),
            source_deck_id: deck_id,
            note_type: "basic".to_string(),
            content_format: "tiptap-json".to_string(),
            content_schema_version: 1,
            front_doc: paragraph(front_text),
            back_doc: paragraph("Answer"),
            front_text: front_text.to_string(),
            back_text: "Answer".to_string(),
            duplicate_hash: duplicate_fingerprint(front_text, "Answer"),
            reverse_enabled: false,
            cloze_source: String::new(),
            cloze_mode: "one_at_a_time".to_string(),
            created_at: 1,
            updated_at: 1,
            revision: 1,
        }],
        cards: vec![PackageCard {
            source_id: card_id,
            source_note_id: note_id,
            direction: "forward".to_string(),
            template_ordinal: 0,
            created_at: 1,
            updated_at: 1,
        }],
        tags: Vec::new(),
        note_tags: Vec::new(),
        assets: Vec::new(),
        note_assets: Vec::new(),
    }
}

fn image_package(front_text: &str) -> PackageData {
    let mut package = basic_package(front_text);
    let asset_id = "4".repeat(32);
    package.notes[0].front_doc = json!({
        "type": "doc",
        "content": [
            {"type":"paragraph","content":[{"type":"text","text":front_text}]},
            {"type":"image","attrs":{
                "assetId": asset_id,
                "src": format!("study-asset://{}", "4".repeat(32)),
                "alt": "diagram.png"
            }}
        ]
    });
    package.assets.push(PackageAsset {
        source_id: "4".repeat(32),
        original_filename: "diagram.png".to_string(),
        mime_type: "image/png".to_string(),
        byte_size: 12,
        data: vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0],
    });
    package.note_assets.push((
        package.notes[0].source_id.clone(),
        "4".repeat(32),
        "front".to_string(),
    ));
    package
}

#[test]
fn reusing_an_import_operation_for_another_package_is_rejected() {
    let paths = temp_paths("conflict");
    let database = Database::open(paths.database_path()).expect("database");
    let operation_id = "a".repeat(32);
    database
        .with_connection(|connection| {
            let first = basic_package("First package");
            read::validate_package_data(&first, true)?;
            import::import_package(connection, &paths, &operation_id, &"1".repeat(64), first)?;
            let second = basic_package("Different package");
            read::validate_package_data(&second, true)?;
            let conflict =
                import::import_package(connection, &paths, &operation_id, &"2".repeat(64), second)
                    .expect_err("operation reuse must conflict");
            assert_eq!(conflict.code, "idempotency_conflict");
            let active_decks: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(active_decks, 1);
            Ok(())
        })
        .expect("conflict should not duplicate the deck");
    let _ = fs::remove_dir_all(paths.app_data_dir());
}

#[test]
fn database_failure_after_asset_staging_leaves_no_partial_deck_or_file() {
    let paths = temp_paths("rollback");
    let database = Database::open(paths.database_path()).expect("database");
    database
        .with_connection(|connection| {
            connection
                .execute_batch(
                    "CREATE TRIGGER injected_note_failure
                     BEFORE INSERT ON notes
                     WHEN NEW.front_text = 'Injected failure'
                     BEGIN
                         SELECT RAISE(ABORT, 'injected note failure');
                     END;",
                )
                .map_err(CommandError::database)?;
            let package = image_package("Injected failure");
            read::validate_package_data(&package, true)?;
            let error = import::import_package(
                connection,
                &paths,
                &"b".repeat(32),
                &"3".repeat(64),
                package,
            )
            .expect_err("injected database failure should abort import");
            assert_eq!(error.code, "database_error");

            for table in ["decks", "notes", "cards", "assets"] {
                let sql = format!("SELECT COUNT(*) FROM {table}");
                let count: i64 = connection
                    .query_row(&sql, [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(count, 0, "partial rows remained in {table}");
            }
            let operation: (String, Option<String>) = connection
                .query_row(
                    "SELECT status, error_code
                     FROM deck_import_operations WHERE id = ?1",
                    ["b".repeat(32)],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(operation.0, "failed");
            assert_eq!(operation.1.as_deref(), Some("database_error"));
            let active_drafts: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM asset_drafts
                     WHERE committed_at IS NULL AND canceled_at IS NULL",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(active_drafts, 0);
            Ok(())
        })
        .expect("failed import should clean its state");
    assert_eq!(fs::read_dir(paths.asset_staging_dir()).unwrap().count(), 0);
    assert_eq!(fs::read_dir(paths.images_dir()).unwrap().count(), 0);
    let _ = fs::remove_dir_all(paths.app_data_dir());
}
