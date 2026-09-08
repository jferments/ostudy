//! Generated fixtures: no downloaded deck content or Anki implementation code.

use std::{fs, io::Write, path::PathBuf};

use base64::Engine;
use rusqlite::{params, Connection};
use serde_json::json;
use zip::{write::SimpleFileOptions, ZipWriter};

use super::*;
use crate::{database::Database, paths::AppPaths};

struct Fixture {
    root: PathBuf,
    path: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn fixture(extra: &[(&str, &[u8])], media: &str) -> Fixture {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "ostudy-anki-test-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    let database_path = root.join("source.sqlite3");
    let connection = Connection::open(&database_path).unwrap();
    connection.execute_batch(
        "CREATE TABLE col (ver INTEGER, models TEXT, decks TEXT);
         CREATE TABLE notes (id INTEGER PRIMARY KEY, mid INTEGER, flds TEXT, tags TEXT);
         CREATE TABLE cards (id INTEGER PRIMARY KEY, nid INTEGER, did INTEGER, ord INTEGER, odid INTEGER);
         CREATE TABLE revlog (id INTEGER PRIMARY KEY, cid INTEGER, ease INTEGER);
         INSERT INTO revlog VALUES (1, 100, 4);",
    ).unwrap();
    let models = json!({
        "1": {"name": "Basic and optional reverse", "type": 0, "css": ".card { color: black; }",
            "flds": [{"name": "Front", "ord": 0}, {"name": "Back", "ord": 1}, {"name": "Reverse", "ord": 2}],
            "tmpls": [
                {"name": "Card 1", "ord": 0, "qfmt": "{{Front}}", "afmt": "{{FrontSide}}<hr id=answer>{{Back}}"},
                {"name": "Card 2", "ord": 1, "qfmt": "{{#Reverse}}{{Back}}{{/Reverse}}", "afmt": "{{Front}}"}
            ]},
        "2": {"name": "Cloze", "type": 1, "flds": [{"name": "Text", "ord": 0}],
            "tmpls": [{"name": "Cloze", "ord": 0, "qfmt": "{{cloze:Text}}", "afmt": "{{cloze:Text}}"}]},
        "3": {"name": "Scripted with static fallback", "type": 0, "flds": [{"name": "Front", "ord": 0}],
            "tmpls": [{"name": "Card", "ord": 0, "qfmt": "<script>unsafe()</script>{{Front}}", "afmt": "Answer"}]},
        "4": {"name": "Type in the answer", "type": 0,
            "flds": [{"name": "Front", "ord": 0}, {"name": "Back", "ord": 1}],
            "tmpls": [{"name": "Card", "ord": 0, "qfmt": "{{Front}} {{type:Back}}", "afmt": "{{FrontSide}}<hr id=answer>{{type:Back}}"}]},
        "5": {"name": "Script generated only", "type": 0, "flds": [{"name": "Front", "ord": 0}],
            "tmpls": [{"name": "Card", "ord": 0, "qfmt": "<script>document.write('Question')</script>", "afmt": "Answer"}]}
    });
    connection
        .execute(
            "INSERT INTO col VALUES (11, ?1, ?2)",
            params![
                models.to_string(),
                json!({"1": {"name": "Examples::Science"}}).to_string()
            ],
        )
        .unwrap();
    for (id, model, fields) in [
        (10, 1, "<b>Symbol for sodium?</b>\u{1f}Na\u{1f}yes"),
        (
            11,
            1,
            "Image example<img src='pixel.png'>\u{1f}A pixel\u{1f}",
        ),
        (12, 2, "{{c1::first::position}} then {{c2::second}}"),
        (13, 3, "Static script card"),
        (14, 4, "What is 2 + 2?\u{1f}4"),
        (15, 5, "Ignored because script generates the question"),
    ] {
        connection
            .execute(
                "INSERT INTO notes VALUES (?1, ?2, ?3, ' science shared ')",
                params![id, model, fields],
            )
            .unwrap();
    }
    for (id, note, ordinal) in [
        (100, 10, 0),
        (101, 10, 1),
        (102, 11, 0),
        (103, 12, 0),
        (104, 12, 1),
        (105, 13, 0),
        (106, 14, 0),
        (107, 15, 0),
    ] {
        connection
            .execute(
                "INSERT INTO cards VALUES (?1, ?2, 1, ?3, 0)",
                params![id, note, ordinal],
            )
            .unwrap();
    }
    drop(connection);
    let path = root.join("examples.apkg");
    let mut zip = ZipWriter::new(fs::File::create(&path).unwrap());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("collection.anki2", options).unwrap();
    zip.write_all(&fs::read(&database_path).unwrap()).unwrap();
    zip.start_file("media", options).unwrap();
    zip.write_all(media.as_bytes()).unwrap();
    zip.start_file("0", options).unwrap();
    let pixel = base64::engine::general_purpose::STANDARD
        .decode("R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7")
        .unwrap();
    zip.write_all(&pixel).unwrap();
    for (name, bytes) in extra {
        zip.start_file(*name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
    Fixture { root, path }
}

#[test]
fn converts_existing_basic_reverse_native_cloze_typed_and_static_script_presentations() {
    let fixture = fixture(&[], r#"{"0":"pixel.png"}"#);
    let (package, report) = read_package(&fixture.path, "examples.apkg").unwrap();
    assert_eq!(report.source_card_count, 8);
    assert_eq!(report.imported_card_count, 7);
    assert_eq!(report.skipped_card_count, 1);
    assert_eq!(report.simplified_card_count, 1);
    assert_eq!(report.native_style_card_count, 3);
    assert_eq!(report.typed_answer_card_count, 1);
    assert_eq!(report.native_cloze_card_count, 2);
    assert_eq!(report.cloze_fallback_card_count, 0);
    assert_eq!(report.javascript_salvaged_card_count, 1);
    assert_eq!(package.notes.len(), 7);
    assert_eq!(package.cards.len(), 7);
    assert_eq!(package.assets.len(), 1);

    // Standard Anki Basic backs commonly contain FrontSide. The stored oStudy
    // answer must remain answer-only; question repetition is a presentation
    // preference, not imported answer content.
    assert!(package
        .notes
        .iter()
        .any(|note| note.front_text == "Symbol for sodium?" && note.back_text == "Na"));
    assert!(package
        .notes
        .iter()
        .any(|note| note.front_text == "Na" && note.back_text == "Symbol for sodium?"));
    assert!(package
        .notes
        .iter()
        .any(|note| note.front_text.trim() == "What is 2 + 2?" && note.back_text == "4"));
    assert!(package
        .notes
        .iter()
        .any(|note| note.front_text == "Static script card" && note.back_text == "Answer"));

    let cloze = package
        .notes
        .iter()
        .filter(|note| note.note_type == "cloze")
        .collect::<Vec<_>>();
    assert_eq!(cloze.len(), 2);
    assert!(cloze
        .iter()
        .any(|note| note.cloze_source == "{{c1::first}} then second"));
    assert!(cloze
        .iter()
        .any(|note| note.cloze_source == "first then {{c1::second}}"));
    assert!(cloze
        .iter()
        .any(|note| note.front_text == "[…] then second"));
    assert!(cloze
        .iter()
        .all(|note| note.back_text.contains("Complete statement:")));

    assert!(package
        .tags
        .iter()
        .any(|tag| tag.name == "Anki deck::Examples::Science"));
    assert!(package.notes.iter().all(|note| !note.reverse_enabled));
    assert!(report.issues.iter().any(|issue| {
        issue.source_card_id == "107"
            && issue.message.contains("JavaScript")
            && issue.message.contains("static")
    }));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.source_card_id == "105"));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.message.contains("CSS")));
    assert!(!fs::read_dir(&fixture.root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains(".anki-")));
}

