use super::*;
use crate::{
    decks::{create_deck_in_connection, CreateDeckInput},
    scheduler::insert_new_schedule,
};
use serde_json::json;

pub(super) fn temp_paths(label: &str) -> AppPaths {
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
                    name: "Shared".to_string(),
                    description: "Portable".to_string(),
                },
            )?;
            let note_id = generate_id(connection)?;
            let card_id = generate_id(connection)?;
            let now = unix_time_millis()?;
            let front = paragraph("Question");
            let back = paragraph("Answer");
            connection
                .execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text, duplicate_hash,
                         created_at, updated_at
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1,
                         ?3, ?4, 'Question', 'Answer', ?5, ?6, ?6)",
                    params![
                        note_id,
                        deck.id,
                        front.to_string(),
                        back.to_string(),
                        duplicate_fingerprint("Question", "Answer"),
                        now
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
                    params![card_id, note_id, deck.id, now],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(connection, &card_id, now)?;
            Ok(deck.id)
        })
        .expect("fixture should save");
    let destination = source_paths.app_data_dir().join("roundtrip.studydeck");
    export::export_deck(&source, &source_paths, &deck_id, &destination).expect("export should work");
    let package = read::read_package(&destination).expect("package should read");
    assert_eq!(package.version, CURRENT_PACKAGE_VERSION);
    assert_eq!(package.notes.len(), 1);
    assert_eq!(package.cards.len(), 1);
    assert_eq!(package.notes[0].front_text, "Question");
    assert_eq!(package.notes[0].back_text, "Answer");

    let target_paths = temp_paths("target");
    let target = Database::open(target_paths.database_path()).expect("target database");
    let result = target
        .with_connection(|connection| {
            import::import_package(
                connection,
                &target_paths,
                &generate_id(connection)?,
                &"a".repeat(64),
                package,
            )
        })
        .expect("import should work");
    assert_eq!(result.card_count, 1);
    target
        .with_connection(|connection| {
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM card_schedules", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(count, 1);
            Ok(())
        })
        .expect("fresh schedule should exist");
    let _ = std::fs::remove_dir_all(source_paths.app_data_dir());
    let _ = std::fs::remove_dir_all(target_paths.app_data_dir());
}
