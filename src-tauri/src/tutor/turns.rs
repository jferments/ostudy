//! Idempotent turn admission and publication. Private quiz criteria are not
//! returned by public reads. Provider work occurs between short transactions.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{
    ai::document_intelligence::DocumentTaskContext,
    database::Database,
    error::CommandError,
    knowledge::{
        contracts::{hash, invalid, SourceCitation},
        repository,
    },
    util::{generate_id, unix_time_millis},
};

use super::{
    context::StudyContext,
    contracts::{PrivateQuiz, ProviderAnswer, QuizFeedback, TutorMode, TutorTurn, TutorTurnInput},
    conversations, history,
};

#[derive(Clone, Debug)]
pub(crate) struct TurnIdentity {
    pub id: String,
    pub conversation_id: String,
    pub request_hash: String,
    pub accepted_revision: i64,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredAssessment {
    quiz: Option<PrivateQuiz>,
    feedback: Option<QuizFeedback>,
}

pub(crate) fn replay(
    database: &Database,
    input: &TutorTurnInput,
) -> Result<Option<TutorTurn>, CommandError> {
    let request_hash = request_hash(input)?;
    database.with_connection(|connection| {
        let saved: Option<(String, String)> = connection
            .query_row(
                "SELECT id, request_hash FROM tutor_turns WHERE operation_id = ?1",
                [&input.operation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        match saved {
            Some((id, saved_hash)) if saved_hash == request_hash => {
                load_public(connection, &id).map(Some)
            }
            Some(_) => Err(invalid(
                "This tutor operation was already used for a different message.",
            )),
            None => Ok(None),
        }
    })
}

pub(crate) fn begin(
    database: &Database,
    input: &TutorTurnInput,
    task: &DocumentTaskContext,
) -> Result<TurnIdentity, CommandError> {
    input.validate()?;
    let request_json = serde_json::to_string(input).map_err(CommandError::serialization)?;
    let request_hash = hash(&request_json);
    database.with_connection(|connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let conversation = conversations::load_summary(&transaction, &input.conversation_id)?;
        if conversation.revision != input.expected_revision {
            return Err(conversations::changed());
        }
        let running: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM tutor_turns WHERE conversation_id = ?1 AND state = 'running')",
            [&input.conversation_id], |row| row.get(0),
        ).map_err(CommandError::database)?;
        if running {
            return Err(CommandError::new("tutor_turn_running", "This conversation already has a response in progress."));
        }
        if let Some(question_id) = &input.question_id {
            load_question(&transaction, &input.conversation_id, question_id)?;
        }
        let sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM tutor_turns WHERE conversation_id = ?1",
            [&input.conversation_id], |row| row.get(0),
        ).map_err(CommandError::database)?;
        let id = generate_id(&transaction)?;
        let now = unix_time_millis()?;
        transaction.execute(
            "INSERT INTO tutor_turns (
                 id, conversation_id, operation_id, sequence, request_hash, request_json,
                 mode, user_text, state, answered_question_id, profile_id, profile_revision,
                 provider_kind, model_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'running', ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
            params![id, input.conversation_id, input.operation_id, sequence, request_hash,
                request_json, input.mode.as_str(), input.text, input.question_id,
                task.profile_id, task.profile_revision, task.provider_kind, task.model_id, now],
        ).map_err(CommandError::database)?;
        transaction.execute(
            "UPDATE tutor_conversations SET revision = revision + 1, updated_at = ?1 WHERE id = ?2",
            params![now, input.conversation_id],
        ).map_err(CommandError::database)?;
        let accepted_revision = conversation.revision.checked_add(1)
            .ok_or_else(|| invalid("The conversation revision exceeded its supported range."))?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(TurnIdentity { id, conversation_id: input.conversation_id.clone(), request_hash, accepted_revision })
    })
}

