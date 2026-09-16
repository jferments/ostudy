use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

use super::tests::{definition, id, populate, rich};
use super::*;
use crate::{
    cards::questions::{LearnerAnswer, QuestionResponse},
    paths::AppPaths,
    practice::{presentation::PracticeContentDto, PracticeSessionDto, SubmitReviewInput},
    practice_selection::{start_card_practice_in_connection, RefreshPracticeAfterCardEditInput},
    projections::practice::load_practice_window,
    scheduler::load_schedule,
};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Cleanup(std::path::PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    database: Database,
    paths: AppPaths,
    _cleanup: Cleanup,
}

impl Fixture {
    fn new(body: Value) -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-practice-questions-{}-{}-{}",
            std::process::id(),
            unix_time_millis().unwrap(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let paths = AppPaths::new(root.clone()).unwrap();
        let database = Database::open(paths.database_path()).unwrap();
        let fixture = Self {
            database,
            paths,
            _cleanup: Cleanup(root),
        };
        fixture.run(|connection| {
            populate(
                connection,
                &AttemptIdentityInput {
                    session_id: id(4),
                    card_id: id(3),
                    instance_id: id(6),
                },
                false,
            )?;
            connection
                .execute_batch("DELETE FROM practice_sessions; DELETE FROM question_attempts;")
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE notes SET question_definition = ?1 WHERE id = ?2",
                    params![json!({"schemaVersion":1,"question":body}).to_string(), id(2)],
                )
                .map_err(CommandError::database)?;
            Ok(())
        });
        fixture
    }

    fn boolean() -> Self {
        Self::new(definition(false, false)["question"].clone())
    }

    fn run<T>(&self, operation: impl FnOnce(&mut Connection) -> Result<T, CommandError>) -> T {
        self.database.with_connection(operation).unwrap()
    }

    fn start(&self) -> PracticeSessionDto {
        self.run(|connection| {
            start_card_practice_in_connection(connection, &self.paths, id(90), vec![id(3)], false)
        })
    }

    fn view(&self, target: &AttemptIdentityInput) -> Value {
        self.run(|connection| {
            serde_json::to_value(read(connection, target)?).map_err(CommandError::serialization)
        })
    }

    fn counts(&self) -> (i64, i64, i64) {
        self.run(|connection| {
            connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM review_events),
                    (SELECT COUNT(*) FROM question_review_links),
                    (SELECT reps FROM card_schedules WHERE card_id = ?1)",
                    [id(3)],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)
        })
    }

    fn check(&self, target: &AttemptIdentityInput, response: QuestionResponse) -> AttemptView {
        self.run(|connection| {
            let revision = storage::load(connection, target)?.attempt.revision();
            check(
                connection,
                CheckPracticeAnswerInput {
                    target: target.clone(),
                    request: CheckAnswerRequest {
                        check_id: id(40),
                        expected_revision: revision,
                        answer: answer(target, response),
                        response_time_ms: 234,
                    },
                },
            )
        })
    }

    fn review(&self, target: &AttemptIdentityInput, outcome: &str) -> SubmitReviewInput {
        self.run(|connection| {
            Ok(SubmitReviewInput {
                session_id: target.session_id.clone(),
                card_id: target.card_id.clone(),
                submission_id: id(50),
                outcome: outcome.to_string(),
                response_time_ms: 234,
                schedule_revision: load_schedule(connection, &target.card_id)?.revision,
            })
        })
    }
}

fn target(session: &PracticeSessionDto) -> AttemptIdentityInput {
    let presentation = &session.first_window.cards[0].presentation;
    let PracticeContentDto::Interactive { question } = &presentation.content else {
        panic!("A typed question must not become a recall card");
    };
    AttemptIdentityInput {
        session_id: session.id.clone(),
        card_id: presentation.id.clone(),
        instance_id: question.attempt.instance_id.clone(),
    }
}

fn context(view: &AttemptView, reason: Option<&str>) -> QuestionReviewContext {
    QuestionReviewContext {
        instance_id: view.instance_id.clone(),
        attempt_revision: view.revision,
        override_reason: reason.map(str::to_string),
    }
}

