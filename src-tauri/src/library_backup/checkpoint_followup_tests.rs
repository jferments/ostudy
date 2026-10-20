use super::*;
use rusqlite::params;
use serde_json::json;
use sha2::{Digest, Sha256};

/// The forward release migrations a schema-4 build applied, byte for byte.
const SCHEMA_FOUR_MIGRATIONS: &[(i64, &str, &str)] = &[
    (
        2,
        "selection_deck_operations",
        include_str!("../../schema/0002_selection_deck_operations.sql"),
    ),
    (
        3,
        "deck_merge_operations",
        include_str!("../../schema/0003_deck_merge_operations.sql"),
    ),
    (
        4,
        "study_target_generation_instructions",
        include_str!("../../schema/0004_study_target_generation_instructions.sql"),
    ),
];

/// Advance the schema-1 fixture exactly as the previous release did.
fn advance_to_schema_four(fixture: &Fixture) {
    let connection = Connection::open(fixture.0.database_path()).unwrap();
    for (version, name, sql) in SCHEMA_FOUR_MIGRATIONS {
        connection.execute_batch(sql).unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, 0)",
                params![
                    version,
                    name,
                    format!("{:x}", Sha256::digest(sql.as_bytes()))
                ],
            )
            .unwrap();
    }
    connection.pragma_update(None, "user_version", 4).unwrap();
    assert_eq!(validate_database(&connection).unwrap().1, 4);
}

