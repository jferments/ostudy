//! Transactional save/check/recovery for an already captured Practice question.
//!
//! The session capture owner inserts the private snapshot and retained assets.
//! This boundary never rebuilds an attempt from a subsequently edited note, and
//! never acknowledges a state change until its SQLite transaction has committed.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    assets::AssetDto,
    cards::questions::{
        AttemptProgress, AttemptView, CheckAnswerRequest, QuestionAttempt, SaveAnswerRequest, Verdict,
    },
    database::Database,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

mod storage;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttemptIdentityInput {
    pub session_id: String,
    pub card_id: String,
    pub instance_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavePracticeAnswerInput {
    pub target: AttemptIdentityInput,
    pub request: SaveAnswerRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckPracticeAnswerInput {
    pub target: AttemptIdentityInput,
    pub request: CheckAnswerRequest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeQuestionDto {
    pub attempt: AttemptView,
    pub assets: Vec<AssetDto>,
}

#[tauri::command]
pub fn get_practice_question_attempt(
    database: State<'_, Database>,
    input: AttemptIdentityInput,
) -> Result<PracticeQuestionDto, CommandError> {
    validate_target(&input)?;
    database.with_connection(|connection| read(connection, &input))
}

#[tauri::command]
pub fn save_practice_question_answer(
    database: State<'_, Database>,
    input: SavePracticeAnswerInput,
) -> Result<AttemptView, CommandError> {
    validate_target(&input.target)?;
    database.with_connection(|connection| save(connection, input))
}

#[tauri::command]
pub fn check_practice_question_answer(
    database: State<'_, Database>,
    input: CheckPracticeAnswerInput,
) -> Result<AttemptView, CommandError> {
    validate_target(&input.target)?;
    database.with_connection(|connection| check(connection, input))
}

fn read(
    connection: &Connection,
    target: &AttemptIdentityInput,
) -> Result<PracticeQuestionDto, CommandError> {
    validate_target(target)?;
    let stored = storage::load(connection, target)?;
    let view = stored.attempt.view()?;
    // Only visible asset identities leave Rust. In particular, an explanation
    // image is not included merely because the private checkpoint retains it.
    let assets = storage::visible_assets(connection, &target.instance_id, &view)?;
    Ok(PracticeQuestionDto {
        attempt: view,
        assets,
    })
}

fn save(
    connection: &mut Connection,
    input: SavePracticeAnswerInput,
) -> Result<AttemptView, CommandError> {
    validate_target(&input.target)?;
    require_same_instance(&input.target, &input.request.answer.instance_id)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let stored = storage::load(&transaction, &input.target)?;
    let mut attempt = stored.attempt.clone();
    attempt.save_answer(input.request)?;
    let view = attempt.view()?;
    if attempt.revision() != stored.attempt.revision() {
        stored.require_mutable()?;
        storage::persist(&transaction, &stored, &attempt, unix_time_millis()?)?;
    }
    // Do not perform a fallible asset reload after durable success. Callers can
    // reconcile media separately without repeating or changing the save identity.
    transaction.commit().map_err(CommandError::database)?;
    Ok(view)
}

fn check(
    connection: &mut Connection,
    input: CheckPracticeAnswerInput,
) -> Result<AttemptView, CommandError> {
    validate_target(&input.target)?;
    require_same_instance(&input.target, &input.request.answer.instance_id)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let stored = storage::load(&transaction, &input.target)?;
    let mut attempt = stored.attempt.clone();
    let now = unix_time_millis()?;
    // Identical checks replay the original timestamp/evaluation, even after the
    // subsequent review completed the session. A new check still needs an open
    // session. Domain identity validation runs before that replay distinction.
    let view = attempt.check_answer(input.request, now)?;
    if attempt.revision() != stored.attempt.revision() {
        stored.require_mutable()?;
        storage::persist(&transaction, &stored, &attempt, now)?;
        let changed = transaction
            .execute(
                "UPDATE practice_session_cards
                 SET viewed_at = COALESCE(viewed_at, ?1),
                     revealed_at = COALESCE(revealed_at, ?1)
                 WHERE session_id = ?2 AND card_id = ?3
                   AND rated_at IS NULL AND dismissed_at IS NULL",
                params![now, input.target.session_id, input.target.card_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(conflict());
        }
        super::refresh_session_counters(&transaction, &input.target.session_id, now)?;
    }
    transaction.commit().map_err(CommandError::database)?;
    Ok(view)
}

fn validate_target(input: &AttemptIdentityInput) -> Result<(), CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    validate_entity_id(&input.card_id, "question card")?;
    validate_entity_id(&input.instance_id, "question instance")
}

fn require_same_instance(
    target: &AttemptIdentityInput,
    answer_instance_id: &str,
) -> Result<(), CommandError> {
    if target.instance_id != answer_instance_id {
        return Err(CommandError::new(
            "question_instance_mismatch",
            "This answer belongs to a different captured question.",
        ));
    }
    Ok(())
}

fn conflict() -> CommandError {
    CommandError::new(
        "question_attempt_conflict",
        "The question attempt changed. Reload it before making another change.",
    )
}

fn invalid_storage() -> CommandError {
    CommandError::new(
        "invalid_question_checkpoint",
        "The saved question attempt does not match its captured evidence.",
    )
}
