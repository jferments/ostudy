//! Per-review scheduler context is review evidence. It travels with the
//! flashcard graph exactly like review attributions and is removed with that
//! graph when flashcards are not selected.

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use rusqlite::{params, Connection};
use serde_json::json;

use super::*;
use crate::{
    cards::validate_content,
    database::Database,
    decks::{create_deck_in_connection, CreateDeckInput},
    device_sync::{
        merge::merge_remote_database,
        package::{create_package, extract_package},
        store::SyncStore,
    },
    paths::AppPaths,
    practice::{record_practice_event, submit_review_in_connection, SubmitReviewInput},
    scheduler::insert_new_schedule,
    util::{generate_id, unix_time_millis},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

type ContextRow = (
    String,
    Option<i64>,
    i64,
    Option<f64>,
    i64,
    i64,
    f64,
    String,
    Option<i64>,
);

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Sender {
    database: Database,
    paths: AppPaths,
    root: PathBuf,
    _cleanup: Cleanup,
}

fn id(value: u32) -> String {
    format!("{value:032x}")
}

fn paragraph(text: &str) -> serde_json::Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn context_rows(connection: &Connection) -> Vec<ContextRow> {
    let mut statement = connection
        .prepare(
            "SELECT review_event_id, previous_review_at, elapsed_days, retrievability,
                    reps_before, lapses_before, desired_retention,
                    scheduler_parameters_sha256, active_time_ms
             FROM review_event_scheduler_context ORDER BY review_event_id",
        )
        .unwrap();
    statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
        .unwrap()
}

impl Sender {
    /// A library with one card reviewed through the real practice write path.
    fn reviewed() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-review-context-sync-{}-{}-{}",
            std::process::id(),
            unix_time_millis().unwrap(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let paths = AppPaths::new(root.join("sender")).unwrap();
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let deck_id = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Synchronized reviews".to_string(),
                        description: None,
                    },
                )?
                .id;
                let content = validate_content(
                    paragraph("Question"),
                    paragraph("Answer"),
                    "Question".to_string(),
                    "Answer".to_string(),
                )?;
                let note_id = generate_id(connection)?;
                let card_id = generate_id(connection)?;
                let session_id = generate_id(connection)?;
                connection
                    .execute(
                        "INSERT INTO notes (id, deck_id, note_type, content_format,
                             content_schema_version, front_doc, back_doc, front_text, back_text,
                             duplicate_hash, created_at, updated_at, revision)
                         VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6,
                                 'review-context-sync', 10, 10, 1)",
                        params![
                            note_id,
                            deck_id,
                            content.front_doc_json,
                            content.back_doc_json,
                            content.front_text,
                            content.back_text,
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal,
                             created_at, updated_at)
                         VALUES (?1, ?2, ?3, 'forward', 0, 10, 10)",
                        params![card_id, note_id, deck_id],
                    )
                    .map_err(CommandError::database)?;
                insert_new_schedule(connection, &card_id, 10)?;
                connection
                    .execute(
                        "INSERT INTO practice_sessions (id, selected_deck_ids_json, shuffle,
                             card_count, mode, started_at)
                         VALUES (?1, '[]', 0, 1, 'custom', 10)",
                        [&session_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO practice_session_cards (session_id, card_id, position,
                             presentation_variant)
                         VALUES (?1, ?2, 0, '')",
                        params![session_id, card_id],
                    )
                    .map_err(CommandError::database)?;
                record_practice_event(connection, &session_id, &card_id, true)?;
                submit_review_in_connection(
                    connection,
                    SubmitReviewInput {
                        session_id,
                        card_id,
                        submission_id: id(77),
                        outcome: "somewhat_confident".to_string(),
                        response_time_ms: 9_000,
                        schedule_revision: 1,
                        active_time_ms: Some(6_000),
                    },
                )?;
                Ok(())
            })
            .unwrap();
        Self {
            database,
            paths,
            root: root.clone(),
            _cleanup: Cleanup(root),
        }
    }

    fn context_rows(&self) -> Vec<ContextRow> {
        self.database
            .with_connection(|connection| Ok(context_rows(connection)))
            .unwrap()
    }

    /// Package the sender for `selection` and open the extracted snapshot.
    fn send(&self, label: &str, selection: SyncSelection) -> (Connection, SyncSelection) {
        let destination = self.root.join(format!("{label}.syncpkg"));
        create_package(&self.database, &self.paths, &destination, selection).unwrap();
        let extracted = extract_package(&destination, &self.root.join(label)).unwrap();
        (
            Connection::open(&extracted.database_path).unwrap(),
            extracted.selection,
        )
    }
}

