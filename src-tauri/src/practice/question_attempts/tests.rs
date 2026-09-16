use serde_json::{json, Value};

use super::*;
use crate::{
    cards::questions::{
        DisplayOrder, InstanceIdentity, LearnerAnswer, QuestionInstance, QuestionResponse,
        ValidatedQuestion,
    },
    scheduler::insert_new_schedule,
};

struct Fixture {
    database: Database,
    target: AttemptIdentityInput,
}

impl Fixture {
    fn new(with_explanation_image: bool) -> Self {
        let database = Database::open_in_memory().unwrap();
        let target = AttemptIdentityInput {
            session_id: id(4),
            card_id: id(3),
            instance_id: id(6),
        };
        database
            .with_connection(|connection| populate(connection, &target, with_explanation_image))
            .unwrap();
        Self { database, target }
    }

    fn answer(&self, selected: Option<bool>) -> LearnerAnswer {
        LearnerAnswer {
            schema_version: 1,
            instance_id: self.target.instance_id.clone(),
            response: QuestionResponse::TrueFalse { selected },
        }
    }

    fn check_input(&self, selected: Option<bool>) -> CheckPracticeAnswerInput {
        CheckPracticeAnswerInput {
            target: self.target.clone(),
            request: CheckAnswerRequest {
                check_id: id(10),
                expected_revision: 1,
                answer: self.answer(selected),
                response_time_ms: 234,
            },
        }
    }

    fn check(&self, selected: Option<bool>) -> Result<AttemptView, CommandError> {
        self.database
            .with_connection(|connection| check(connection, self.check_input(selected)))
    }

    fn view(&self) -> Value {
        self.database
            .with_connection(|connection| {
                let attempt = storage::load(connection, &self.target)?.attempt;
                serde_json::to_value(attempt.view()?).map_err(CommandError::serialization)
            })
            .unwrap()
    }
}

