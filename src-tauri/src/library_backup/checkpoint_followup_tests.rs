use super::*;
use rusqlite::params;
use serde_json::json;

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
    drop(source);
    let original = fs::read(fixture.0.database_path()).unwrap();
    let prepared = fixture.prepare();
    assert_eq!(fs::read(fixture.0.database_path()).unwrap(), original);
    let snapshot = read_database(&prepared.transaction.join("checkpoint/library.sqlite3")).unwrap();
    assert_eq!(preservation::rows(&snapshot).unwrap(), before);
    drop(snapshot);
    let mut guard = install_prepared_restore(&fixture.0, prepared).unwrap();
    let active = read_database(fixture.0.database_path()).unwrap();
    let mut after = preservation::rows(&active).unwrap();
    assert!(after
        .remove("selection_deck_operations")
        .unwrap()
        .1
        .is_empty());
    assert_eq!(before, after);
    drop(active);
    commit_upgrade(&fixture.0, &mut guard).unwrap();
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
