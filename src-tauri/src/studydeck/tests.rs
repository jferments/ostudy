use super::*;
use crate::{
    decks::{create_deck_in_connection, CreateDeckInput},
    scheduler::insert_new_schedule,
};
use serde_json::json;

fn temp_paths(label: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-studydeck-{label}-{}-{}",
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

#[test]
fn current_package_round_trip_preserves_content_and_uses_fresh_schedules() {
    let source_paths = temp_paths("source");
    let source = Database::open(source_paths.database_path()).expect("source database");
    let deck_id = source
        .with_connection(|connection| {
            let deck = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Pharmacology".to_string(),
                    description: Some("ACE inhibitors".to_string()),
                },
            )?;
            let note_id = generate_id(connection)?;
            let forward_id = generate_id(connection)?;
            let reverse_id = generate_id(connection)?;
            let front = paragraph("What does lisinopril inhibit?");
            let back = paragraph("Angiotensin-converting enzyme");
            let validated = validate_content(front, back, String::new(), String::new())?;
            connection
                .execute(
                    "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, duplicate_hash,
                     reverse_enabled, cloze_source, cloze_mode,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, 'basic', 'tiptap-json', 1,
                     ?3, ?4, ?5, ?6, ?7,
                     1, '', 'one_at_a_time', 10, 10, 1
                 )",
                    params![
                        note_id,
                        deck.id,
                        validated.front_doc_json,
                        validated.back_doc_json,
                        validated.front_text,
                        validated.back_text,
                        duplicate_fingerprint(&validated.front_text, &validated.back_text),
                    ],
                )
                .map_err(CommandError::database)?;
            for (card_id, direction) in [(&forward_id, "forward"), (&reverse_id, "reverse")] {
                connection
                    .execute(
                        "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, 0, 10, 10)",
                        params![card_id, note_id, deck.id, direction],
                    )
                    .map_err(CommandError::database)?;
                insert_new_schedule(connection, card_id, 10)?;
            }
            connection
                .execute(
                    "UPDATE card_schedules
                 SET state = 2, due_at = 99, reps = 4, revision = 3
                 WHERE card_id = ?1",
                    [forward_id.as_str()],
                )
                .map_err(CommandError::database)?;
            let tag_id = generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO tags (
                     id, name, normalized_name, created_at, updated_at
                 ) VALUES (?1, 'Cardiology', 'cardiology', 10, 10)",
                    [tag_id.as_str()],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                    params![note_id, tag_id],
                )
                .map_err(CommandError::database)?;
            Ok(deck.id)
        })
        .expect("fixture");

    let package_path = source_paths.app_data_dir().join("deck.studydeck");
    source
        .with_connection(|connection| {
            export::export_deck_from_connection(
                connection,
                &source_paths,
                &deck_id,
                &package_path,
            )?;
            Ok(())
        })
        .expect("export");
    let package_connection =
        Connection::open_with_flags(&package_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("package");
    assert_eq!(
        metadata_value(&package_connection, "formatVersion").expect("version"),
        "2"
    );

    let target_paths = temp_paths("target");
    let target = Database::open(target_paths.database_path()).expect("target database");
    let package = read::read_package(&package_path).expect("read package");
    let fingerprint = hash_file(&package_path).expect("fingerprint");
    let operation_id = "a".repeat(32);
    let first = target
        .with_connection(|connection| {
            import::import_package(
                connection,
                &target_paths,
                &operation_id,
                &fingerprint,
                package,
            )
        })
        .expect("import");
    assert_eq!(first.card_count, 2);
    let replay_package = read::read_package(&package_path).expect("read package");
    let replay = target
        .with_connection(|connection| {
            import::import_package(
                connection,
                &target_paths,
                &operation_id,
                &fingerprint,
                replay_package,
            )
        })
        .expect("replay");
    assert_eq!(replay.deck.id, first.deck.id);

    target
        .with_connection(|connection| {
            let counts: (i64, i64, i64) = connection
                .query_row(
                    "SELECT
                     (SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL),
                     (SELECT COUNT(*) FROM cards c
                      JOIN notes n ON n.id = c.note_id
                      WHERE c.deck_id = ?1 AND c.deleted_at IS NULL AND n.deleted_at IS NULL),
                     (SELECT COUNT(*) FROM note_tags nt
                      JOIN notes n ON n.id = nt.note_id WHERE n.deck_id = ?1)",
                    [first.deck.id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(counts, (1, 2, 1));
            let fresh: (i64, i64) = connection
                .query_row(
                    "SELECT MIN(s.reps), MAX(s.reps)
                 FROM card_schedules s
                 JOIN cards c ON c.id = s.card_id
                 WHERE c.deck_id = ?1",
                    [first.deck.id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(fresh, (0, 0));
            Ok(())
        })
        .expect("verify import");

    let _ = fs::remove_dir_all(source_paths.app_data_dir());
    let _ = fs::remove_dir_all(target_paths.app_data_dir());
}

#[test]
fn legacy_multi_card_cloze_is_adapted_before_import() {
    let paths = temp_paths("legacy");
    let package_path = paths.app_data_dir().join("legacy.studydeck");
    let package = Connection::open(&package_path).expect("package");
    package
        .execute_batch(
            "CREATE TABLE package_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
         INSERT INTO package_metadata VALUES ('format', 'studydeck-sqlite');
         INSERT INTO package_metadata VALUES ('formatVersion', '1');
         CREATE TABLE deck (
             source_id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL,
             created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, revision INTEGER NOT NULL
         ) STRICT;
         CREATE TABLE notes (
             source_id TEXT PRIMARY KEY, source_deck_id TEXT NOT NULL,
             note_type TEXT NOT NULL, content_format TEXT NOT NULL,
             content_schema_version INTEGER NOT NULL,
             front_doc TEXT NOT NULL, back_doc TEXT NOT NULL,
             front_text TEXT NOT NULL, back_text TEXT NOT NULL,
             created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
             revision INTEGER NOT NULL
         ) STRICT;
         CREATE TABLE cards (
             source_id TEXT PRIMARY KEY, source_note_id TEXT NOT NULL,
             direction TEXT NOT NULL, template_ordinal INTEGER NOT NULL,
             created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
         ) STRICT;
         CREATE TABLE assets (
             source_id TEXT PRIMARY KEY, original_filename TEXT NOT NULL,
             mime_type TEXT NOT NULL, byte_size INTEGER NOT NULL, data BLOB NOT NULL
         ) STRICT;
         CREATE TABLE note_assets (
             source_note_id TEXT NOT NULL, source_asset_id TEXT NOT NULL,
             side TEXT NOT NULL, PRIMARY KEY(source_note_id, source_asset_id, side)
         ) STRICT;
         CREATE TABLE card_type_metadata (
             source_note_id TEXT PRIMARY KEY, reverse_enabled INTEGER NOT NULL,
             cloze_source TEXT NOT NULL, cloze_mode TEXT NOT NULL
         ) STRICT;",
        )
        .expect("schema");
    let deck_id = "1".repeat(32);
    let note_id = "2".repeat(32);
    package
        .execute(
            "INSERT INTO deck VALUES (?1, 'Legacy cloze', '', 1, 1, 1)",
            [deck_id.as_str()],
        )
        .expect("deck");
    package
        .execute(
            "INSERT INTO notes VALUES (
             ?1, ?2, 'cloze', 'tiptap-json', 1,
             ?3, ?4, 'The heart pumps blood.', 'heart · blood', 1, 1, 1
         )",
            params![
                note_id,
                deck_id,
                serde_json::to_string(&paragraph("The heart pumps blood.")).unwrap(),
                serde_json::to_string(&paragraph("heart · blood")).unwrap(),
            ],
        )
        .expect("note");
    package
        .execute(
            "INSERT INTO card_type_metadata VALUES (?1, 0, ?2, 'one_at_a_time')",
            params![note_id, "The {{c1::heart}} pumps {{c2::blood}}."],
        )
        .expect("metadata");
    for (id, ordinal) in [("3".repeat(32), 1), ("4".repeat(32), 2)] {
        package
            .execute(
                "INSERT INTO cards VALUES (?1, ?2, 'cloze', ?3, 1, 1)",
                params![id, note_id, ordinal],
            )
            .expect("card");
    }
    drop(package);

    let data = read::read_package(&package_path).expect("legacy adapter");
    assert_eq!(data.cards.len(), 1);
    assert_eq!(data.cards[0].direction, "forward");
    assert_eq!(data.cards[0].template_ordinal, 0);
    assert_eq!(data.notes[0].note_type, "cloze");
    let _ = fs::remove_dir_all(paths.app_data_dir());
}