fn answer(target: &AttemptIdentityInput, response: QuestionResponse) -> LearnerAnswer {
    LearnerAnswer {
        schema_version: 1,
        instance_id: target.instance_id.clone(),
        response,
    }
}

fn boolean(selected: bool) -> QuestionResponse {
    QuestionResponse::TrueFalse {
        selected: Some(selected),
    }
}

fn choice() -> Value {
    json!({"kind":"single_choice", "prompt":rich("Choose a letter."),
        "options":[{"id":id(11),"content":rich("Alpha"),"rationale":rich("Private alpha rationale")},
                   {"id":id(12),"content":rich("Beta"),"rationale":null}],
        "correctOptionId":id(12),"explanation":rich("Private overall explanation")})
}

#[test]
fn automatic_capture_has_no_fake_front_back_and_retries_keep_the_same_instance() {
    let fixture = Fixture::new(choice());
    let session = fixture.start();
    let initial = serde_json::to_value(&session).unwrap();
    let public = &initial["firstWindow"]["cards"][0]["presentation"];
    assert!(public.get("frontDoc").is_none());
    assert!(public.get("backDoc").is_none());
    assert!(public.get("backText").is_none());
    assert_eq!(public["frontText"], "Choose a letter.");
    assert!(!public.to_string().contains("correctOptionId"));
    assert!(!public.to_string().contains("rationale"));
    assert!(!public.to_string().contains("Private"));
    assert_eq!(initial, serde_json::to_value(fixture.start()).unwrap());
    let target = target(&session);
    let reloaded = fixture.run(|connection| {
        load_practice_window(connection, &fixture.paths, &session.id, 0, 50)
    });
    assert_eq!(initial["firstWindow"], serde_json::to_value(reloaded).unwrap());
    assert_eq!(fixture.view(&target)["attempt"]["revision"], 1);
}

#[test]
fn a_saved_selection_and_its_shuffle_survive_database_reopen() {
    let fixture = Fixture::new(choice());
    let session = fixture.start();
    let target = target(&session);
    fixture.run(|connection| {
        save(
            connection,
            SavePracticeAnswerInput {
                target: target.clone(),
                request: SaveAnswerRequest {
                    mutation_id: id(30),
                    expected_revision: 1,
                    answer: answer(
                        &target,
                        QuestionResponse::SingleChoice {
                            selected_option_id: Some(id(12)),
                        },
                    ),
                },
            },
        )
    });
    let before = fixture.view(&target);
    let reopened = Database::open(fixture.paths.database_path()).unwrap();
    let after = reopened
        .with_connection(|connection| {
            serde_json::to_value(read(connection, &target)?).map_err(CommandError::serialization)
        })
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        after["attempt"]["progress"]["savedAnswer"]["response"]["selectedOptionId"],
        id(12)
    );
    assert_eq!(fixture.counts(), (0, 0, 0));
}

#[test]
fn the_real_session_capture_rolls_back_every_card_if_a_later_question_is_invalid() {
    let fixture = Fixture::boolean();
    fixture.run(|connection| {
        let invalid = json!({"schemaVersion":1,"question":{"kind":"unknown"}}).to_string();
        connection
            .execute(
                "INSERT INTO notes (id, deck_id, note_type, content_format, question_definition,
                front_text, back_text, created_at, updated_at)
             VALUES (?1, ?2, 'interactive', 'question-json', ?3, 'Bad', '', 1, 1)",
                params![id(21), id(1), invalid],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO cards (id, note_id, deck_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 1, 1)",
                params![id(22), id(21), id(1)],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO card_question_presentations (card_id, note_id, question_key)
                 VALUES (?1, ?2, 'main')",
                params![id(22), id(21)],
            )
            .map_err(CommandError::database)?;
        crate::scheduler::insert_new_schedule(connection, &id(22), 1)?;
        let failed = start_card_practice_in_connection(
            connection,
            &fixture.paths,
            id(90),
            vec![id(3), id(22)],
            false,
        );
        assert!(failed.is_err());
        let counts: (i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM practice_sessions),
                    (SELECT COUNT(*) FROM question_attempts),
                    (SELECT COUNT(*) FROM practice_session_card_snapshots)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(CommandError::database)?;
        assert_eq!(counts, (0, 0, 0));
        Ok(())
    });
}

