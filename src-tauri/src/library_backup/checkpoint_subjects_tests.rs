//! Release 6 (subjects) through the checkpointed upgrade. The real migration
//! passes its preservation contract on a library that uses every filing and
//! copy-merging choice, a faulty migration is refused before activation, and
//! the checkpoint gives an older build its schema-5 library back.

use super::followup::refused_upgrade;
use super::*;
use crate::database::subjects_fixture::{self, SubjectsLibrary, DAY, T0};
use crate::database::CURRENT_SCHEMA_LINEAGE;

/// Tutor rows that retiring a collection copy invalidates through every
/// knowledge trigger, beyond the fixture's own sources and checkpoints.
struct Cascade {
    document_source: String,
    unit: String,
    window: String,
    conversation: String,
    /// Cites the retired copy's source.
    first_turn: String,
    /// The first turn's conversation source.
    conversation_source: String,
    /// Cites the first turn's conversation source. SQLite does not fire a
    /// trigger from inside itself, so retiring a note leaves this turn current
    /// at runtime, and the upgrade does the same.
    second_turn: String,
    /// Cites only the original's source, which stays active.
    third_turn: String,
}

fn hex(byte: char) -> String {
    byte.to_string().repeat(64)
}

fn seed_cascade(connection: &Connection, library: &SubjectsLibrary) -> Cascade {
    let at = T0 + 3 * DAY;
    let cascade = Cascade {
        document_source: format!("{:032x}", 0x5d0c_u64),
        unit: format!("{:032x}", 0x0417_u64),
        window: format!("{:032x}", 0x0a1d_u64),
        conversation: format!("{:032x}", 0xc0de_u64),
        first_turn: format!("{:032x}", 0x7101_u64),
        conversation_source: format!("{:032x}", 0x5c01_u64),
        second_turn: format!("{:032x}", 0x7102_u64),
        third_turn: format!("{:032x}", 0x7103_u64),
    };
    let execute = |sql: &str, values: &[&dyn rusqlite::ToSql]| {
        connection
            .execute(sql, values)
            .unwrap_or_else(|error| panic!("{error}: {sql}"));
    };
    execute(
        "INSERT INTO knowledge_sources(id, kind, owner_id, segment_key, parent_key,
             owner_revision, content_hash, record_json, active, document_id,
             extraction_contract, extracted_hash, created_at, updated_at)
         VALUES (?1, 'document', ?2, 'document', 'document', '1', ?3, '{}', 1, ?2, ?4, ?5,
                 ?6, ?6)",
        &[
            &cascade.document_source,
            &library.d_cells,
            &hex('8'),
            &hex('2'),
            &hex('3'),
            &at,
        ],
    );
    // The original's source holds a unit whose evidence is the copy's source.
    let unit = serde_json::json!({
        "id": cascade.unit,
        "sourceId": library.original_source,
        "sourceHash": hex('1'),
        "origin": "user_authored",
        "content": {
            "title": "Cells divide",
            "kind": "fact",
            "statement": "Cells divide.",
            "qualifications": [],
            "topic": "Biology",
            "supportingExcerpt": "Cells divide."
        }
    });
    execute(
        "INSERT INTO knowledge_units(id, source_id, source_hash, content_hash, record_json,
             extraction_contract, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        &[
            &cascade.unit,
            &library.original_source,
            &hex('1'),
            &hex('9'),
            &unit.to_string(),
            &hex('2'),
            &at,
        ],
    );
    execute(
        "INSERT INTO knowledge_unit_evidence(unit_id, evidence_id, source_id, source_hash)
         VALUES (?1, ?2, ?3, ?4)",
        &[
            &cascade.unit,
            &"e".repeat(32),
            &library.copy_source,
            &hex('1'),
        ],
    );
    execute(
        "INSERT INTO knowledge_artifact_links(source_id, unit_id, source_hash, relationship,
             created_at)
         VALUES (?1, ?2, ?3, 'mentions', ?4)",
        &[&library.copy_source, &cascade.unit, &hex('1'), &at],
    );
    // One analysis window read the copy and the document together.
    execute(
        "INSERT INTO knowledge_analysis_windows(id, document_id, contract, state,
             accepted_count, rejected_count, report_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'complete', 0, 0,
                 '{\"accepted\":[],\"rejected\":[],\"noLearning\":[],\"missingSources\":[]}',
                 ?4, ?4)",
        &[&cascade.window, &library.d_cells, &hex('2'), &at],
    );
    for source in [&library.copy_source, &cascade.document_source] {
        execute(
            "INSERT INTO knowledge_analysis_sources(window_id, source_id, source_hash)
             VALUES (?1, ?2, ?3)",
            &[&cascade.window, source, &hex('1')],
        );
    }
    execute(
        "INSERT INTO knowledge_document_views(document_id, analysis_hash, contract,
             outline_json, core_unit_ids_json, reduced, created_at)
         VALUES (?1, ?2, ?3, '{}', '[]', 0, ?4)",
        &[&library.d_cells, &hex('a'), &hex('2'), &at],
    );
    execute(
        "INSERT INTO tutor_conversations(id, operation_id, title, scope_json, summary_text,
             summary_through_sequence, revision, created_at, updated_at, history_enabled,
             retrieval_enabled, memory_start_sequence)
         VALUES (?1, ?2, 'Cells', '{}', 'What we covered', 3, 1, ?3, ?3, 1, 1, 0)",
        &[&cascade.conversation, &"0c".repeat(16), &at],
    );
    for (sequence, turn) in [
        (1_i64, &cascade.first_turn),
        (2, &cascade.second_turn),
        (3, &cascade.third_turn),
    ] {
        execute(
            "INSERT INTO tutor_turns(id, conversation_id, operation_id, sequence, request_hash,
                 request_json, mode, user_text, assistant_text, state, evidence_current,
                 profile_revision, provider_kind, model_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, '{}', 'explain', 'Why?', 'Because.', 'complete', 1,
                     1, 'local', 'local-model', ?6, ?6)",
            &[
                turn,
                &cascade.conversation,
                &format!("{:032x}", 0x0900 + sequence),
                &sequence,
                &hex('1'),
                &at,
            ],
        );
    }
    execute(
        "INSERT INTO knowledge_sources(id, kind, owner_id, segment_key, parent_key,
             owner_revision, content_hash, record_json, active, conversation_id, turn_id,
             created_at, updated_at)
         VALUES (?1, 'conversation', ?2, 'turn-1', 'conversation', '1', ?3, '{}', 1, ?2, ?4,
                 ?5, ?5)",
        &[
            &cascade.conversation_source,
            &cascade.conversation,
            &hex('b'),
            &cascade.first_turn,
            &at,
        ],
    );
    for (turn, source) in [
        (&cascade.first_turn, &library.copy_source),
        (&cascade.second_turn, &cascade.conversation_source),
        (&cascade.third_turn, &library.original_source),
    ] {
        execute(
            "INSERT INTO tutor_turn_sources(turn_id, source_id, content_hash)
             VALUES (?1, ?2, ?3)",
            &[turn, source, &hex('1')],
        );
    }
    cascade
}