fn review_context_rows(connection: &Connection) -> i64 {
    connection
        .query_row(
            "SELECT COUNT(*) FROM review_event_scheduler_context",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

/// What an upgrade must keep in a library with no folders, collections or
/// collection copies: every table, with decks compared without their filing
/// column (`folder_id` before schema 6, `subject_id` after), which is empty.
/// Tables a release adds empty, and the filing structure release 6 replaces,
/// are set aside after checking that they hold nothing.
#[derive(Debug, PartialEq)]
struct Kept {
    rows: std::collections::BTreeMap<String, (Vec<String>, Vec<[u8; 32]>)>,
    decks: Vec<Vec<rusqlite::types::Value>>,
}

fn kept(connection: &Connection) -> Kept {
    let mut rows = preservation::rows(connection).unwrap();
    for table in [
        "selection_deck_operations",
        "deck_merge_operations",
        "study_target_generation_instructions",
        "review_event_scheduler_context",
        "deck_folders",
        "document_collection_memberships",
        "document_collection_ranges",
        "document_collections",
        "documents",
        "document_study_ranges",
        "card_merges",
        "subject_filing_reviews",
        "study_activity_retained",
        "review_day_rollups",
        "review_day_rollup_built_decks",
        "document_annotations",
        "lessons",
        "lesson_revisions",
        "lesson_revision_units",
        "lesson_progress",
        "note_annotation_sources",
        "speech_settings",
        "speech_segments",
        "document_study_guide_section_reads",
        "document_study_guide_positions",
        "document_figure_scans",
        "document_figures",
        "document_figure_passages",
    ] {
        if let Some((_, values)) = rows.remove(table) {
            assert!(values.is_empty(), "{table} must be empty in this library");
        }
    }
    rows.remove("decks").expect("every schema has decks");
    let filing: String = connection
        .query_row(
            "SELECT name FROM pragma_table_info('decks')
             WHERE name IN ('folder_id', 'subject_id')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let filed: i64 = connection
        .query_row(
            &format!("SELECT COUNT(*) FROM decks WHERE {filing} IS NOT NULL"),
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(filed, 0, "decks in this library are unfiled");
    let mut statement = connection
        .prepare(
            "SELECT id, name, description, sort_order, created_at, updated_at,
                    archived_at, deleted_at, revision
             FROM decks ORDER BY id",
        )
        .unwrap();
    let decks = statement
        .query_map([], |row| (0..9).map(|index| row.get(index)).collect())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    Kept { rows, decks }
}

fn seed_history(fixture: &Fixture) {
    let mut connection = Connection::open(fixture.0.database_path()).unwrap();
    let deck = crate::decks::create_deck_in_connection(
        &mut connection,
        crate::decks::CreateDeckInput {
            name: "Upgrade preservation fixture".into(),
            description: Some("Synthetic retained study history".into()),
        },
    )
    .unwrap();
    let note = "1".repeat(32);
    let card = "2".repeat(32);
    let session = "3".repeat(32);
    let review = "4".repeat(32);
    let content = crate::cards::validate_content(
        json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Question"}]}]}),
        json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Learner-edited answer"}]}]}),
        String::new(),
        String::new(),
    ).unwrap();
    connection
        .execute(
            "INSERT INTO notes (id, deck_id, note_type, content_format, content_schema_version,
         front_doc, back_doc, front_text, back_text, created_at, updated_at, revision)
         VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, 10, 10, 1)",
            params![
                note,
                deck.id,
                content.front_doc_json,
                content.back_doc_json,
                content.front_text,
                content.back_text
            ],
        )
        .unwrap();
    connection.execute(
        "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'forward', 0, 10, 10)",
        params![card, note, deck.id],
    ).unwrap();
    crate::scheduler::insert_new_schedule(&connection, &card, 10).unwrap();
    connection
        .execute(
            "UPDATE card_schedules SET state = 2, due_at = 5000, reps = 7,
         stability = 3.0, difficulty = 4.5, revision = 4 WHERE card_id = ?1",
            [&card],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO practice_sessions (id, selected_deck_ids_json, shuffle, card_count,
         viewed_count, revealed_count, started_at, completed_at, rated_count, correct_count,
         mode, selected_tag_ids_json)
         VALUES (?1, ?2, 0, 1, 1, 1, 20, 30, 1, 1, 'scheduled', '[]')",
            params![session, json!([deck.id.clone()]).to_string()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO practice_session_cards (session_id, card_id, position, viewed_at,
         revealed_at, rated_at, rating, response_time_ms, presentation_variant)
         VALUES (?1, ?2, 0, 21, 22, 23, 3, 1200, 'forward')",
            params![session, card],
        )
        .unwrap();
    connection.execute(
        "INSERT INTO practice_session_card_snapshots (session_id, card_id, note_id, note_revision,
         owner_deck_id_at_capture, captured_deck_name, direction, card_type, card_label,
         presentation_variant, front_doc_json, back_doc_json, front_text, back_text,
         question_excerpt, question_text_hash, question_was_truncated, captured_at)
         VALUES (?1, ?2, ?3, 1, ?4, 'Upgrade preservation fixture', 'forward', 'basic', 'Basic',
         'forward', ?5, ?6, 'Question', 'Learner-edited answer', 'Question', ?7, 0, 20)",
        params![session, card, note, deck.id, content.front_doc_json, content.back_doc_json, "a".repeat(64)],
    ).unwrap();
    connection
        .execute(
            "INSERT INTO review_events (id, submission_id, session_id, card_id, reviewed_at,
         rating, answer_correct, confidence, response_time_ms, state_before, state_after,
         due_before, due_after, stability_before, stability_after, difficulty_before,
         difficulty_after, scheduled_days_before, scheduled_days_after, scheduler_name,
         scheduler_version, created_at, request_fingerprint, result_completed,
         result_rated_count, result_correct_count)
         VALUES (?1, ?2, ?3, ?4, 23, 3, 1, 'somewhat_confident', 1200, 0, 2, 10, 5000,
         0.0, 3.0, 0.0, 4.5, 0, 5, 'fsrs', 'test', 23, ?5, 1, 1, 1)",
            params![review, "5".repeat(32), session, card, "f".repeat(64)],
        )
        .unwrap();
    connection.execute(
        "INSERT INTO review_event_attributions (review_event_id, reviewed_at, review_local_date,
         time_zone_id, utc_offset_minutes, card_id, note_id, note_revision,
         owner_deck_id_at_review, captured_deck_name, direction, card_type, card_label,
         presentation_variant, question_excerpt, question_text_hash, question_was_truncated,
         attribution_source)
         VALUES (?1, 23, '1970-01-01', 'UTC', 0, ?2, ?3, 1, ?4, 'Upgrade preservation fixture',
         'forward', 'basic', 'Basic', 'forward', 'Question', ?5, 0, 'captured')",
        params![review, card, note, deck.id, "b".repeat(64)],
    ).unwrap();
    validate_database(&connection).expect("valid historical fixture");
}

