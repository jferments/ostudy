//! Subjects and merged collection copies across device sync (release schema 6).
//! An updated device takes a package from a device without the update, the
//! other way round is refused with the update message, subjects travel with
//! either category, card merges travel with flashcards, and the merge lists
//! the filing and copy choices it settled on its own.

use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection};

use super::*;
use crate::{
    analytics::{load_analytics, AnalyticsQueryDto},
    database::subjects_fixture::{
        advance_release_library, release_library, seed, SubjectsLibrary, DAY, T0,
    },
    device_sync::{
        merge::{merge_remote_database, remember_sent_database},
        scope::sanitize_snapshot,
        store::SyncStore,
    },
    subjects::load_filing_reviews,
};

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-subjects-sync-{label}-{}-{}",
            std::process::id(),
            crate::platform::now_utc_millis().unwrap_or_default()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const THIS_DEVICE: &str = "000000000000000000000000000000a1";
const OTHER_DEVICE: &str = "000000000000000000000000000000b2";

fn every_category() -> SyncSelection {
    SyncSelection::default()
}

fn flashcards_only() -> SyncSelection {
    SyncSelection {
        flashcards: true,
        documents: false,
        study_guides: false,
        api_settings: false,
        api_credentials: false,
    }
}

fn copy_to(connection: &Connection, path: &Path) {
    connection
        .execute_batch(&format!("VACUUM INTO '{}'", path.display()))
        .unwrap();
}

fn store(scratch: &Scratch) -> SyncStore {
    let paths = AppPaths::new(scratch.path("receiver")).unwrap();
    let store = SyncStore::open(&paths).unwrap();
    store
        .upsert_peer(OTHER_DEVICE, "Other device", "public-key", &"b".repeat(64), "")
        .unwrap();
    store
}

fn all_time(deck_ids: Vec<String>) -> AnalyticsQueryDto {
    AnalyticsQueryDto {
        range: "all".to_string(),
        deck_ids,
        tag_ids: Vec::new(),
        custom_start_local_date: None,
        custom_end_local_date: None,
        highlighted_session_id: None,
    }
}