/// The schema-5 subjects library in the fixture's file, as a schema-5 build
/// left it.
fn subjects_library(fixture: &Fixture) -> (SubjectsLibrary, Cascade) {
    let connection = Connection::open(fixture.0.database_path()).unwrap();
    subjects_fixture::advance_release_library(&connection, 1, 5);
    let library = subjects_fixture::seed(&connection);
    let cascade = seed_cascade(&connection, &library);
    assert_eq!(
        validate_database(&connection).unwrap(),
        (CURRENT_SCHEMA_LINEAGE.to_string(), 5)
    );
    (library, cascade)
}

fn count(connection: &Connection, sql: &str, value: &str) -> i64 {
    connection
        .query_row(sql, [value], |row| row.get(0))
        .unwrap_or_else(|error| panic!("{error}: {sql}"))
}

#[test]
fn the_subjects_upgrade_passes_its_preservation_contract_and_activates() {
    let fixture = Fixture::new();
    let (library, cascade) = subjects_library(&fixture);
    let source = read_database(fixture.0.database_path()).unwrap();
    let before = preservation::rows(&source).unwrap();
    drop(source);

    let mut guard = begin_upgrade(
        &fixture.0,
        APPLICATION,
        (CURRENT_SCHEMA_LINEAGE, 5),
        &mut |_| {},
    )
    .expect("the real migration satisfies the schema-6 contract");
    let checkpoint =
        read_database(&fixture.transaction().join("checkpoint/library.sqlite3")).unwrap();
    assert_eq!(preservation::rows(&checkpoint).unwrap(), before);
    drop(checkpoint);

    let active = read_database(fixture.0.database_path()).unwrap();
    assert_eq!(
        validate_database(&active).unwrap(),
        (CURRENT_SCHEMA_LINEAGE.to_string(), 6)
    );
    let after = preservation::rows(&active).unwrap();
    for table in [
        "review_events",
        "review_event_attributions",
        "review_event_scheduler_context",
        "question_attempts",
        "question_review_links",
        "practice_session_cards",
        "schedule_action_events",
        "study_target_card_origins",
        "study_targets",
    ] {
        assert_eq!(after[table], before[table], "{table} is review evidence");
    }
    let merges: i64 = active
        .query_row("SELECT COUNT(*) FROM card_merges", [], |row| row.get(0))
        .unwrap();
    assert_eq!(merges as usize, library.expected_merges().len());

    // The copy's source fell, and with it everything that cited it: the unit
    // and analysis window built on it, the turn that quoted it with that
    // turn's own conversation source, and the conversation summary. The
    // original's source and the document's stay active with their extraction
    // cleared, as the knowledge triggers do when a note is deleted.
    for (sql, id, expected) in [
        (
            "SELECT active FROM knowledge_sources WHERE id = ?1",
            &library.copy_source,
            0,
        ),
        (
            "SELECT active FROM knowledge_sources WHERE id = ?1",
            &cascade.conversation_source,
            0,
        ),
        (
            "SELECT active + length(extraction_contract) FROM knowledge_sources WHERE id = ?1",
            &library.original_source,
            1,
        ),
        (
            "SELECT active + length(extracted_hash) FROM knowledge_sources WHERE id = ?1",
            &cascade.document_source,
            1,
        ),
        (
            "SELECT COUNT(*) FROM knowledge_units WHERE id = ?1",
            &cascade.unit,
            0,
        ),
        (
            "SELECT COUNT(*) FROM knowledge_analysis_windows WHERE id = ?1",
            &cascade.window,
            0,
        ),
        (
            "SELECT COUNT(*) FROM knowledge_document_views WHERE document_id = ?1",
            &library.d_cells,
            0,
        ),
        (
            "SELECT evidence_current FROM tutor_turns WHERE id = ?1",
            &cascade.first_turn,
            0,
        ),
        (
            "SELECT evidence_current FROM tutor_turns WHERE id = ?1",
            &cascade.second_turn,
            1,
        ),
        (
            "SELECT evidence_current FROM tutor_turns WHERE id = ?1",
            &cascade.third_turn,
            1,
        ),
        (
            "SELECT length(summary_text) + summary_through_sequence
             FROM tutor_conversations WHERE id = ?1",
            &cascade.conversation,
            0,
        ),
        (
            "SELECT COUNT(*) FROM collection_guide_checkpoints WHERE cache_key = ?1",
            &library.copy_checkpoint,
            0,
        ),
    ] {
        assert_eq!(count(&active, sql, id), expected, "{sql} {id}");
    }
    drop(active);
    commit_upgrade(&fixture.0, &mut guard).unwrap();
    drop(guard);
    admit_startup(&fixture.0).unwrap();
    assert_eq!(fixture.version(), 6);
    fixture.original_managed_files_are_present();
}