#[test]
fn source_edit_refresh_keeps_the_captured_key_response_and_labels() {
    let fixture = Fixture::boolean();
    let session = fixture.start();
    let target = target(&session);
    fixture.run(|connection| {
        save(
            connection,
            SavePracticeAnswerInput {
                target: target.clone(),
                request: SaveAnswerRequest {
                    mutation_id: id(30),
                    expected_revision: 1,
                    answer: answer(&target, boolean(false)),
                },
            },
        )?;
        connection
            .execute(
                "UPDATE notes SET question_definition = ?1, revision = 2,
                 front_text = 'New statement' WHERE id = ?2",
                params![definition(true, false).to_string(), id(2)],
            )
            .map_err(CommandError::database)?;
        let refreshed = crate::practice_selection::refresh_practice_after_card_edit_in_connection(
            connection,
            &fixture.paths,
            RefreshPracticeAfterCardEditInput {
                session_id: session.id.clone(),
                note_id: id(2),
                start_position: 0,
            },
        )?;
        assert_eq!(refreshed.window.cards[0].presentation.revision, 1);
        let view = storage::load(connection, &target)?.attempt.view()?;
        assert_eq!(view.revision, 2);
        Ok(())
    });
    let checked = fixture.check(&target, boolean(false));
    assert_eq!(
        serde_json::to_value(checked).unwrap()["progress"]["evaluation"]["verdict"],
        "correct"
    );
}