fn rows(connection: &Connection, sql: &str) -> Vec<Vec<String>> {
    let mut statement = connection.prepare(sql).unwrap();
    let columns = statement.column_count();
    statement
        .query_map([], |row| {
            (0..columns)
                .map(|index| {
                    Ok(match row.get_ref(index)? {
                        rusqlite::types::ValueRef::Null => "NULL".to_string(),
                        rusqlite::types::ValueRef::Integer(value) => value.to_string(),
                        rusqlite::types::ValueRef::Text(value) => {
                            String::from_utf8_lossy(value).into_owned()
                        }
                        other => format!("{other:?}"),
                    })
                })
                .collect::<Result<Vec<_>, rusqlite::Error>>()
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

const MERGES: &str =
    "SELECT retired_card_id, surviving_card_id FROM card_merges ORDER BY retired_card_id";
const SUBJECTS: &str = "SELECT id, name, sort_order FROM document_collections ORDER BY id";
const DECK_FILING: &str = "SELECT id, subject_id FROM decks ORDER BY id";

/// The fixture library at schema 5 without its documents, as if the learner
/// deleted them after making the study bundles. A package with the Documents
/// category then needs no document files.
fn schema5_without_documents() -> (Connection, SubjectsLibrary) {
    let connection = release_library(5);
    let library = seed(&connection);
    connection
        .execute_batch(
            "DELETE FROM document_collection_ranges;
             DELETE FROM document_collection_memberships;
             DELETE FROM document_decks;
             DELETE FROM documents;",
        )
        .unwrap();
    (connection, library)
}

/// A review recorded on a device without the update, with the schedule it
/// produced, the way the fixture records reviews.
fn review_without_the_update(connection: &Connection, card: &str, at: i64) {
    let id = format!("ee{}", "f".repeat(30));
    connection
        .execute(
            "INSERT INTO review_events(id, submission_id, session_id, card_id, reviewed_at,
                 rating, answer_correct, confidence, response_time_ms, state_before,
                 state_after, due_before, due_after, stability_before, stability_after,
                 difficulty_before, difficulty_after, scheduled_days_before,
                 scheduled_days_after, scheduler_name, scheduler_version, created_at)
             VALUES (?1, ?2, NULL, ?3, ?4, 3, 1, 'somewhat_confident', 1000, 2, 2, ?4, ?5,
                     3.0, 9.0, 5.0, 5.0, 3, 9, 'fsrs', 'rs-fsrs-1.2.1', ?4)",
            params![id, format!("5b{}", "f".repeat(30)), card, at, at + 9 * DAY],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO review_event_attributions(review_event_id, reviewed_at,
                 review_local_date, time_zone_id, utc_offset_minutes, card_id, note_id,
                 note_revision, owner_deck_id_at_review, captured_deck_name, direction,
                 card_type, card_label, presentation_variant, question_excerpt,
                 question_text_hash, question_was_truncated, attribution_source)
             SELECT ?1, ?2, date(?2 / 1000, 'unixepoch'), 'UTC', 0, c.id, c.note_id, 1,
                    c.deck_id, d.name, c.direction, 'basic', 'Basic', c.direction,
                    'question', ?3, 0, 'captured'
             FROM cards c JOIN decks d ON d.id = c.deck_id WHERE c.id = ?4",
            params![id, at, "0".repeat(64), card],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE card_schedules
             SET state = 2, due_at = ?2, stability = 9.0, scheduled_days = 9,
                 reps = reps + 1, last_review_at = ?3, updated_at = ?3,
                 revision = revision + 1
             WHERE card_id = ?1",
            params![card, at + 9 * DAY, at],
        )
        .unwrap();
}

/// A package as a build before release schema 6 writes it, with the frozen
/// public schema number 30 unless `schema_version` says otherwise.
fn handmade_package(
    snapshot: &Path,
    destination: &Path,
    schema_version: i64,
    selection: SyncSelection,
) {
    let package = Connection::open(destination).unwrap();
    package.execute_batch(PACKAGE_SCHEMA).unwrap();
    for (key, value) in [
        ("format", PACKAGE_FORMAT.to_string()),
        ("formatVersion", PACKAGE_VERSION.to_string()),
        ("schemaVersion", schema_version.to_string()),
        ("createdAt", T0.to_string()),
        ("applicationVersion", "0.1.0".to_string()),
        ("includeApiSettings", "false".to_string()),
        ("selectionJson", serde_json::to_string(&selection).unwrap()),
    ] {
        package
            .execute(
                "INSERT INTO package_metadata (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .unwrap();
    }
    package
        .execute(
            "INSERT INTO database_snapshot (id, data) VALUES (1, ?1)",
            [fs::read(snapshot).unwrap()],
        )
        .unwrap();
}

#[test]
fn an_updated_device_merges_a_package_from_a_device_without_the_update() {
    let scratch = Scratch::new("old-sender");
    let (common, library) = schema5_without_documents();
    // Both devices last synchronized at this state.
    let agreed = scratch.path("agreed.sqlite3");
    copy_to(&common, &agreed);
    let sender_file = scratch.path("sender.sqlite3");
    copy_to(&common, &sender_file);

    // This device upgraded. The other device, without the update, then studied
    // a Physics copy and edited a Biology copy that this device retired.
    let mut local = common;
    advance_release_library(&local, 5, 6);
    let merges_before = rows(&local, MERGES);
    let sender = Connection::open(&sender_file).unwrap();
    review_without_the_update(&sender, library.cm_phys[0].card(), T0 + 8 * DAY);
    sender
        .execute(
            "UPDATE notes SET revision = 2, front_text = 'edited on the other device',
                 updated_at = ?2
             WHERE id = ?1",
            params![library.cb[1].id, T0 + 8 * DAY],
        )
        .unwrap();
    drop(sender);
    let package = scratch.path("from-old-device.syncpkg");
    handmade_package(&sender_file, &package, 30, every_category());

    // The package is migrated to schema 6 before it is merged.
    let extracted = extract_package(&package, &scratch.path("staging")).unwrap();
    let remote = Connection::open(&extracted.database_path).unwrap();
    assert_eq!(
        remote
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        crate::database::CURRENT_SCHEMA_VERSION
    );
    let store = store(&scratch);
    let agreed = Connection::open(&agreed).unwrap();
    remember_sent_database(&agreed, &store, OTHER_DEVICE, extracted.selection).unwrap();

    merge_remote_database(
        &mut local,
        &remote,
        &store,
        THIS_DEVICE,
        OTHER_DEVICE,
        extracted.selection,
    )
    .unwrap();

    // The new review is on the retired copy and counts under its original.
    assert_eq!(rows(&local, MERGES), merges_before);
    let review_card: String = local
        .query_row(
            "SELECT card_id FROM review_events WHERE reviewed_at = ?1",
            [T0 + 8 * DAY],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(review_card, library.cm_phys[0].card());
    let mechanics = load_analytics(&mut local, all_time(vec![library.k_mech.clone()])).unwrap();
    assert_eq!(mechanics.history.review_count, 7);
    assert_eq!(mechanics.history.cards_practiced, 3);
    // The original takes the whole schedule of that newer review.
    let (last_review_at, due_at): (i64, i64) = local
        .query_row(
            "SELECT last_review_at, due_at FROM card_schedules WHERE card_id = ?1",
            [library.o_mech[0].card()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((last_review_at, due_at), (T0 + 8 * DAY, T0 + 17 * DAY));

    // The edited copy stays retired, with its new text, and is listed.
    let (deleted_at, text): (Option<i64>, String) = local
        .query_row(
            "SELECT deleted_at, front_text FROM notes WHERE id = ?1",
            [&library.cb[1].id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(deleted_at.is_some());
    assert_eq!(text, "edited on the other device");
    let listed = load_filing_reviews(&local).unwrap();
    let edited = listed
        .iter()
        .find(|review| review.entity_id == library.cb[1].id)
        .unwrap();
    assert_eq!(edited.reason, "sync_edited_retired_copy");
    assert_eq!(edited.label, "edited on the other device");
    assert_eq!(
        edited.detail,
        serde_json::json!({"originalNoteId": library.o_bio[1].id})
    );
    // Both devices filed every deck the same way.
    assert!(listed
        .iter()
        .all(|review| review.reason != "sync_filing_conflict"));
}

#[test]
fn packages_carry_the_release_schema_so_older_builds_ask_for_the_update() {
    let scratch = Scratch::new("schema-number");
    let database = Database::open_in_memory().unwrap();
    let paths = AppPaths::new(scratch.path("sender")).unwrap();
    let package = scratch.path("current.syncpkg");
    create_package(&database, &paths, &package, flashcards_only()).unwrap();
    let recorded: i64 = Connection::open(&package)
        .unwrap()
        .query_row(
            "SELECT value FROM package_metadata WHERE key = 'schemaVersion'",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap()
        .parse()
        .unwrap();
    // Builds before release schema 6 refuse any number above 30 with
    // "Update this device first".
    assert_eq!(recorded, 36);
    assert!(recorded > crate::database::LEGACY_PUBLIC_SCHEMA_VERSION);

    // This build refuses a package from a later release the same way.
    let (library, _) = schema5_without_documents();
    let snapshot = scratch.path("later.sqlite3");
    copy_to(&library, &snapshot);
    let later = scratch.path("later.syncpkg");
    handmade_package(&snapshot, &later, recorded + 1, flashcards_only());
    let Err(error) = extract_package(&later, &scratch.path("later-staging")) else {
        panic!("a package from a later release must be refused");
    };
    assert_eq!(error.code, "device_sync_package_invalid");
    assert_eq!(
        error.message,
        "The sending device uses a newer ostudy database schema. Update this device first."
    );
}

#[test]
fn subjects_and_card_merges_reach_a_new_device_with_flashcards_only() {
    let scratch = Scratch::new("round-trip");
    let (common, _) = schema5_without_documents();
    advance_release_library(&common, 5, 6);
    let sender_file = scratch.path("sender.sqlite3");
    copy_to(&common, &sender_file);
    let sender = Database::open(&sender_file).unwrap();
    let paths = AppPaths::new(scratch.path("sender")).unwrap();
    let package = scratch.path("flashcards.syncpkg");
    create_package(&sender, &paths, &package, flashcards_only()).unwrap();
    let extracted = extract_package(&package, &scratch.path("staging")).unwrap();
    let remote = Connection::open(&extracted.database_path).unwrap();

    let receiver = Database::open_in_memory().unwrap();
    let store = store(&scratch);
    receiver
        .with_connection(|local| {
            merge_remote_database(
                local,
                &remote,
                &store,
                THIS_DEVICE,
                OTHER_DEVICE,
                extracted.selection,
            )?;
            assert_eq!(rows(local, MERGES), rows(&common, MERGES));
            assert_eq!(rows(local, SUBJECTS), rows(&common, SUBJECTS));
            assert_eq!(rows(local, DECK_FILING), rows(&common, DECK_FILING));
            // Reviews of retired copies count under their originals here too.
            let analytics = load_analytics(local, all_time(Vec::new()))?;
            assert_eq!(analytics.history.review_count, 12);
            assert_eq!(analytics.history.cards_practiced, 8);
            Ok(())
        })
        .unwrap();
}

#[test]
fn each_category_keeps_subjects_and_only_its_own_rows() {
    let count = |connection: &Connection, table: &str| -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    };
    let upgraded = || {
        let connection = release_library(5);
        let library = seed(&connection);
        advance_release_library(&connection, 5, 6);
        (connection, library)
    };
    let (reference, _) = upgraded();
    let subjects = count(&reference, "document_collections");
    assert!(count(&reference, "document_study_ranges") > 0);
    assert!(count(&reference, "subject_filing_reviews") > 0);

    let (flashcards, _) = upgraded();
    sanitize_snapshot(&flashcards, flashcards_only()).unwrap();
    assert_eq!(count(&flashcards, "document_collections"), subjects);
    assert_eq!(rows(&flashcards, DECK_FILING), rows(&reference, DECK_FILING));
    assert_eq!(rows(&flashcards, MERGES), rows(&reference, MERGES));
    assert_eq!(count(&flashcards, "documents"), 0);
    assert_eq!(count(&flashcards, "document_study_ranges"), 0);
    assert_eq!(count(&flashcards, "subject_filing_reviews"), 0);

    let (documents, _) = upgraded();
    let documents_only = SyncSelection {
        flashcards: false,
        documents: true,
        ..flashcards_only()
    };
    sanitize_snapshot(&documents, documents_only).unwrap();
    assert_eq!(count(&documents, "document_collections"), subjects);
    let filing = "SELECT id, subject_id FROM documents ORDER BY id";
    assert_eq!(rows(&documents, filing), rows(&reference, filing));
    assert_eq!(
        count(&documents, "document_study_ranges"),
        count(&reference, "document_study_ranges")
    );
    assert_eq!(count(&documents, "decks"), 0);
    assert_eq!(count(&documents, "card_merges"), 0);
    // The fixture resets a card; its schedule action goes with the cards.
    assert!(count(&reference, "schedule_action_events") > 0);
    assert_eq!(count(&documents, "schedule_action_events"), 0);
    assert_eq!(count(&documents, "subject_filing_reviews"), 0);

    let (settings, _) = upgraded();
    let settings_only = SyncSelection {
        flashcards: false,
        api_settings: true,
        ..flashcards_only()
    };
    // Generated decks' document links leave with the documents and decks.
    assert!(count(&reference, "document_decks") > 0);
    sanitize_snapshot(&settings, settings_only).unwrap();
    assert_eq!(count(&settings, "document_collections"), 0);
    assert_eq!(count(&settings, "document_decks"), 0);
}

#[test]
fn a_deck_the_two_devices_filed_differently_is_listed() {
    let scratch = Scratch::new("filing-conflict");
    let connection = release_library(5);
    let library = seed(&connection);
    advance_release_library(&connection, 5, 6);
    let local_file = scratch.path("local.sqlite3");
    let remote_file = scratch.path("remote.sqlite3");
    copy_to(&connection, &local_file);
    copy_to(&connection, &remote_file);
    let store = store(&scratch);
    remember_sent_database(&connection, &store, OTHER_DEVICE, every_category()).unwrap();

    let mut local = Connection::open(&local_file).unwrap();
    let remote = Connection::open(&remote_file).unwrap();
    for (device, subject) in [(&local, &library.c_hist), (&remote, &library.c_unused)] {
        device
            .execute(
                "UPDATE decks SET subject_id = ?2 WHERE id = ?1",
                params![library.k_acids, subject],
            )
            .unwrap();
    }
    merge_remote_database(
        &mut local,
        &remote,
        &store,
        THIS_DEVICE,
        OTHER_DEVICE,
        every_category(),
    )
    .unwrap();

    // The device with the lower id wins the conflict, and the deck is listed.
    let subject: String = local
        .query_row(
            "SELECT subject_id FROM decks WHERE id = ?1",
            [&library.k_acids],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(&subject, &library.c_hist);
    let listed = load_filing_reviews(&local).unwrap();
    let acids = listed
        .iter()
        .find(|review| review.entity_id == library.k_acids)
        .unwrap();
    assert_eq!(acids.entity_kind, "deck");
    assert_eq!(acids.reason, "sync_filing_conflict");
    assert_eq!(
        acids.detail,
        serde_json::json!({"subjectIds": [library.c_hist, library.c_unused]})
    );
}