#[test]
fn the_checkpoint_restores_the_schema_five_library_for_an_older_build() {
    let fixture = Fixture::new();
    subjects_library(&fixture);
    let before = preservation::rows(&read_database(fixture.0.database_path()).unwrap()).unwrap();
    let mut guard = begin_upgrade(
        &fixture.0,
        APPLICATION,
        (CURRENT_SCHEMA_LINEAGE, 5),
        &mut |_| {},
    )
    .unwrap();
    guard.try_commit().unwrap();
    drop(guard);
    assert_eq!(fixture.version(), 6);

    restore_checkpoint(&fixture.0, &fixture.operation(), &mut |_| {}).unwrap();
    assert_eq!(fixture.version(), 5);
    let restored = read_database(fixture.0.database_path()).unwrap();
    assert_eq!(preservation::rows(&restored).unwrap(), before);
    drop(restored);
    // This build waits for an explicit decision; a schema-5 build reopens the
    // folders, collections and copies exactly as it left them.
    assert!(admit_startup(&fixture.0).is_err());
    admit_startup_for_schema(&fixture.0, CURRENT_SCHEMA_LINEAGE, 5).unwrap();
    assert_eq!(fixture.version(), 5);
    fixture.original_managed_files_are_present();
}

/// The SQL a faulty migration would also run, for one fixture library.
type Fault = fn(&SubjectsLibrary, &Cascade) -> String;