#[test]
fn check_then_review_records_one_fsrs_transition_and_an_immutable_link() {
    let fixture = Fixture::boolean();
    let session = fixture.start();
    let target = target(&session);
    let checked = fixture.check(&target, boolean(false));
    assert_eq!(fixture.counts(), (0, 0, 0));
    let input = fixture.review(&target, "very_confident");
    fixture.run(|connection| {
        let first = crate::practice::submit_review_with_question(
            connection,
            input.clone(),
            Some(context(&checked, None)),
        )?;
        let before = connection.total_changes();
        let replay = crate::practice::submit_review_with_question(
            connection,
            input.clone(),
            Some(context(&checked, None)),
        )?;
        assert_eq!(
            serde_json::to_value(first).unwrap(),
            serde_json::to_value(replay).unwrap()
        );
        assert_eq!(connection.total_changes(), before);
        let mut duplicate = input;
        duplicate.submission_id = id(51);
        let result = crate::practice::submit_review_with_question(
            connection,
            duplicate,
            Some(context(&checked, None)),
        );
        assert!(result.is_err());
        let correct: (i64, i64) = connection
            .query_row(
                "SELECT r.answer_correct, ps.correct_count FROM review_events r
             JOIN practice_sessions ps ON ps.id = r.session_id WHERE r.card_id = ?1",
                [&target.card_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(CommandError::database)?;
        assert_eq!(correct, (1, 1));
        Ok(())
    });
    assert_eq!(fixture.counts(), (1, 1, 1));
}

#[test]
fn ordinary_reveal_and_review_cannot_bypass_objective_checking() {
    let fixture = Fixture::boolean();
    let session = fixture.start();
    let target = target(&session);
    fixture.run(|connection| {
        let error = crate::practice::record_practice_event(
            connection,
            &target.session_id,
            &target.card_id,
            true,
        )
        .unwrap_err();
        assert_eq!(error.code, "question_not_checked");
        Ok(())
    });
    fixture.check(&target, boolean(false));
    let input = fixture.review(&target, "very_confident");
    fixture.run(|connection| {
        assert_eq!(
            crate::practice::submit_review_in_connection(connection, input)
                .unwrap_err()
                .code,
            "question_review_required"
        );
        Ok(())
    });
    assert_eq!(fixture.counts(), (0, 0, 0));
}

#[test]
fn an_explicit_scheduling_override_does_not_rewrite_objective_correctness() {
    let fixture = Fixture::boolean();
    let session = fixture.start();
    let target = target(&session);
    let checked = fixture.check(&target, boolean(true));
    let input = fixture.review(&target, "somewhat_confident");
    fixture.run(|connection| {
        let error = crate::practice::submit_review_with_question(
            connection,
            input.clone(),
            Some(context(&checked, None)),
        )
        .unwrap_err();
        assert_eq!(error.code, "question_rating_override_required");
        let result = crate::practice::submit_review_with_question(
            connection,
            input,
            Some(context(&checked, Some("The authored key needs correction."))),
        )?;
        assert_eq!(result.correct_count, 0);
        let evidence: (i64, String, String) = connection
            .query_row(
                "SELECT r.answer_correct, q.override_reason, a.verdict
             FROM review_events r JOIN question_review_links q ON q.review_event_id = r.id
             JOIN question_attempts a ON a.instance_id = q.instance_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(CommandError::database)?;
        assert_eq!(
            evidence,
            (
                0,
                "The authored key needs correction.".to_string(),
                "incorrect".to_string()
            )
        );
        Ok(())
    });
    assert_eq!(fixture.counts(), (1, 1, 1));
}

#[test]
fn stale_schedule_or_attempt_and_changed_elapsed_time_leave_the_check_unrated() {
    let fixture = Fixture::boolean();
    let session = fixture.start();
    let target = target(&session);
    let checked = fixture.check(&target, boolean(false));
    let original = fixture.review(&target, "very_confident");
    fixture.run(|connection| {
        let mut stale = context(&checked, None);
        stale.attempt_revision -= 1;
        let error = crate::practice::submit_review_with_question(
            connection,
            original.clone(),
            Some(stale),
        )
        .unwrap_err();
        assert_eq!(error.code, "question_attempt_conflict");
        let mut changed_time = original.clone();
        changed_time.response_time_ms += 1;
        let error = crate::practice::submit_review_with_question(
            connection,
            changed_time,
            Some(context(&checked, None)),
        )
        .unwrap_err();
        assert_eq!(error.code, "question_response_time_changed");
        connection
            .execute("UPDATE card_schedules SET revision = revision + 1", [])
            .map_err(CommandError::database)?;
        let error = crate::practice::submit_review_with_question(
            connection,
            original,
            Some(context(&checked, None)),
        )
        .unwrap_err();
        assert_eq!(error.code, "schedule_changed");
        Ok(())
    });
    assert_eq!(fixture.counts(), (0, 0, 0));
    assert_eq!(
        fixture.view(&target)["attempt"]["progress"]["phase"],
        "checked"
    );
}

#[test]
fn link_failure_rolls_back_review_schedule_and_counters_but_keeps_the_first_check() {
    let fixture = Fixture::boolean();
    let session = fixture.start();
    let target = target(&session);
    let checked = fixture.check(&target, boolean(false));
    let input = fixture.review(&target, "very_confident");
    fixture.run(|connection| {
        connection
            .execute_batch(
                "CREATE TRIGGER reject_question_link BEFORE INSERT ON question_review_links
             BEGIN SELECT RAISE(ABORT, 'injected link failure'); END;",
            )
            .map_err(CommandError::database)?;
        let failed = crate::practice::submit_review_with_question(
            connection,
            input.clone(),
            Some(context(&checked, None)),
        );
        assert!(failed.is_err());
        let rated: i64 = connection
            .query_row("SELECT rated_count FROM practice_sessions", [], |row| {
                row.get(0)
            })
            .map_err(CommandError::database)?;
        assert_eq!(rated, 0);
        Ok(())
    });
    assert_eq!(fixture.counts(), (0, 0, 0));
    assert_eq!(
        fixture.view(&target)["attempt"]["progress"]["phase"],
        "checked"
    );
    fixture.run(|connection| {
        connection
            .execute_batch("DROP TRIGGER reject_question_link;")
            .map_err(CommandError::database)?;
        crate::practice::submit_review_with_question(
            connection,
            input,
            Some(context(&checked, None)),
        )?;
        Ok(())
    });
    assert_eq!(fixture.counts(), (1, 1, 1));
}

#[test]
fn remediation_records_separate_evidence_without_erasing_the_first_mistake_or_scheduling() {
    let fixture = Fixture::boolean();
    let session = fixture.start();
    let target = target(&session);
    let make_input = || remediation::RemediationInput {
        target: target.clone(),
        operation_id: id(70),
        answer: answer(&target, boolean(false)),
    };
    fixture.run(|connection| {
        assert!(remediation::remediate(connection, make_input()).is_err());
        Ok(())
    });
    let checked = fixture.check(&target, boolean(true));
    let first = fixture.view(&target);
    fixture.run(|connection| {
        let result = remediation::remediate(connection, make_input())?;
        assert_eq!(result.evaluation.verdict, Verdict::Correct);
        let before = connection.total_changes();
        let replay = remediation::remediate(connection, make_input())?;
        assert_eq!(connection.total_changes(), before);
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            serde_json::to_value(replay).unwrap()
        );
        let mut different = make_input();
        different.answer.response = boolean(true);
        assert_eq!(
            remediation::remediate(connection, different)
                .err()
                .unwrap()
                .code,
            "question_operation_conflict"
        );
        Ok(())
    });
    assert_eq!(fixture.view(&target), first);
    assert_eq!(fixture.counts(), (0, 0, 0));
    let input = fixture.review(&target, "wrong");
    fixture.run(|connection| {
        let result = crate::practice::submit_review_with_question(
            connection,
            input,
            Some(context(&checked, None)),
        )?;
        assert_eq!(result.correct_count, 0);
        remediation::remediate(connection, make_input())?;
        Ok(())
    });
    assert_eq!(fixture.counts(), (1, 1, 1));
}

#[test]
fn empty_select_all_is_an_explicit_correct_submission() {
    let mut body = choice();
    body["kind"] = json!("select_all");
    body.as_object_mut().unwrap().remove("correctOptionId");
    body["correctOptionIds"] = json!([]);
    let fixture = Fixture::new(body);
    let session = fixture.start();
    let target = target(&session);
    assert!(fixture.view(&target)["attempt"]["progress"]["savedAnswer"].is_null());
    let checked = fixture.check(
        &target,
        QuestionResponse::SelectAll {
            selected_option_ids: vec![],
        },
    );
    assert_eq!(
        serde_json::to_value(&checked).unwrap()["progress"]["evaluation"]["verdict"],
        "correct"
    );
    let input = fixture.review(&target, "very_confident");
    fixture.run(|connection| {
        crate::practice::submit_review_with_question(
            connection,
            input,
            Some(context(&checked, None)),
        )?;
        Ok(())
    });
    assert_eq!(fixture.counts(), (1, 1, 1));
}

#[test]
fn ordering_capture_does_not_expose_the_authored_order_as_search_metadata() {
    let body = json!({"kind":"ordering", "prompt":rich("Arrange from first to last."),
        "items":[{"id":id(11),"content":rich("First step")},
                 {"id":id(12),"content":rich("Second step")},
                 {"id":id(13),"content":rich("Third step")}],"explanation":null});
    let fixture = Fixture::new(body);
    let session = fixture.start();
    let public = serde_json::to_value(&session.first_window.cards[0].presentation).unwrap();
    assert_eq!(public["frontText"], "Arrange from first to last.");
    let ids: Vec<_> = public["question"]["attempt"]["question"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect();
    assert_ne!(ids, vec![id(11), id(12), id(13)]);
    let target = target(&session);
    fixture.run(|connection| {
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM card_schedules", [], |row| row.get(0))
            .map_err(CommandError::database)?;
        assert_eq!(count, 1);
        let public_again = read(connection, &target)?;
        assert_eq!(
            serde_json::to_value(public_again).unwrap(),
            public["question"]
        );
        Ok(())
    });
}