#[test]
fn progress_reports_card_totals_successes_failures_and_phases() {
    let fixture = fixture(&[], r#"{"0":"pixel.png"}"#);
    let mut updates = Vec::new();
    let (_, report) = read_package_with_progress(&fixture.path, "examples.apkg", &mut |progress| {
        updates.push(progress)
    })
    .unwrap();
    assert_eq!(report.imported_card_count, 7);
    assert_eq!(
        updates.first().unwrap().phase,
        ProgressPhase::ReadingPackage
    );
    assert!(updates.iter().any(|update| {
        update.phase == ProgressPhase::ConvertingCards
            && update.processed == Some(0)
            && update.total == Some(8)
    }));
    assert!(updates.iter().any(|update| {
        update.phase == ProgressPhase::ConvertingCards
            && update.processed == Some(8)
            && update.succeeded == 7
            && update.failed == 1
    }));
    assert_eq!(updates.last().unwrap().phase, ProgressPhase::ImportingMedia);
    assert_eq!(updates.last().unwrap().total, Some(8));
}

#[test]
fn missing_images_skip_affected_cards_without_external_fetches_or_placeholder_answers() {
    let fixture = fixture(&[], "{}");
    let (package, report) = read_package(&fixture.path, "examples.apkg").unwrap();
    assert_eq!(report.imported_card_count, 6);
    assert_eq!(report.skipped_card_count, 2);
    assert!(package.assets.is_empty());
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.source_card_id == "102" && issue.message.contains("image")));
}