#[test]
fn a_faulty_subjects_migration_is_refused_before_activation() {
    // Each change runs after the real migration in the private copy, as a
    // faulty release migration would, and is refused with the library intact.
    let cases: &[(&str, Fault, &str)] = &[
        (
            "a merged copy's review is moved onto the surviving card",
            |_, _| {
                "UPDATE review_events SET card_id = (
                     SELECT surviving_card_id FROM card_merges
                     WHERE retired_card_id = review_events.card_id)
                 WHERE card_id IN (SELECT retired_card_id FROM card_merges)"
                    .into()
            },
            "changed retained library values",
        ),
        (
            "a merged copy's review evidence is rewritten",
            |_, _| {
                "UPDATE review_events SET rating = 1
                 WHERE card_id IN (SELECT retired_card_id FROM card_merges)"
                    .into()
            },
            "changed retained library values",
        ),
        (
            "a merged copy's review attribution is moved onto the original note",
            |_, _| {
                "UPDATE review_event_attributions SET note_id = (
                     SELECT surviving_note_id FROM card_merges
                     WHERE retired_card_id = review_event_attributions.card_id)
                 WHERE card_id IN (SELECT retired_card_id FROM card_merges)"
                    .into()
            },
            "changed retained library values",
        ),
        (
            "a retired copy's content changes",
            |library, _| {
                format!(
                    "UPDATE notes SET front_text = 'rewritten' WHERE id = '{}'",
                    library.cb[0].id
                )
            },
            "changed retained library values",
        ),
        (
            "an undeclared table appears",
            |_, _| "CREATE TABLE subject_colors (id TEXT PRIMARY KEY)".into(),
            "changed retained library values",
        ),
        (
            "an unfiled document is filed without a reason",
            |library, _| {
                format!(
                    "UPDATE documents SET subject_id = '{}' WHERE id = '{}'",
                    library.c_unused, library.d_unfiled
                )
            },
            "changes to documents",
        ),
        (
            "a document loses its first collection",
            |library, _| {
                format!(
                    "UPDATE documents SET subject_id = '{}' WHERE id = '{}'",
                    library.c_bio, library.d_mech
                )
            },
            "changes to collection memberships",
        ),
        (
            "a kept page range is lost",
            |library, _| {
                format!(
                    "DELETE FROM document_study_ranges WHERE id = '{}'",
                    library.kept_ranges[0]
                )
            },
            "changes to collection page ranges",
        ),
        (
            "a folder's subject is renamed",
            |library, _| {
                format!(
                    "UPDATE document_collections SET name = 'Chem' WHERE id = '{}'",
                    library.f_chem
                )
            },
            "changes to deck folders",
        ),
        (
            "subjects are reordered",
            |library, _| {
                format!(
                    "UPDATE document_collections SET sort_order = 0 WHERE id = '{}'",
                    library.f_chem
                )
            },
            "changes to subjects",
        ),
        (
            "a deck leaves its folder's subject unlisted",
            |library, _| {
                format!(
                    "UPDATE decks SET subject_id = NULL WHERE id = '{}'",
                    library.k_acids
                )
            },
            "changes to decks",
        ),
        (
            "an edited copy is merged",
            |library, _| {
                format!(
                    "INSERT INTO card_merges(retired_card_id, surviving_card_id, retired_note_id,
                         surviving_note_id, reason, copied_at)
                     VALUES ('{}', '{}', '{}', '{}', 'collection_copy', {})",
                    library.cb[2].card(),
                    library.o_bio[2].card(),
                    library.cb[2].id,
                    library.o_bio[2].id,
                    T0 + DAY
                )
            },
            "changes to merged collection copies",
        ),
        (
            "a survivor takes the schedule of a card outside its merge",
            |library, _| {
                format!(
                    "UPDATE card_schedules SET (state, due_at, stability, difficulty, reps) = (
                         SELECT state, due_at, stability, difficulty, reps
                         FROM card_schedules WHERE card_id = '{}')
                     WHERE card_id = '{}'",
                    library.o_dates.card(),
                    library.o_bio[1].card()
                )
            },
            "changes to card schedules",
        ),
        (
            "a source no retired copy invalidated is deactivated",
            |_, cascade| {
                format!(
                    "UPDATE knowledge_sources SET active = 0 WHERE id = '{}'",
                    cascade.document_source
                )
            },
            "changes to Tutor knowledge sources",
        ),
        (
            "a current Tutor turn is marked out of date",
            |_, cascade| {
                format!(
                    "UPDATE tutor_turns SET evidence_current = 0 WHERE id = '{}'",
                    cascade.third_turn
                )
            },
            "changed retained library values",
        ),
        (
            "a filing choice is dismissed for the learner",
            |_, _| "UPDATE subject_filing_reviews SET dismissed_at = 1".into(),
            "changes to the \"Check these\" list",
        ),
    ];
    for (story, sql, refusal) in cases {
        let fixture = Fixture::new();
        let (library, cascade) = subjects_library(&fixture);
        let sql = sql(&library, &cascade);
        let error = refused_upgrade(&fixture, 5, &sql);
        assert!(error.contains(refusal), "{story}: {error}");
    }
}

#[test]
fn proofs_read_both_libraries_without_writing_either() {
    let fixture = Fixture::new();
    subjects_library(&fixture);
    let original = fs::read(fixture.0.database_path()).unwrap();
    let prepared = prepare_upgrade_with(
        &fixture.0,
        APPLICATION,
        (CURRENT_SCHEMA_LINEAGE, 5),
        &mut |_| {},
        None,
        |path| {
            let database = Database::open(path)?;
            settle_candidate(&database)
        },
    )
    .unwrap();
    assert_eq!(fs::read(fixture.0.database_path()).unwrap(), original);
    let candidate = prepared.transaction.join("candidate/library.sqlite3");
    let migrated = fs::read(&candidate).unwrap();
    let source = read_database(fixture.0.database_path()).unwrap();
    let target = read_database(&candidate).unwrap();
    preservation::verify(&source, &target, CURRENT_SCHEMA_LINEAGE, 5).unwrap();
    drop((source, target));
    assert_eq!(fs::read(fixture.0.database_path()).unwrap(), original);
    assert_eq!(fs::read(&candidate).unwrap(), migrated);
}