#[test]
fn populated_upgrade_preserves_learner_edits_schedules_and_review_evidence() {
    let fixture = Fixture::new();
    seed_history(&fixture);
    let source = read_database(fixture.0.database_path()).unwrap();
    let before = preservation::rows(&source).unwrap();
    assert_eq!(before["notes"].1.len(), 1);
    assert_eq!(before["review_events"].1.len(), 1);
    let kept_before = kept(&source);
    drop(source);
    let original = fs::read(fixture.0.database_path()).unwrap();
    let prepared = fixture.prepare();
    assert_eq!(fs::read(fixture.0.database_path()).unwrap(), original);
    let snapshot = read_database(&prepared.transaction.join("checkpoint/library.sqlite3")).unwrap();
    assert_eq!(preservation::rows(&snapshot).unwrap(), before);
    drop(snapshot);
    let mut guard = install_prepared_restore(&fixture.0, prepared).unwrap();
    let active = read_database(fixture.0.database_path()).unwrap();
    assert_eq!(kept(&active), kept_before);
    drop(active);
    commit_upgrade(&fixture.0, &mut guard).unwrap();
    fixture.original_managed_files_are_present();
}

#[test]
fn schema_four_reviews_upgrade_to_current_without_invented_scheduler_context() {
    let fixture = Fixture::new();
    advance_to_schema_four(&fixture);
    seed_history(&fixture);
    let source = read_database(fixture.0.database_path()).unwrap();
    let before = preservation::rows(&source).unwrap();
    assert_eq!(before["review_events"].1.len(), 1);
    assert_eq!(before["review_event_attributions"].1.len(), 1);
    assert!(!before.contains_key("review_event_scheduler_context"));
    let kept_before = kept(&source);
    drop(source);

    let mut guard = begin_upgrade(
        &fixture.0,
        APPLICATION,
        (crate::database::CURRENT_SCHEMA_LINEAGE, 4),
        &mut |_| {},
    )
    .unwrap();
    let manifest = verify_checkpoint(&fixture.transaction()).unwrap();
    assert_eq!(
        (manifest.source_schema, manifest.target_schema),
        (4, crate::database::CURRENT_SCHEMA_VERSION)
    );
    let checkpoint =
        read_database(&fixture.transaction().join("checkpoint/library.sqlite3")).unwrap();
    assert_eq!(validate_database(&checkpoint).unwrap().1, 4);
    assert_eq!(preservation::rows(&checkpoint).unwrap(), before);
    drop(checkpoint);

    let active = read_database(fixture.0.database_path()).unwrap();
    assert_eq!(
        validate_database(&active).unwrap().1,
        crate::database::CURRENT_SCHEMA_VERSION
    );
    assert_eq!(
        review_context_rows(&active),
        0,
        "old reviews get no side row"
    );
    assert_eq!(
        kept(&active),
        kept_before,
        "every retained review value is unchanged"
    );
    drop(active);
    commit_upgrade(&fixture.0, &mut guard).unwrap();
    drop(guard);
    admit_startup(&fixture.0).unwrap();
    assert_eq!(fixture.version(), crate::database::CURRENT_SCHEMA_VERSION);
    fixture.original_managed_files_are_present();
}

