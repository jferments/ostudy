//! The single authoritative FSRS write remains in practice.rs. This adapter
//! admits the checked evidence and records its link inside that same transaction.

use super::*;
use crate::practice::{ReviewResultDto, SubmitReviewInput};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuestionReviewContext {
    pub instance_id: String,
    pub attempt_revision: i64,
    pub override_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubmitQuestionReviewInput {
    pub review: SubmitReviewInput,
    pub question: QuestionReviewContext,
}

pub(crate) struct QuestionReviewEvidence {
    pub correct: bool,
    instance_id: String,
    override_reason: Option<String>,
}

impl QuestionReviewEvidence {
    pub fn link(&self, connection: &Connection, event_id: &str) -> Result<(), CommandError> {
        connection
            .execute(
                "INSERT INTO question_review_links (review_event_id, instance_id, override_reason)
             VALUES (?1, ?2, ?3)",
                params![event_id, self.instance_id, self.override_reason],
            )
            .map_err(CommandError::database)?;
        Ok(())
    }
}

#[tauri::command]
pub fn submit_practice_question_review(
    database: State<'_, Database>,
    input: SubmitQuestionReviewInput,
) -> Result<ReviewResultDto, CommandError> {
    database.with_connection(|connection| {
        crate::practice::submit_review_with_question(connection, input.review, Some(input.question))
    })
}

pub(crate) fn authorize_review(
    connection: &Connection,
    input: &SubmitReviewInput,
    context: Option<&QuestionReviewContext>,
) -> Result<Option<QuestionReviewEvidence>, CommandError> {
    let target = target_for_card(connection, &input.session_id, &input.card_id)?;
    let Some(target) = target else {
        if context.is_some() {
            return Err(invalid_storage());
        }
        return Ok(None);
    };
    let context = context.ok_or_else(|| {
        CommandError::new(
            "question_review_required",
            "Interactive questions require their checked attempt when recording a review.",
        )
    })?;
    validate_entity_id(&context.instance_id, "question instance")?;
    if target.instance_id != context.instance_id || context.attempt_revision < 1 {
        return Err(conflict());
    }
    let stored = storage::load(connection, &target)?;
    stored.require_mutable()?;
    if stored.attempt.revision() != context.attempt_revision {
        return Err(conflict());
    }
    let view = stored.attempt.view()?;
    let AttemptProgress::Checked {
        evaluation,
        response_time_ms,
        ..
    } = view.progress
    else {
        return Err(not_checked());
    };
    if input.response_time_ms != response_time_ms {
        return Err(CommandError::new(
            "question_response_time_changed",
            "Use the response time captured by the first answer check.",
        ));
    }
    let reason = context.override_reason.as_deref().map(str::trim);
    if reason.is_some_and(|value| value.is_empty() || value.chars().count() > 2000) {
        return Err(CommandError::new(
            "invalid_question_override",
            "Provide an override explanation between 1 and 2000 characters.",
        ));
    }
    let correct = evaluation.verdict == Verdict::Correct;
    let rating_correct = crate::scheduler::ReviewOutcome::parse(&input.outcome)?.answer_correct();
    if correct != rating_correct && reason.is_none() {
        return Err(CommandError::new(
            "question_rating_override_required",
            "Changing the scheduling result of a checked answer requires an explicit explanation.",
        ));
    }
    Ok(Some(QuestionReviewEvidence {
        correct,
        instance_id: context.instance_id.clone(),
        override_reason: reason.map(str::to_string),
    }))
}

pub(crate) fn require_checked_for_reveal(
    connection: &Connection,
    session_id: &str,
    card_id: &str,
) -> Result<(), CommandError> {
    if let Some(target) = target_for_card(connection, session_id, card_id)? {
        let stored = storage::load(connection, &target)?;
        if !matches!(
            stored.attempt.view()?.progress,
            AttemptProgress::Checked { .. }
        ) {
            return Err(not_checked());
        }
    }
    Ok(())
}

fn target_for_card(
    connection: &Connection,
    session_id: &str,
    card_id: &str,
) -> Result<Option<AttemptIdentityInput>, CommandError> {
    let stored: Option<(String, Option<String>)> = connection
        .query_row(
            "SELECT n.note_type, snap.question_instance_id
         FROM cards c JOIN notes n ON n.id = c.note_id
         LEFT JOIN practice_session_card_snapshots snap
           ON snap.card_id = c.id AND snap.session_id = ?1
         WHERE c.id = ?2",
            params![session_id, card_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    match stored {
        Some((kind, Some(instance_id))) if kind == "interactive" => {
            Ok(Some(AttemptIdentityInput {
                session_id: session_id.to_string(),
                card_id: card_id.to_string(),
                instance_id,
            }))
        }
        Some((kind, None)) if kind != "interactive" => Ok(None),
        _ => Err(invalid_storage()),
    }
}

fn not_checked() -> CommandError {
    CommandError::new(
        "question_not_checked",
        "Check your answer before choosing its difficulty.",
    )
}