#[test]
fn rejects_modern_collection_before_legacy_placeholder_and_unsafe_paths() {
    let modern = fixture(
        &[("collection.anki21b", b"compressed collection")],
        r#"{"0":"pixel.png"}"#,
    );
    let error = read_package(&modern.path, "modern.apkg").err().unwrap();
    assert!(error.message.contains("Support older Anki versions"));
    let traversal = fixture(&[("../escape", b"not extracted")], r#"{"0":"pixel.png"}"#);
    assert!(read_package(&traversal.path, "unsafe.apkg").is_err());
    assert!(!traversal.root.parent().unwrap().join("escape").exists());
}

#[test]
fn rejects_duplicate_media_map_entries_unknown_schema_and_views() {
    let duplicate = fixture(&[], r#"{"0":"pixel.png","0":"different.png"}"#);
    assert!(read_package(&duplicate.path, "duplicate.apkg").is_err());
    let fixture = fixture(&[], r#"{"0":"pixel.png"}"#);
    let path = fixture.root.join("source.sqlite3");
    let connection = Connection::open(&path).unwrap();
    connection.execute("UPDATE col SET ver = 18", []).unwrap();
    assert!(collection::read(&path)
        .err()
        .unwrap()
        .message
        .contains("Support older"));
    connection
        .execute_batch(
            "UPDATE col SET ver = 11; DROP TABLE cards; CREATE VIEW cards AS SELECT 1 AS id;",
        )
        .unwrap();
    assert!(collection::read(&path).is_err());
}

#[test]
fn imported_content_uses_transactional_replay_and_fresh_native_schedules() {
    let fixture = fixture(&[], r#"{"0":"pixel.png"}"#);
    let paths = AppPaths::new(fixture.root.join("target")).unwrap();
    let database = Database::open(paths.database_path()).unwrap();
    let operation = "a".repeat(32);
    let fingerprint = super::super::hash_file(&fixture.path).unwrap();
    let (package, _) = read_package(&fixture.path, "examples.apkg").unwrap();
    let first = database
        .with_connection(|connection| {
            super::super::import::import_package(
                connection,
                &paths,
                &operation,
                &fingerprint,
                package,
            )
        })
        .unwrap();
    let (package, _) = read_package(&fixture.path, "examples.apkg").unwrap();
    let replay = database
        .with_connection(|connection| {
            super::super::import::import_package(
                connection,
                &paths,
                &operation,
                &fingerprint,
                package,
            )
        })
        .unwrap();
    assert_eq!(first.deck.id, replay.deck.id);
    assert_eq!(first.card_count, 7);
    database
        .with_connection(|connection| {
            let counts: (i64, i64, i64, i64) = connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM decks), (SELECT COUNT(*) FROM cards),
             (SELECT COUNT(*) FROM review_events), (SELECT MAX(reps) FROM card_schedules)",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(counts, (1, 7, 0, 0));
            Ok(())
        })
        .unwrap();
    let old_receipt = serde_json::to_value(first).unwrap();
    let compatible: super::super::ImportedDeckDto = serde_json::from_value(old_receipt).unwrap();
    assert!(compatible.anki_report.is_none());
}

#[test]
fn failed_native_transaction_does_not_leave_a_partial_deck() {
    let fixture = fixture(&[], r#"{"0":"pixel.png"}"#);
    let paths = AppPaths::new(fixture.root.join("rollback-target")).unwrap();
    let database = Database::open(paths.database_path()).unwrap();
    database.with_connection(|connection| {
        connection.execute_batch("CREATE TRIGGER refuse_import BEFORE INSERT ON notes BEGIN SELECT RAISE(ABORT, 'test failure'); END;")
            .map_err(CommandError::database)
    }).unwrap();
    let (package, _) = read_package(&fixture.path, "examples.apkg").unwrap();
    let result = database.with_connection(|connection| {
        super::super::import::import_package(
            connection,
            &paths,
            &"b".repeat(32),
            "fixture",
            package,
        )
    });
    assert!(result.is_err());
    database
        .with_connection(|connection| {
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM decks", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn compatibility_issue_details_are_bounded_without_losing_counts() {
    let mut report = ImportReport::default();
    for id in 0..150 {
        report.issue(id, "x".repeat(1000));
    }
    assert_eq!(report.issues.len(), 100);
    assert_eq!(report.omitted_issue_count, 50);
    assert!(report.issues.iter().all(|issue| issue.message.len() <= 300));
}