#[test]
fn upgrade_checkpoint_restores_the_schema_four_library_for_an_older_build() {
    let fixture = Fixture::new();
    advance_to_schema_four(&fixture);
    seed_history(&fixture);
    let before = preservation::rows(&read_database(fixture.0.database_path()).unwrap()).unwrap();
    let mut guard = begin_upgrade(
        &fixture.0,
        APPLICATION,
        (crate::database::CURRENT_SCHEMA_LINEAGE, 4),
        &mut |_| {},
    )
    .unwrap();
    // The activation committed while the recovery dialog still owns the
    // upgrade, so the learner can choose to restore the schema-4 checkpoint.
    guard.try_commit().unwrap();
    drop(guard);
    assert_eq!(fixture.version(), crate::database::CURRENT_SCHEMA_VERSION);

    restore_checkpoint(&fixture.0, &fixture.operation(), &mut |_| {}).unwrap();
    assert_eq!(fixture.version(), 4);
    let restored = read_database(fixture.0.database_path()).unwrap();
    assert_eq!(preservation::rows(&restored).unwrap(), before);
    drop(restored);
    // This build waits for an explicit decision; a schema-4 build reopens it.
    assert!(admit_startup(&fixture.0).is_err());
    admit_startup_for_schema(&fixture.0, crate::database::CURRENT_SCHEMA_LINEAGE, 4).unwrap();
    assert_eq!(fixture.version(), 4);
    fixture.original_managed_files_are_present();
}

#[test]
fn a_valid_but_lossy_private_migration_is_rejected_even_when_row_counts_match() {
    let fixture = Fixture::new();
    seed_history(&fixture);
    let original = fs::read(fixture.0.database_path()).unwrap();
    let error = prepare_upgrade_with(
        &fixture.0,
        APPLICATION,
        (crate::database::CURRENT_SCHEMA_LINEAGE, 1),
        &mut |_| {},
        None,
        |path| {
            let database = Database::open(path)?;
            database
                .with_connection(|connection| {
                    connection
                        .execute("UPDATE card_schedules SET due_at = 6000", [])
                        .map_err(crate::error::CommandError::database)?;
                    Ok(())
                })
                .map_err(|error| error.message)?;
            settle_candidate(&database)
        },
    )
    .expect_err("equal-count value loss must fail preservation");
    assert!(error.contains("changed retained library values"), "{error}");
    assert_eq!(fixture.version(), 1);
    assert_eq!(fs::read(fixture.0.database_path()).unwrap(), original);
    assert!(!fixture.0.app_data_dir().join(JOURNAL).exists());
}

/// Prepare an upgrade whose private migration also runs `sql`, as a faulty
/// release migration would, and return the refusal. The library is untouched.
pub(super) fn refused_upgrade(fixture: &Fixture, source_schema: i64, sql: &str) -> String {
    let original = fs::read(fixture.0.database_path()).unwrap();
    let error = prepare_upgrade_with(
        &fixture.0,
        APPLICATION,
        (crate::database::CURRENT_SCHEMA_LINEAGE, source_schema),
        &mut |_| {},
        None,
        |path| {
            let database = Database::open(path)?;
            database
                .with_connection(|connection| {
                    connection
                        .execute_batch(sql)
                        .map_err(crate::error::CommandError::database)
                })
                .map_err(|error| error.message)?;
            settle_candidate(&database)
        },
    )
    .expect_err(sql);
    assert_eq!(fixture.version(), source_schema);
    assert_eq!(fs::read(fixture.0.database_path()).unwrap(), original);
    assert!(!fixture.0.app_data_dir().join(JOURNAL).exists());
    error
}