fn populate(
    connection: &Connection,
    target: &AttemptIdentityInput,
    with_explanation_image: bool,
) -> Result<(), CommandError> {
    let definition = definition(false, with_explanation_image);
    let question = ValidatedQuestion::parse(&definition.to_string())?;
    let instance = QuestionInstance::capture(
        InstanceIdentity {
            instance_id: target.instance_id.clone(),
            note_id: id(2),
            card_id: target.card_id.clone(),
            note_revision: 1,
        },
        question,
        "main".to_string(),
        DisplayOrder::default(),
    )?;
    let attempt = QuestionAttempt::start(instance)?;
    connection
        .execute(
            "INSERT INTO decks (id, name, created_at, updated_at) VALUES (?1, 'Test', 1, 1)",
            [id(1)],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO notes (id, deck_id, note_type, content_format,
                question_definition, front_text, back_text, created_at, updated_at)
             VALUES (?1, ?2, 'interactive', 'question-json', ?3, 'Statement', '', 1, 1)",
            params![id(2), id(1), definition.to_string()],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO cards (id, note_id, deck_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, 1)",
            params![target.card_id, id(2), id(1)],
        )
        .map_err(CommandError::database)?;
    insert_new_schedule(connection, &target.card_id, 1)?;
    connection
        .execute(
            "INSERT INTO card_question_presentations (card_id, note_id, question_key)
             VALUES (?1, ?2, 'main')",
            params![target.card_id, id(2)],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO practice_sessions (id, selected_deck_ids_json,
                selected_tag_ids_json, shuffle, card_count, mode, started_at)
             VALUES (?1, ?2, '[]', 0, 1, 'custom', 1)",
            params![target.session_id, json!([id(1)]).to_string()],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO practice_session_cards (session_id, card_id, position, presentation_variant)
             VALUES (?1, ?2, 0, '')",
            params![target.session_id, target.card_id],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO question_attempts (instance_id, session_id, card_id, note_id,
                question_kind, instance_hash, revision, phase, assisted, checkpoint_json,
                created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'true_false', ?5, 1, 'answering', 0, ?6, 1, 1)",
            params![
                target.instance_id,
                target.session_id,
                target.card_id,
                id(2),
                attempt.instance().fingerprint(),
                attempt.to_json()?
            ],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO practice_session_card_snapshots (session_id, card_id, note_id,
                note_revision, owner_deck_id_at_capture, captured_deck_name, direction,
                card_type, card_label, presentation_variant, front_text, back_text,
                question_excerpt, question_text_hash, question_was_truncated,
                captured_at, question_instance_id)
             VALUES (?1, ?2, ?3, 1, ?4, 'Test', 'forward', 'true_false', 'True / false',
                '', 'Statement', '', 'Statement', ?5, 0, 1, ?6)",
            params![
                target.session_id,
                target.card_id,
                id(2),
                id(1),
                "a".repeat(64),
                target.instance_id
            ],
        )
        .map_err(CommandError::database)?;
    if with_explanation_image {
        connection
            .execute(
                "INSERT INTO assets (id, original_filename, mime_type, byte_size, relative_path, created_at)
                 VALUES (?1, 'explanation.png', 'image/png', 1, 'assets/images/explanation.png', 1)",
                [id(8)],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO question_attempt_assets (instance_id, asset_id) VALUES (?1, ?2)",
                params![target.instance_id, id(8)],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn id(value: u32) -> String {
    format!("{value:032x}")
}

fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn definition(correct: bool, image: bool) -> Value {
    let explanation = if image {
        json!({"type":"doc","content":[{"type":"image","attrs":{
            "assetId":id(8),"src":format!("study-asset://{}",id(8))}}]})
    } else {
        rich("Explanation appears only after checking.")
    };
    json!({"schemaVersion":1,"question":{"kind":"true_false","statement":rich("Statement"),
        "correct":correct,"explanation":explanation,"correctedStatement":null}})
}

#[test]
fn saved_answer_survives_rehydration_and_stale_autosaves_do_not_overwrite_it() {
    let fixture = Fixture::new(false);
    let request = SavePracticeAnswerInput {
        target: fixture.target.clone(),
        request: SaveAnswerRequest {
            mutation_id: id(9),
            expected_revision: 1,
            answer: fixture.answer(Some(false)),
        },
    };
    fixture
        .database
        .with_connection(|connection| save(connection, request))
        .unwrap();
    let restored = fixture.view();
    assert_eq!(restored["revision"], 2);
    assert_eq!(restored["progress"]["savedAnswer"]["response"]["selected"], false);
    let stale = SavePracticeAnswerInput {
        target: fixture.target.clone(),
        request: SaveAnswerRequest {
            mutation_id: id(11),
            expected_revision: 1,
            answer: fixture.answer(Some(true)),
        },
    };
    let error = fixture
        .database
        .with_connection(|connection| save(connection, stale))
        .err()
        .unwrap();
    assert_eq!(error.code, "question_attempt_conflict");
    assert_eq!(fixture.view(), restored);
}

#[test]
fn checked_answer_and_session_reveal_are_one_transaction_without_a_review() {
    let fixture = Fixture::new(false);
    fixture.check(Some(false)).unwrap();
    assert_eq!(fixture.view()["progress"]["evaluation"]["verdict"], "correct");
    fixture
        .database
        .with_connection(|connection| {
            let counts: (i64, i64, i64) = connection
                .query_row(
                    "SELECT viewed_count, revealed_count, rated_count FROM practice_sessions WHERE id = ?1",
                    [&fixture.target.session_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(counts, (1, 1, 0));
            let reviews: i64 = connection
                .query_row("SELECT count(*) FROM review_events", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(reviews, 0);
            let reps: i64 = connection
                .query_row(
                    "SELECT reps FROM card_schedules WHERE card_id = ?1",
                    [&fixture.target.card_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(reps, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_counter_failure_rolls_back_the_check_and_reveal_together() {
    let fixture = Fixture::new(false);
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute_batch(
                    "CREATE TRIGGER reject_counter BEFORE UPDATE ON practice_sessions
                     BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    let before = fixture.view();
    assert!(fixture.check(Some(false)).is_err());
    assert_eq!(fixture.view(), before);
    fixture
        .database
        .with_connection(|connection| {
            let revealed: Option<i64> = connection
                .query_row("SELECT revealed_at FROM practice_session_cards", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)?;
            assert!(revealed.is_none());
            Ok(())
        })
        .unwrap();
}

#[test]
fn an_identical_check_replays_after_session_completion_without_new_writes() {
    let fixture = Fixture::new(false);
    let first = fixture.check(Some(false)).unwrap();
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute("UPDATE practice_sessions SET completed_at = 5", [])
                .map_err(CommandError::database)?;
            let before = connection.total_changes();
            let replay = check(connection, fixture.check_input(Some(false)))?;
            assert_eq!(
                serde_json::to_value(replay).unwrap(),
                serde_json::to_value(&first).unwrap()
            );
            assert_eq!(connection.total_changes(), before);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        fixture.check(Some(true)).err().unwrap().code,
        "question_operation_conflict"
    );
}

#[test]
fn changed_source_does_not_change_the_frozen_answer_key() {
    let fixture = Fixture::new(false);
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE notes SET question_definition = ?1, revision = 2 WHERE id = ?2",
                    params![definition(true, false).to_string(), id(2)],
                )
                .map_err(CommandError::database)?;
            check(connection, fixture.check_input(Some(false)))?;
            Ok(())
        })
        .unwrap();
    assert_eq!(fixture.view()["progress"]["evaluation"]["verdict"], "correct");
}

#[test]
fn foreign_session_and_instance_identifiers_cannot_mutate_another_attempt() {
    let fixture = Fixture::new(false);
    let before = fixture.view();
    let mut request = fixture.check_input(Some(false));
    request.target.session_id = id(99);
    assert!(fixture
        .database
        .with_connection(|connection| check(connection, request))
        .is_err());
    let mut request = fixture.check_input(Some(false));
    request.request.answer.instance_id = id(99);
    let error = fixture
        .database
        .with_connection(|connection| check(connection, request))
        .err()
        .unwrap();
    assert_eq!(error.code, "question_instance_mismatch");
    assert_eq!(fixture.view(), before);
}

#[test]
fn corrupted_fingerprint_is_rejected_before_answer_mutation() {
    let fixture = Fixture::new(false);
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE question_attempts SET instance_hash = ?1",
                    ["f".repeat(64)],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        fixture.check(Some(false)).err().unwrap().code,
        "invalid_question_checkpoint"
    );
}

#[test]
fn hidden_feedback_assets_and_missing_media_do_not_undo_a_check() {
    let fixture = Fixture::new(true);
    fixture
        .database
        .with_connection(|connection| {
            assert!(read(connection, &fixture.target)?.assets.is_empty());
            let first = check(connection, fixture.check_input(Some(false)))?;
            assert_eq!(read(connection, &fixture.target)?.assets.len(), 1);
            connection
                .execute(
                    "UPDATE assets SET lifecycle_state = 'missing' WHERE id = ?1",
                    [id(8)],
                )
                .map_err(CommandError::database)?;
            let replay = check(connection, fixture.check_input(Some(false)))?;
            assert_eq!(
                serde_json::to_value(first).unwrap(),
                serde_json::to_value(replay).unwrap()
            );
            assert_eq!(
                read(connection, &fixture.target).err().unwrap().code,
                "question_asset_unavailable"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn an_unchecked_completed_session_cannot_gain_a_new_first_answer() {
    let fixture = Fixture::new(false);
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute("UPDATE practice_sessions SET completed_at = 2", [])
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    let before = fixture.view();
    assert_eq!(
        fixture.check(Some(false)).err().unwrap().code,
        "practice_question_unavailable"
    );
    assert_eq!(fixture.view(), before);
}