pub(crate) fn complete(
    database: &Database,
    identity: &TurnIdentity,
    answer: &ProviderAnswer,
    context: &StudyContext,
    total_tokens: Option<i64>,
) -> Result<TutorTurn, CommandError> {
    let citations = context
        .sources
        .iter()
        .filter(|source| answer.source_ids.contains(&source.id))
        .map(SourceCitation::from)
        .collect::<Vec<_>>();
    let citation_json = serde_json::to_string(&citations).map_err(CommandError::serialization)?;
    let assessment_json = serde_json::to_string(&StoredAssessment {
        quiz: answer.quiz.clone(),
        feedback: answer.feedback.clone(),
    })
    .map_err(CommandError::serialization)?;
    let text = if context.notice.is_empty() {
        answer.text.clone()
    } else {
        format!("Context note: {}\n\n{}", context.notice, answer.text)
    };
    crate::knowledge::contracts::bounded(&text, 24_000, "published tutor message", false)?;
    database.with_connection(|connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let conversation = conversations::load_summary(&transaction, &identity.conversation_id)?;
        if conversation.revision != identity.accepted_revision {
            return Err(conversations::changed());
        }
        for source in &context.sources {
            if !repository::source_is_current(&transaction, source)? {
                return Err(repository::stale_source());
            }
        }
        history::verify_and_inherit(&transaction, &identity.conversation_id, &identity.id,
            &context.history, &context.inherited_turns)?;
        let now = unix_time_millis()?;
        let changed = transaction.execute(
            "UPDATE tutor_turns SET assistant_text = ?1, state = 'complete',
                 citations_json = ?2, quiz_json = ?3, total_tokens = ?4, updated_at = ?5
             WHERE id = ?6 AND request_hash = ?7 AND state = 'running'",
            params![text, citation_json, assessment_json, total_tokens, now, identity.id, identity.request_hash],
        ).map_err(CommandError::database)?;
        if changed != 1 {
            return Err(conversations::changed());
        }
        // Dependencies include all material supplied to the model, not just the
        // citations it elected to display. Hint citations stay entirely private.
        for source in &context.sources {
            transaction.execute(
                "INSERT OR IGNORE INTO tutor_turn_sources (turn_id, source_id, content_hash) VALUES (?1, ?2, ?3)",
                params![identity.id, source.id, source.content_hash],
            ).map_err(CommandError::database)?;
        }
        transaction.execute(
            "UPDATE tutor_conversations SET revision = revision + 1, updated_at = ?1 WHERE id = ?2",
            params![now, identity.conversation_id],
        ).map_err(CommandError::database)?;
        let result = load_public(&transaction, &identity.id)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

pub(crate) fn fail(
    database: &Database,
    identity: &TurnIdentity,
    error: &CommandError,
    total_tokens: Option<i64>,
) -> Result<TutorTurn, CommandError> {
    let state = if error.code == "ai_request_canceled" {
        "canceled"
    } else {
        "failed"
    };
    database.with_connection(|connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let now = unix_time_millis()?;
        let changed = transaction.execute(
            "UPDATE tutor_turns SET state = ?1, safe_error_code = ?2, safe_error_message = ?3,
                 total_tokens = ?4, updated_at = ?5
             WHERE id = ?6 AND request_hash = ?7 AND state = 'running'",
            params![state, error.code.chars().take(80).collect::<String>(),
                error.message.chars().take(1000).collect::<String>(), total_tokens, now, identity.id, identity.request_hash],
        ).map_err(CommandError::database)?;
        if changed == 1 {
            transaction.execute(
                "UPDATE tutor_conversations SET revision = revision + 1, updated_at = ?1 WHERE id = ?2",
                params![now, identity.conversation_id],
            ).map_err(CommandError::database)?;
        }
        let result = load_public(&transaction, &identity.id)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

pub(crate) fn load_public(connection: &Connection, id: &str) -> Result<TutorTurn, CommandError> {
    let row = connection
        .query_row(
            "SELECT id, operation_id, sequence, mode, user_text, assistant_text, state,
                citations_json, evidence_current, quiz_json, provider_kind, model_id,
                safe_error_code, safe_error_message, total_tokens, created_at, conversation_id
         FROM tutor_turns WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, bool>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                    row.get::<_, Option<i64>>(14)?,
                    row.get::<_, i64>(15)?,
                    row.get::<_, String>(16)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| invalid("This tutor message is no longer available."))?;
    let mode = match row.3.as_str() {
        "explain" => TutorMode::Explain,
        "hint" => TutorMode::Hint,
        "quiz" => TutorMode::Quiz,
        "answer" => TutorMode::Answer,
        _ => return Err(invalid("The stored tutor mode is invalid.")),
    };
    let assessment = row
        .9
        .map(|text| serde_json::from_str::<StoredAssessment>(&text))
        .transpose()
        .map_err(CommandError::serialization)?
        .unwrap_or_default();
    let pending_question = assessment.quiz.is_some()
        && row.6 == "complete"
        && row.8
        && question_is_pending(connection, &row.16, &row.0)?;
    Ok(TutorTurn {
        id: row.0,
        operation_id: row.1,
        sequence: row.2,
        mode,
        user_text: row.4,
        assistant_text: row.5,
        state: row.6,
        citations: serde_json::from_str(&row.7).map_err(CommandError::serialization)?,
        evidence_current: row.8,
        pending_question,
        feedback: assessment.feedback,
        provider: row.10,
        model: row.11,
        error_code: row.12,
        error_message: row.13,
        total_tokens: row.14,
        created_at: row.15,
    })
}

pub(crate) fn load_question(
    connection: &Connection,
    conversation_id: &str,
    question_id: &str,
) -> Result<PrivateQuiz, CommandError> {
    if !question_is_pending(connection, conversation_id, question_id)? {
        return Err(invalid("This quiz question was answered, superseded, or its source changed. Ask for a new question."));
    }
    let json: Option<String> = connection
        .query_row(
            "SELECT quiz_json FROM tutor_turns WHERE id = ?1 AND conversation_id = ?2
         AND state = 'complete' AND evidence_current = 1 AND mode = 'quiz'",
            params![question_id, conversation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .flatten();
    let assessment: StoredAssessment = serde_json::from_str(
        &json.ok_or_else(|| invalid("The selected quiz question is unavailable."))?,
    )
    .map_err(CommandError::serialization)?;
    let quiz = assessment
        .quiz
        .ok_or_else(|| invalid("The selected message is not a quiz question."))?;
    quiz.validate()?;
    Ok(quiz)
}

fn question_is_pending(
    connection: &Connection,
    conversation_id: &str,
    question_id: &str,
) -> Result<bool, CommandError> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM tutor_turns q
         WHERE q.id = ?1 AND q.conversation_id = ?2 AND q.mode = 'quiz'
           AND q.sequence > (SELECT memory_start_sequence FROM tutor_conversations WHERE id = ?2)
           AND q.state = 'complete' AND q.evidence_current = 1
           AND NOT EXISTS(SELECT 1 FROM tutor_turns a WHERE a.answered_question_id = q.id AND a.state IN ('running', 'complete'))
           AND NOT EXISTS(SELECT 1 FROM tutor_turns n WHERE n.conversation_id = q.conversation_id
                          AND n.sequence > q.sequence AND n.state = 'complete' AND n.mode IN ('explain', 'quiz')))",
        params![question_id, conversation_id], |row| row.get(0),
    ).map_err(CommandError::database)
}

fn request_hash(input: &TutorTurnInput) -> Result<String, CommandError> {
    Ok(hash(
        &serde_json::to_string(input).map_err(CommandError::serialization)?,
    ))
}

pub(crate) fn reconcile_startup(database: &Database) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let now = unix_time_millis()?;
        transaction.execute(
            "UPDATE tutor_conversations SET revision = revision + 1, updated_at = ?1
             WHERE id IN (SELECT conversation_id FROM tutor_turns WHERE state = 'running')",
            [now],
        ).map_err(CommandError::database)?;
        transaction.execute(
            "UPDATE tutor_turns SET state = 'interrupted', safe_error_code = 'tutor_turn_interrupted',
                 safe_error_message = 'The response was interrupted. Retry explicitly to send another request.',
                 updated_at = ?1 WHERE state = 'running'",
            [now],
        ).map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })
}
