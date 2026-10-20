//! A lesson step's check as the learner sees and answers it.
//!
//! The reader gets a check's prompt and options but not its answer. Answering
//! is a native call that says whether the choice was right, explains it, and
//! records the answer in the learner's progress. It never writes a review or
//! changes a schedule.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use super::{progress, require_lesson, revision_content, LessonProgressDto};
use crate::{
    cards::questions::{QuestionBody, QuestionDefinition},
    database::Database,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

/// A single-choice check without its answer. Other question kinds are named
/// by the step's `checkKind` but cannot be answered in a lesson yet.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonCheckDto {
    pub prompt: String,
    pub options: Vec<LessonCheckOptionDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonCheckOptionDto {
    pub id: String,
    pub text: String,
}

/// The learner's latest answer to one step's check.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnsweredCheckDto {
    pub option_id: String,
    pub correct: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnswerLessonCheckInput {
    pub lesson_id: String,
    pub revision_number: i64,
    pub step_key: String,
    pub option_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonCheckResultDto {
    pub correct: bool,
    pub correct_option_id: String,
    pub explanation: Option<String>,
    pub progress: LessonProgressDto,
}

pub(super) fn view(check: &QuestionDefinition) -> Option<LessonCheckDto> {
    match &check.question {
        QuestionBody::SingleChoice {
            prompt, options, ..
        } => {
            // Shown in id order, a stable order that does not follow the
            // author's, so the right answer is not always first.
            let mut options = options
                .iter()
                .map(|option| LessonCheckOptionDto {
                    id: option.id.clone(),
                    text: plain(&option.content),
                })
                .collect::<Vec<_>>();
            options.sort_by(|left, right| left.id.cmp(&right.id));
            Some(LessonCheckDto {
                prompt: plain(prompt),
                options,
            })
        }
        _ => None,
    }
}

/// Stored answers by step key. Rows written before answers were recorded
/// hold `{}`.
pub(super) fn parse_answers(
    json: &str,
) -> Result<BTreeMap<String, AnsweredCheckDto>, CommandError> {
    serde_json::from_str(json).map_err(CommandError::serialization)
}

#[tauri::command]
pub fn answer_lesson_check(
    database: State<'_, Database>,
    input: AnswerLessonCheckInput,
) -> Result<LessonCheckResultDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| answer(connection, input, now))
}

/// Grades the answer and records it. Answering again replaces the earlier
/// answer; answering a different revision than the stored progress starts
/// that revision's progress at this step, as moving to it would.
pub(crate) fn answer(
    connection: &mut Connection,
    input: AnswerLessonCheckInput,
    now: i64,
) -> Result<LessonCheckResultDto, CommandError> {
    validate_entity_id(&input.lesson_id, "lesson")?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    require_lesson(&transaction, &input.lesson_id)?;
    let content = revision_content(&transaction, &input.lesson_id, input.revision_number)?.content;
    let (index, step) = content
        .steps
        .iter()
        .enumerate()
        .find(|(_, step)| step.key == input.step_key)
        .ok_or_else(|| {
            CommandError::new(
                "lesson_step_missing",
                "That step is not part of this lesson.",
            )
        })?;
    let Some(QuestionDefinition {
        question:
            QuestionBody::SingleChoice {
                options,
                correct_option_id,
                explanation,
                ..
            },
        ..
    }) = &step.check
    else {
        return Err(CommandError::new(
            "lesson_check_missing",
            "This step has no check to answer here.",
        ));
    };
    if !options.iter().any(|option| option.id == input.option_id) {
        return Err(CommandError::new(
            "lesson_check_option_missing",
            "That answer is not one of this check's options.",
        ));
    }
    let answered = AnsweredCheckDto {
        option_id: input.option_id.clone(),
        correct: input.option_id == *correct_option_id,
    };
    let step_number = i64::try_from(index).map_err(|_| step_overflow())?;
    let saved = record(
        &transaction,
        &input.lesson_id,
        input.revision_number,
        step_number,
        &input.step_key,
        answered.clone(),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(LessonCheckResultDto {
        correct: answered.correct,
        correct_option_id: correct_option_id.clone(),
        explanation: explanation
            .as_ref()
            .map(plain)
            .filter(|text| !text.is_empty()),
        progress: saved,
    })
}

fn record(
    connection: &Connection,
    lesson_id: &str,
    revision_number: i64,
    step: i64,
    step_key: &str,
    answered: AnsweredCheckDto,
    now: i64,
) -> Result<LessonProgressDto, CommandError> {
    match progress(connection, lesson_id)? {
        Some(stored) if stored.revision_number == revision_number => {
            let mut checks = stored.checks.clone();
            if checks.get(step_key) == Some(&answered) {
                return Ok(stored);
            }
            checks.insert(step_key.to_string(), answered);
            let updated_at = now.max(stored.started_at);
            connection
                .execute(
                    "UPDATE lesson_progress
                     SET checks_json = ?2, revision = revision + 1, updated_at = ?3
                     WHERE lesson_id = ?1",
                    params![lesson_id, to_json(&checks)?, updated_at],
                )
                .map_err(CommandError::database)?;
            Ok(LessonProgressDto {
                checks,
                updated_at,
                revision: stored.revision + 1,
                ..stored
            })
        }
        stored => {
            let revision = stored.map_or(1, |stored| stored.revision + 1);
            let checks = BTreeMap::from([(step_key.to_string(), answered)]);
            connection
                .execute(
                    "INSERT INTO lesson_progress(
                         lesson_id, revision_number, current_step, furthest_step, checks_json,
                         revision, started_at, updated_at, finished_at
                     ) VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?6, NULL)
                     ON CONFLICT(lesson_id) DO UPDATE SET
                         revision_number = excluded.revision_number,
                         current_step = excluded.current_step,
                         furthest_step = excluded.furthest_step,
                         checks_json = excluded.checks_json,
                         revision = excluded.revision,
                         started_at = excluded.started_at,
                         updated_at = excluded.updated_at,
                         finished_at = NULL",
                    params![
                        lesson_id,
                        revision_number,
                        step,
                        to_json(&checks)?,
                        revision,
                        now
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(LessonProgressDto {
                revision_number,
                current_step: step,
                furthest_step: step,
                checks,
                started_at: now,
                updated_at: now,
                finished_at: None,
                revision,
            })
        }
    }
}

fn to_json(checks: &BTreeMap<String, AnsweredCheckDto>) -> Result<String, CommandError> {
    serde_json::to_string(checks).map_err(CommandError::serialization)
}

fn step_overflow() -> CommandError {
    CommandError::new(
        "lesson_step_missing",
        "That step is not part of this lesson.",
    )
}

/// The text of a rich document. Lesson checks are written as plain paragraphs.
fn plain(document: &Value) -> String {
    fn collect(node: &Value, out: &mut Vec<String>) {
        if let Some(text) = node.get("text").and_then(Value::as_str) {
            out.push(text.to_string());
        }
        if let Some(children) = node.get("content").and_then(Value::as_array) {
            for child in children {
                collect(child, out);
            }
        }
    }
    let mut paragraphs = Vec::new();
    for block in document
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let mut parts = Vec::new();
        collect(block, &mut parts);
        let text = parts.concat();
        if !text.trim().is_empty() {
            paragraphs.push(text.trim().to_string());
        }
    }
    paragraphs.join("\n\n")
}
