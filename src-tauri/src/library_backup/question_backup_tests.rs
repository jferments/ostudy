use std::fs;

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::*;
use crate::{
    cards::questions::{
        ChoiceOption, DisplayOrder, InstanceIdentity, QuestionAttempt, QuestionBody,
        QuestionDefinition, QuestionInstance, ValidatedQuestion, QUESTION_SCHEMA_VERSION,
    },
    scheduler::insert_new_schedule,
};

fn id(value: u32) -> String {
    format!("{value:032x}")
}

fn rich(text: &str) -> Value {
    json!({
        "type":"doc",
        "content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]
    })
}

#[test]
fn complete_backup_round_trip_preserves_typed_question_definition_attempt_and_generation_evidence()
{
    let paths = AppPaths::new(std::env::temp_dir().join(format!(
        "ostudy-backup-typed-question-{}-{}",
        std::process::id(),
        unix_time_millis().expect("clock")
    )))
    .expect("paths should initialize");
    let database = Database::open(paths.database_path()).expect("database should initialize");

    let deck_id = id(1);
    let note_id = id(2);
    let card_id = id(3);
    let instance_id = id(4);
    let option_a = id(10);
    let option_b = id(11);
    let display_order = DisplayOrder {
        item_ids: vec![option_a.clone(), option_b.clone()],
        target_ids: Vec::new(),
    };
    let definition = QuestionDefinition {
        schema_version: QUESTION_SCHEMA_VERSION,
        question: QuestionBody::SingleChoice {
            prompt: rich("Which chamber pumps blood into the systemic circulation?"),
            options: vec![
                ChoiceOption {
                    id: option_a.clone(),
                    content: rich("Left ventricle"),
                    rationale: Some(rich("The left ventricle ejects into the aorta.")),
                },
                ChoiceOption {
                    id: option_b,
                    content: rich("Right atrium"),
                    rationale: Some(rich("The right atrium receives systemic venous return.")),
                },
            ],
            correct_option_id: option_a,
            explanation: Some(rich("Systemic ejection begins from the left ventricle.")),
        },
    };
    let question = ValidatedQuestion::validate(definition).expect("question should validate");
    let definition_json = question.to_json().expect("question should serialize");
    let search = question
        .search_text()
        .expect("question should project search text");
    let instance = QuestionInstance::capture(
        InstanceIdentity {
            instance_id: instance_id.clone(),
            note_id: note_id.clone(),
            card_id: card_id.clone(),
            note_revision: 1,
        },
        question,
        "main".to_string(),
        display_order,
    )
    .expect("instance should capture");
    let attempt = QuestionAttempt::start(instance).expect("attempt should start");
    let checkpoint = attempt.to_json().expect("attempt should serialize");
    let instance_hash = attempt.instance().fingerprint().to_string();

    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO decks (id, name, created_at, updated_at)
                     VALUES (?1, 'Typed backup', 10, 10)",
                    [&deck_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         question_definition, front_text, back_text, duplicate_hash,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'interactive', 'question-json', 1,
                         ?3, ?4, ?5, ?6, 10, 10, 1)",
                    params![
                        note_id,
                        deck_id,
                        definition_json,
                        search.prompt,
                        search.answer,
                        "a".repeat(64),
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 'forward', 0, 10, 10)",
                    params![card_id, note_id, deck_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO card_question_presentations (card_id, note_id, question_key)
                     VALUES (?1, ?2, 'main')",
                    params![card_id, note_id],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(connection, &card_id, 10)?;
            connection
                .execute(
                    "INSERT INTO question_attempts (
                         instance_id, card_id, note_id, question_kind, instance_hash,
                         revision, phase, assisted, checkpoint_json, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 'single_choice', ?4, 1,
                         'answering', 0, ?5, 20, 20)",
                    params![instance_id, card_id, note_id, instance_hash, checkpoint],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO question_generation_evidence (
                         note_id, note_revision, document_id, question_hash,
                         opportunity_id, policy_hash, record_json, created_at
                     ) VALUES (?1, 1, NULL, ?2, ?3, ?4, ?5, 10)",
                    params![
                        note_id,
                        "b".repeat(64),
                        "c".repeat(64),
                        "d".repeat(64),
                        json!({"schemaVersion":1,"components":[]}).to_string(),
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .expect("typed fixture should save");

    let destination = paths.app_data_dir().join("typed.ostudybackup");
    export_backup(&database, &paths, &destination).expect("backup should export");
    validate_backup_package(&destination, None).expect("backup should validate");

    let restore_root = paths.app_data_dir().join("restore-typed");
    let restored_database = restore_root.join("library.sqlite3");
    let restored_assets = restore_root.join("assets");
    extract_backup_package(&destination, &restored_database, &restored_assets)
        .expect("backup should extract");

    let restored = Connection::open(&restored_database).expect("restored database should open");
    let restored_definition: String = restored
        .query_row(
            "SELECT question_definition FROM notes WHERE id = ?1",
            [&note_id],
            |row| row.get(0),
        )
        .expect("typed note should restore");
    let restored_question = ValidatedQuestion::parse(&restored_definition)
        .expect("restored definition should validate");
    assert_eq!(
        restored_question.presentation_keys(),
        vec!["main".to_string()]
    );
    let restored_checkpoint: String = restored
        .query_row(
            "SELECT checkpoint_json FROM question_attempts WHERE instance_id = ?1",
            [&instance_id],
            |row| row.get(0),
        )
        .expect("question attempt should restore");
    let restored_attempt =
        QuestionAttempt::restore(&restored_checkpoint).expect("restored attempt should validate");
    assert_eq!(restored_attempt.instance().identity().note_id, note_id);
    let evidence_count: i64 = restored
        .query_row(
            "SELECT COUNT(*) FROM question_generation_evidence WHERE note_id = ?1",
            [&note_id],
            |row| row.get(0),
        )
        .expect("generation evidence should restore");
    assert_eq!(evidence_count, 1);
    let presentation_count: i64 = restored
        .query_row(
            "SELECT COUNT(*) FROM card_question_presentations WHERE card_id = ?1",
            [&card_id],
            |row| row.get(0),
        )
        .expect("presentation should restore");
    assert_eq!(presentation_count, 1);

    let _ = fs::remove_dir_all(paths.app_data_dir());
}
