//! Native reveal permission and assistance publication for conversational hints.
//! Reading/opening a Tutor does not change evidence or schedule a review.

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct HintTarget {
    pub target: AttemptIdentityInput,
    pub note_revision: i64,
}

pub(crate) struct FocusProjection {
    pub target: HintTarget,
    pub prompt: String,
    pub answer: Option<String>,
    pub hidden: bool,
}

pub(crate) fn focus(
    connection: &Connection,
    session_id: &str,
    card_id: &str,
    note_revision: i64,
) -> Result<Option<FocusProjection>, CommandError> {
    let instance: Option<String> = connection
        .query_row(
            "SELECT question_instance_id FROM practice_session_card_snapshots
             WHERE session_id = ?1 AND card_id = ?2 AND note_revision = ?3",
            params![session_id, card_id, note_revision],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let Some(instance_id) = instance else {
        return Ok(None);
    };
    let target = HintTarget {
        target: AttemptIdentityInput {
            session_id: session_id.to_string(),
            card_id: card_id.to_string(),
            instance_id,
        },
        note_revision,
    };
    let stored = storage::load(connection, &target.target)?;
    stored.require_available()?;
    if stored.attempt.instance().identity().note_revision != note_revision {
        return Err(conflict());
    }
    let view = stored.attempt.view()?;
    let hidden = matches!(&view.progress, AttemptProgress::Answering { .. });
    let answer = if hidden {
        None
    } else {
        Some(stored.attempt.instance().knowledge_text()?.answer)
    };
    Ok(Some(FocusProjection {
        target,
        prompt: view.question.knowledge_prompt_text()?,
        answer,
        hidden,
    }))
}

/// Join the successful Tutor-publication transaction. A stale, checked, removed
/// or replaced attempt cannot receive a late hint. Retried delivery is idempotent.
pub(crate) fn expose_hint(
    connection: &Connection,
    target: &HintTarget,
    now: i64,
) -> Result<(), CommandError> {
    validate_target(&target.target)?;
    let stored = storage::load(connection, &target.target)?;
    stored.require_mutable()?;
    let current: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM notes n JOIN cards c ON c.note_id = n.id
             WHERE c.id = ?1 AND n.revision = ?2 AND n.deleted_at IS NULL)",
            params![target.target.card_id, target.note_revision],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if !current || stored.attempt.instance().identity().note_revision != target.note_revision {
        return Err(conflict());
    }
    let mut attempt = stored.attempt.clone();
    attempt.mark_assisted()?;
    if attempt.revision() != stored.attempt.revision() {
        storage::persist(connection, &stored, &attempt, now)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cards::questions::{LearnerAnswer, QuestionResponse};

    use super::*;

    fn fixture() -> (Database, HintTarget) {
        let database = Database::open_in_memory().unwrap();
        let target = HintTarget {
            target: AttemptIdentityInput {
                session_id: format!("{:032x}", 4),
                card_id: format!("{:032x}", 3),
                instance_id: format!("{:032x}", 6),
            },
            note_revision: 1,
        };
        database
            .with_connection(|connection| {
                super::super::tests::populate(connection, &target.target, false)
            })
            .unwrap();
        (database, target)
    }

    fn assisted(connection: &Connection, target: &HintTarget) -> bool {
        storage::load(connection, &target.target)
            .unwrap()
            .attempt
            .view()
            .unwrap()
            .assisted
    }

    #[test]
    fn hidden_projection_excludes_private_answer_and_reading_does_not_mark_assistance() {
        let (database, target) = fixture();
        database
            .with_connection(|connection| {
                let projection = focus(
                    connection,
                    &target.target.session_id,
                    &target.target.card_id,
                    target.note_revision,
                )?
                .unwrap();
                assert!(projection.hidden);
                assert!(projection.answer.is_none());
                assert!(!assisted(connection, &target));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn exposed_hint_is_atomic_idempotent_and_never_creates_a_review() {
        let (database, target) = fixture();
        database
            .with_connection(|connection| {
                {
                    let transaction = connection.transaction().map_err(CommandError::database)?;
                    expose_hint(&transaction, &target, 10)?;
                    // Dropping a failed publication rolls assistance back with it.
                }
                assert!(!assisted(connection, &target));
                let transaction = connection.transaction().map_err(CommandError::database)?;
                expose_hint(&transaction, &target, 11)?;
                expose_hint(&transaction, &target, 12)?;
                let attempt = storage::load(&transaction, &target.target)?.attempt;
                assert!(attempt.view()?.assisted);
                assert_eq!(attempt.revision(), 2);
                let reviews: i64 = transaction
                    .query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(reviews, 0);
                transaction.commit().map_err(CommandError::database)
            })
            .unwrap();
    }

    #[test]
    fn late_hint_cannot_change_an_immutable_first_check() {
        let (database, target) = fixture();
        database
            .with_connection(|connection| {
                check(
                    connection,
                    CheckPracticeAnswerInput {
                        target: target.target.clone(),
                        request: CheckAnswerRequest {
                            check_id: format!("{:032x}", 10),
                            expected_revision: 1,
                            answer: LearnerAnswer {
                                schema_version: 1,
                                instance_id: target.target.instance_id.clone(),
                                response: QuestionResponse::TrueFalse {
                                    selected: Some(false),
                                },
                            },
                            response_time_ms: 50,
                        },
                    },
                )?;
                let before = storage::load(connection, &target.target)?.attempt.to_json()?;
                {
                    let transaction = connection.transaction().map_err(CommandError::database)?;
                    assert!(expose_hint(&transaction, &target, 100).is_err());
                }
                let after = storage::load(connection, &target.target)?.attempt.to_json()?;
                assert_eq!(before, after);
                let projection = focus(
                    connection,
                    &target.target.session_id,
                    &target.target.card_id,
                    target.note_revision,
                )?
                .unwrap();
                assert!(!projection.hidden);
                assert!(projection.answer.unwrap().contains("Authored verdict: False"));
                Ok(())
            })
            .unwrap();
    }
}