#[test]
fn an_upgrade_that_rewrites_or_backfills_protected_review_evidence_is_rejected() {
    for (source_schema, sql, refusal) in [
        (
            1,
            "UPDATE review_events SET rating = 4",
            "changed retained library values",
        ),
        (
            1,
            "UPDATE review_event_attributions SET captured_deck_name = 'Biology'",
            "changed retained library values",
        ),
        (
            1,
            "INSERT INTO review_event_tag_attributions
             SELECT review_event_id, printf('%032d', 7), 'Biology' FROM review_event_attributions",
            "changed retained library values",
        ),
        // A schema-4 source has no scheduler-context table yet, so schema 5's
        // rule that the table arrives empty is what refuses invented context.
        (
            4,
            "INSERT INTO review_event_scheduler_context (review_event_id, elapsed_days,
             reps_before, lapses_before, desired_retention, scheduler_parameters_sha256)
             SELECT id, 0, 0, 0, 0.9, printf('%064d', 0) FROM review_events",
            "unexpectedly created review scheduler-context rows",
        ),
    ] {
        let fixture = Fixture::new();
        if source_schema == 4 {
            advance_to_schema_four(&fixture);
        }
        seed_history(&fixture);
        let error = refused_upgrade(&fixture, source_schema, sql);
        assert!(error.contains(refusal), "{sql}: {error}");
    }
}

#[test]
fn an_upgrade_that_changes_a_table_no_contract_lists_is_rejected() {
    for sql in [
        // A new column changes the shape of a table no release declares.
        "ALTER TABLE tags ADD COLUMN color TEXT",
        // A table that no release declares, even an empty one.
        "CREATE TABLE subjects (id TEXT PRIMARY KEY)",
    ] {
        let fixture = Fixture::new();
        seed_history(&fixture);
        let error = refused_upgrade(&fixture, 1, sql);
        assert!(
            error.contains("changed retained library values"),
            "{sql}: {error}"
        );
    }
}

#[test]
fn removed_or_new_managed_roots_are_not_mistaken_for_an_unchanged_source() {
    for originally_present in [true, false] {
        let fixture = Fixture::new();
        let source = fixture.0.app_data_dir().join("documents");
        let retained = fixture.0.app_data_dir().join("external-retained-documents");
        if !originally_present {
            fs::rename(&source, &retained).unwrap();
        }
        let error = prepare_upgrade_with(
            &fixture.0,
            APPLICATION,
            (crate::database::CURRENT_SCHEMA_LINEAGE, 1),
            &mut |_| {},
            None,
            |path| {
                let database = Database::open(path)?;
                settle_candidate(&database)?;
                if originally_present {
                    fs::rename(&source, &retained).unwrap();
                } else {
                    fs::create_dir(&source).unwrap();
                }
                Ok(())
            },
        )
        .expect_err("external filesystem change must block activation");
        assert!(error.contains("Managed files changed"), "{error}");
        assert_eq!(fixture.version(), 1);
        assert_eq!(
            fs::read(retained.join("retained.bin")).unwrap(),
            b"documents"
        );
        assert!(!fixture.0.app_data_dir().join(JOURNAL).exists());
    }
}

#[test]
fn a_compatible_previous_executable_can_open_a_restored_checkpoint_without_migrating() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare();
    let mut guard = install_prepared_restore(&fixture.0, prepared).unwrap();
    guard.try_commit().unwrap();
    drop(guard);
    restore_checkpoint(&fixture.0, &fixture.operation(), &mut |_| {}).unwrap();
    assert!(admit_startup(&fixture.0).is_err());
    let before = fs::read(fixture.0.database_path()).unwrap();
    // Exercise the compatibility decision with the older executable's target.
    // A real installation/downgrade remains a separate package acceptance test.
    admit_startup_for_schema(&fixture.0, crate::database::CURRENT_SCHEMA_LINEAGE, 1).unwrap();
    assert_eq!(fixture.version(), 1);
    assert_eq!(fs::read(fixture.0.database_path()).unwrap(), before);
    assert!(!has_intent(&fixture.0).unwrap());
}