#[test]
fn scheduler_context_travels_with_its_review_and_merges_once() {
    let sender = Sender::reviewed();
    let sent = sender.context_rows();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].8, Some(6_000));

    let (remote, selection) = sender.send("flashcards", SyncSelection::default());
    assert_eq!(context_rows(&remote), sent);

    let receiver_paths = AppPaths::new(sender.root.join("receiver")).unwrap();
    let receiver = Database::open_in_memory().unwrap();
    let store = SyncStore::open(&receiver_paths).unwrap();
    store
        .upsert_peer(&id(101), "sender", "public-key", &"b".repeat(64), "")
        .unwrap();
    receiver
        .with_connection(|local| {
            let first = merge_remote_database(local, &remote, &store, &id(100), &id(101), selection)?;
            assert!(first.changed_row_count > 0);
            assert_eq!(first.conflict_count, 0);
            assert_eq!(context_rows(local), sent);
            assert_eq!(count(local, "review_events"), 1);

            // The same package again is already merged: nothing changes.
            let again = merge_remote_database(local, &remote, &store, &id(100), &id(101), selection)?;
            assert_eq!(again.changed_row_count, 0);
            assert_eq!(again.conflict_count, 0);
            assert_eq!(context_rows(local), sent);
            Ok(())
        })
        .unwrap();
}

#[test]
fn scheduler_context_is_scoped_and_sanitized_like_review_attributions() {
    let selections = [
        SyncSelection::default(),
        SyncSelection {
            flashcards: false,
            ..SyncSelection::default()
        },
        SyncSelection {
            documents: false,
            study_guides: false,
            ..SyncSelection::default()
        },
        SyncSelection {
            flashcards: false,
            documents: false,
            study_guides: false,
            notes: false,
            lessons: false,
            api_settings: true,
            api_credentials: false,
        },
    ];
    for selection in selections {
        assert_eq!(
            selection.includes_table("review_event_scheduler_context"),
            selection.includes_table("review_event_attributions"),
            "{selection:?}"
        );
    }
    assert!(crate::device_sync::merge::table_spec("review_event_scheduler_context").is_some());

    let sender = Sender::reviewed();
    let (remote, _) = sender.send(
        "documents-only",
        SyncSelection {
            flashcards: false,
            ..SyncSelection::default()
        },
    );
    for table in [
        "review_events",
        "review_event_attributions",
        "review_event_scheduler_context",
    ] {
        assert_eq!(count(&remote, table), 0, "unselected review rows in {table}");
    }
    let violations: i64 = remote
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
    assert_eq!(sender.context_rows().len(), 1, "the sender keeps its own rows");
}

/// Kept totals of purged reviews are learner history and travel with
/// flashcards; the rollup cache is local and each device rebuilds its own.
#[test]
fn kept_study_activity_travels_with_flashcards_and_rollups_stay_local() {
    let sender = Sender::reviewed();
    sender
        .database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO study_activity_retained(operation_id, wall_date, wall_hour,
                         review_count, remembered_count, new_card_count, study_time_ms)
                     VALUES (?1, '2026-09-01', 9, 3, 2, 1, 12000)",
                    [id(55)],
                )
                .map_err(CommandError::database)?;
            crate::study_activity::refresh_rollups(connection)
        })
        .unwrap();
    assert!(selection_includes_ledger(SyncSelection::default()));
    assert!(!selection_includes_ledger(SyncSelection {
        flashcards: false,
        ..SyncSelection::default()
    }));
    assert!(crate::device_sync::merge::table_spec("study_activity_retained").is_some());
    assert!(crate::device_sync::merge::table_spec("review_day_rollups").is_none());

    let (remote, selection) = sender.send("ledger", SyncSelection::default());
    assert_eq!(count(&remote, "study_activity_retained"), 1);
    assert_eq!(count(&remote, "review_day_rollups"), 0);
    assert_eq!(count(&remote, "review_day_rollup_built_decks"), 0);

    let receiver_paths = AppPaths::new(sender.root.join("ledger-receiver")).unwrap();
    let receiver = Database::open_in_memory().unwrap();
    let store = SyncStore::open(&receiver_paths).unwrap();
    store
        .upsert_peer(&id(101), "sender", "public-key", &"b".repeat(64), "")
        .unwrap();
    receiver
        .with_connection(|local| {
            merge_remote_database(local, &remote, &store, &id(100), &id(101), selection)?;
            merge_remote_database(local, &remote, &store, &id(100), &id(101), selection)?;
            assert_eq!(count(local, "study_activity_retained"), 1);
            crate::study_activity::refresh_rollups(local)?;
            let reviews: i64 = local
                .query_row("SELECT SUM(review_count) FROM review_day_rollups", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)?;
            assert_eq!(reviews, 1);
            Ok(())
        })
        .unwrap();

    let (documents, _) = sender.send(
        "ledger-documents-only",
        SyncSelection {
            flashcards: false,
            ..SyncSelection::default()
        },
    );
    assert_eq!(count(&documents, "study_activity_retained"), 0);
    assert_eq!(count(&documents, "review_day_rollups"), 0);
}

fn selection_includes_ledger(selection: SyncSelection) -> bool {
    selection.includes_table("study_activity_retained")
}