#[test]
fn committed_bookkeeping_failure_does_not_undo_or_repeat_a_successful_upgrade() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare();
    let transaction = prepared.transaction.clone();
    let mut guard = install_prepared_restore(&fixture.0, prepared).unwrap();
    guard.try_commit().unwrap();
    drop(guard);
    fs::write(
        transaction.join("completed-intent.json"),
        b"conflicting evidence",
    )
    .unwrap();
    admit_startup(&fixture.0).expect("committed library stays usable");
    assert_eq!(fixture.version(), crate::database::CURRENT_SCHEMA_VERSION);
    assert!(has_intent(&fixture.0).unwrap());
    assert_eq!(
        fs::read(transaction.join("completed-intent.json")).unwrap(),
        b"conflicting evidence"
    );
    verify_checkpoint(&transaction).unwrap();
}

#[test]
fn stale_recovery_choices_cannot_replace_or_acknowledge_another_attempt() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare();
    drop(prepared);
    let operation = fixture.operation();
    assert!(allow_retry(&fixture.0, &"f".repeat(32)).is_err());
    assert!(restore_checkpoint(&fixture.0, &"f".repeat(32), &mut |_| {}).is_err());
    assert_eq!(fixture.operation(), operation);
    assert_eq!(fixture.version(), 1);
    assert!(!fixture.0.app_data_dir().join(JOURNAL).exists());
}

#[test]
fn preparation_crash_child() {
    let Some(root) = std::env::var_os("OSTUDY_UPGRADE_PREPARATION_ROOT") else {
        return;
    };
    let paths = AppPaths::new(PathBuf::from(root)).unwrap();
    let phase = std::env::var("OSTUDY_UPGRADE_PREPARATION_PHASE").unwrap();
    let _guard = begin_upgrade(
        &paths,
        APPLICATION,
        (crate::database::CURRENT_SCHEMA_LINEAGE, 1),
        &mut |message| {
            if message == phase {
                std::process::exit(77);
            }
        },
    )
    .unwrap();
    panic!("requested preparation cutpoint was not reached");
}

#[test]
fn preparation_crashes_preserve_the_original_and_require_an_explicit_retry() {
    for phase in [
        "Creating a recovery checkpoint…",
        "Updating a private library copy…",
        "Verifying the updated library…",
    ] {
        let fixture = Fixture::new();
        seed_history(&fixture);
        let before = fs::read(fixture.0.database_path()).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "library_backup::activation::checkpoint::tests::followup::preparation_crash_child",
                "--test-threads=1",
            ])
            .env("OSTUDY_UPGRADE_PREPARATION_ROOT", fixture.0.app_data_dir())
            .env("OSTUDY_UPGRADE_PREPARATION_PHASE", phase)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(77), "{phase}");
        assert!(super::super::super::apply_pending_restore(&fixture.0)
            .unwrap()
            .is_none());
        assert!(admit_startup(&fixture.0).is_err());
        assert_eq!(fs::read(fixture.0.database_path()).unwrap(), before);
        fixture.original_managed_files_are_present();
    }
}

#[test]
fn populated_crash_restore_and_explicit_retry_preserve_study_data_and_newer_copy() {
    let fixture = Fixture::new();
    seed_history(&fixture);
    let source = read_database(fixture.0.database_path()).unwrap();
    let before = preservation::rows(&source).unwrap();
    let kept_before = kept(&source);
    for table in [
        "notes",
        "cards",
        "card_schedules",
        "practice_session_card_snapshots",
        "review_events",
        "review_event_attributions",
    ] {
        assert_eq!(before[table].1.len(), 1, "fixture must contain {table}");
    }
    drop(source);
    let original_bytes = fs::read(fixture.0.database_path()).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "library_backup::activation::checkpoint::tests::followup::preparation_crash_child",
            "--test-threads=1",
        ])
        .env("OSTUDY_UPGRADE_PREPARATION_ROOT", fixture.0.app_data_dir())
        .env(
            "OSTUDY_UPGRADE_PREPARATION_PHASE",
            "Updating a private library copy…",
        )
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(77));
    assert!(admit_startup(&fixture.0).is_err());
    assert_eq!(fs::read(fixture.0.database_path()).unwrap(), original_bytes);
    let first_attempt = fixture.transaction();
    verify_checkpoint(&first_attempt).unwrap();

    // Simulate later work to prove the warning about restoring an older backup:
    // those values leave the active library but remain in the saved newer copy.
    let later = Connection::open(fixture.0.database_path()).unwrap();
    later
        .execute("UPDATE card_schedules SET due_at = 6000", [])
        .unwrap();
    let newer_values = preservation::rows(&later).unwrap();
    assert_ne!(newer_values, before);
    drop(later);
    fs::write(
        fixture.0.app_data_dir().join("documents/retained.bin"),
        b"newer document bytes",
    )
    .unwrap();

    restore_checkpoint(&fixture.0, &fixture.operation(), &mut |_| {}).unwrap();
    let restored = read_database(fixture.0.database_path()).unwrap();
    assert_eq!(validate_database(&restored).unwrap().1, 1);
    assert_eq!(preservation::rows(&restored).unwrap(), before);
    drop(restored);
    fixture.original_managed_files_are_present();
    let restoration = fixture.transaction();
    let saved_newer = read_database(&restoration.join("previous/library.sqlite3")).unwrap();
    assert_eq!(preservation::rows(&saved_newer).unwrap(), newer_values);
    drop(saved_newer);
    assert_eq!(
        fs::read(restoration.join("previous/documents/retained.bin")).unwrap(),
        b"newer document bytes"
    );

    assert!(admit_startup(&fixture.0).is_err());
    let restored_bytes = fs::read(fixture.0.database_path()).unwrap();
    allow_retry(&fixture.0, &fixture.operation()).unwrap();
    assert_eq!(fs::read(fixture.0.database_path()).unwrap(), restored_bytes);
    assert_eq!(fixture.version(), 1, "allowing retry is not running it");
    let mut retry = begin_upgrade(
        &fixture.0,
        APPLICATION,
        (crate::database::CURRENT_SCHEMA_LINEAGE, 1),
        &mut |_| {},
    )
    .unwrap();
    let retried = read_database(fixture.0.database_path()).unwrap();
    assert_eq!(kept(&retried), kept_before);
    drop(retried);
    commit_upgrade(&fixture.0, &mut retry).unwrap();
    drop(retry);
    admit_startup(&fixture.0).unwrap();
    assert_eq!(fixture.version(), crate::database::CURRENT_SCHEMA_VERSION);
    fixture.original_managed_files_are_present();
    verify_checkpoint(&first_attempt).unwrap();
    verify_checkpoint(&restoration).unwrap();
}

#[test]
fn an_update_reports_rising_progress_that_its_plan_nearly_fills() {
    let fixture = Fixture::new();
    advance_to_schema_four(&fixture);
    seed_history(&fixture);
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let record = std::rc::Rc::clone(&seen);
    let attached = super::super::meter::attach(move |permille| record.borrow_mut().push(permille));
    let _guard = begin_upgrade(
        &fixture.0,
        APPLICATION,
        (crate::database::CURRENT_SCHEMA_LINEAGE, 4),
        &mut |_| {},
    )
    .unwrap();
    let before_finish = seen.borrow().clone();
    super::super::meter::finish();
    drop(attached);

    assert!(
        before_finish.windows(2).all(|pair| pair[0] < pair[1]),
        "{before_finish:?}"
    );
    // Only the activated library's own checks remain after preparation, so
    // the plan should be nearly used up, never overrun.
    let last = *before_finish.last().unwrap();
    assert!((800..=990).contains(&last), "{before_finish:?}");
    assert_eq!(seen.borrow().last(), Some(&1000));
}
