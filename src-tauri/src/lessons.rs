//! A subject's lessons: short teaching sequences written from its knowledge.
//!
//! The lessons of a subject, in their order, are its course. A lesson's text is
//! kept in immutable revisions (release schema 9); writing a lesson again adds a
//! revision and the highest one is the lesson a learner reads, while earlier
//! ones stay readable. Each revision records the knowledge units it explains
//! with their content hashes, so a later change to the source shows the lesson
//! as out of date without rewriting it.
//!
//! Progress is where a learner stopped, not what they know: it never writes a
//! review or changes a schedule. Lessons and progress are part of whole-library
//! backups; device sync leaves them out because it does not carry the
//! knowledge units they are written from.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    cards::questions::QuestionKind,
    database::Database,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

pub(crate) mod authoring;
pub(crate) mod checks;
pub(crate) mod content;
pub(crate) mod lecture;
pub(crate) mod narration;
pub(crate) mod pptx;
pub(crate) mod teaching;
pub(crate) mod writing;

#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod tests;

use content::{LessonBlock, ValidatedLesson};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListSubjectLessonsInput {
    pub subject_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetLessonInput {
    pub lesson_id: String,
    /// An earlier revision to read; the latest when absent.
    pub revision_number: Option<i64>,
}

/// Where the learner is in one revision of a lesson. Saving the same state
/// again changes nothing, so a request can be repeated.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveLessonProgressInput {
    pub lesson_id: String,
    pub revision_number: i64,
    pub current_step: i64,
    /// Marks the lesson finished. A finished lesson stays finished when it is
    /// read again.
    pub finished: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectLessonsDto {
    pub subject_id: String,
    /// In course order.
    pub lessons: Vec<LessonSummaryDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonSummaryDto {
    pub id: String,
    pub subject_id: String,
    pub title: String,
    pub revision: i64,
    /// The latest revision; absent while the lesson is planned but unwritten.
    pub written: Option<WrittenLessonDto>,
    pub progress: Option<LessonProgressDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WrittenLessonDto {
    pub revision_number: i64,
    pub origin: String,
    pub step_count: usize,
    pub check_count: usize,
    pub estimated_minutes: u32,
    pub sources: SourceStatus,
    pub created_at: i64,
}

/// How the knowledge a revision explains compares with the library now.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    /// Every unit is as it was when the lesson was written.
    Current,
    /// At least one unit's content has changed since.
    Changed,
    /// Nothing changed, but at least one unit is no longer extracted.
    Unavailable,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonProgressDto {
    /// The revision this progress belongs to, which may be older than the
    /// latest one.
    pub revision_number: i64,
    pub current_step: i64,
    pub furthest_step: i64,
    /// The latest answer to each step's check, by step key.
    pub checks: BTreeMap<String, checks::AnsweredCheckDto>,
    pub started_at: i64,
    pub updated_at: i64,
    pub finished_at: Option<i64>,
    pub revision: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonDto {
    pub lesson: LessonSummaryDto,
    /// The revision read, which `content` shows.
    pub revision_number: i64,
    pub sources: SourceStatus,
    pub content: LessonViewDto,
}

/// A lesson as the reader shows it. Checks come without their answers, which
/// stay native until a learner answers.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonViewDto {
    pub title: String,
    pub summary: String,
    pub objectives: Vec<String>,
    pub estimated_minutes: u32,
    pub steps: Vec<LessonStepViewDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonStepViewDto {
    pub key: String,
    pub title: String,
    pub blocks: Vec<LessonBlock>,
    pub narration: String,
    pub slide_points: Vec<String>,
    pub check_kind: Option<QuestionKind>,
    /// The check to answer, when its kind can be answered in a lesson.
    pub check: Option<checks::LessonCheckDto>,
    pub knowledge_unit_ids: Vec<String>,
}

#[tauri::command]
pub fn list_subject_lessons(
    database: State<'_, Database>,
    input: ListSubjectLessonsInput,
) -> Result<SubjectLessonsDto, CommandError> {
    database.with_connection(|connection| list_lessons(connection, &input.subject_id))
}

#[tauri::command]
pub fn get_lesson(
    database: State<'_, Database>,
    input: GetLessonInput,
) -> Result<LessonDto, CommandError> {
    database.with_connection(|connection| read_lesson(connection, input))
}

#[tauri::command]
pub fn save_lesson_progress(
    database: State<'_, Database>,
    input: SaveLessonProgressInput,
) -> Result<LessonProgressDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| save_progress(connection, input, now))
}

pub(crate) fn list_lessons(
    connection: &Connection,
    subject_id: &str,
) -> Result<SubjectLessonsDto, CommandError> {
    validate_entity_id(subject_id, "subject")?;
    require_subject(connection, subject_id)?;
    let ids = {
        let mut statement = connection
            .prepare("SELECT id FROM lessons WHERE subject_id = ?1 ORDER BY sort_order, id")
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([subject_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    let lessons = ids
        .iter()
        .map(|id| summary(connection, id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SubjectLessonsDto {
        subject_id: subject_id.to_string(),
        lessons,
    })
}

pub(crate) fn read_lesson(
    connection: &Connection,
    input: GetLessonInput,
) -> Result<LessonDto, CommandError> {
    validate_entity_id(&input.lesson_id, "lesson")?;
    let lesson = summary(connection, &input.lesson_id)?;
    let revision_number = match input.revision_number {
        Some(number) => number,
        None => lesson
            .written
            .as_ref()
            .map(|written| written.revision_number)
            .ok_or_else(|| {
                CommandError::new(
                    "lesson_not_written",
                    "This lesson has not been written yet.",
                )
            })?,
    };
    let validated = revision_content(connection, &input.lesson_id, revision_number)?;
    let sources = source_status(connection, &input.lesson_id, revision_number)?;
    let estimated_minutes = validated.estimated_minutes();
    let content = validated.content;
    Ok(LessonDto {
        lesson,
        revision_number,
        sources,
        content: LessonViewDto {
            title: content.title,
            summary: content.summary,
            objectives: content.objectives,
            estimated_minutes,
            steps: content
                .steps
                .into_iter()
                .map(|step| LessonStepViewDto {
                    key: step.key,
                    title: step.title,
                    blocks: step.blocks,
                    narration: step.narration,
                    slide_points: step.slide_points,
                    check_kind: step.check.as_ref().map(|check| check.question.kind()),
                    check: step.check.as_ref().and_then(checks::view),
                    knowledge_unit_ids: step.knowledge_unit_ids,
                })
                .collect(),
        },
    })
}

/// Records where the learner is. Progress for a different revision than the
/// stored one starts over on that revision: old positions are never mapped
/// onto rewritten steps.
pub(crate) fn save_progress(
    connection: &mut Connection,
    input: SaveLessonProgressInput,
    now: i64,
) -> Result<LessonProgressDto, CommandError> {
    validate_entity_id(&input.lesson_id, "lesson")?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    require_lesson(&transaction, &input.lesson_id)?;
    let steps = revision_content(&transaction, &input.lesson_id, input.revision_number)?
        .content
        .steps
        .len();
    if usize::try_from(input.current_step).map_or(true, |step| step >= steps) {
        return Err(CommandError::new(
            "lesson_step_missing",
            "That step is not part of this lesson.",
        ));
    }
    let stored = progress(&transaction, &input.lesson_id)?;
    let saved = match stored {
        Some(stored) if stored.revision_number == input.revision_number => {
            let finished_at = stored
                .finished_at
                .or_else(|| input.finished.then(|| now.max(stored.started_at)));
            let furthest_step = stored.furthest_step.max(input.current_step);
            if stored.current_step == input.current_step
                && stored.furthest_step == furthest_step
                && stored.finished_at == finished_at
            {
                stored
            } else {
                // A clock that went backwards must not break `updated_at >= started_at`.
                let updated_at = now.max(stored.started_at);
                transaction
                    .execute(
                        "UPDATE lesson_progress
                         SET current_step = ?2, furthest_step = ?3, finished_at = ?4,
                             revision = revision + 1, updated_at = ?5
                         WHERE lesson_id = ?1",
                        params![
                            input.lesson_id,
                            input.current_step,
                            furthest_step,
                            finished_at,
                            updated_at
                        ],
                    )
                    .map_err(CommandError::database)?;
                LessonProgressDto {
                    current_step: input.current_step,
                    furthest_step,
                    finished_at,
                    updated_at,
                    revision: stored.revision + 1,
                    ..stored
                }
            }
        }
        stored => {
            let revision = stored.map_or(1, |stored| stored.revision + 1);
            let finished_at = input.finished.then_some(now);
            transaction
                .execute(
                    "INSERT INTO lesson_progress(
                         lesson_id, revision_number, current_step, furthest_step, checks_json,
                         revision, started_at, updated_at, finished_at
                     ) VALUES (?1, ?2, ?3, ?3, '{}', ?4, ?5, ?5, ?6)
                     ON CONFLICT(lesson_id) DO UPDATE SET
                         revision_number = excluded.revision_number,
                         current_step = excluded.current_step,
                         furthest_step = excluded.furthest_step,
                         checks_json = excluded.checks_json,
                         revision = excluded.revision,
                         started_at = excluded.started_at,
                         updated_at = excluded.updated_at,
                         finished_at = excluded.finished_at",
                    params![
                        input.lesson_id,
                        input.revision_number,
                        input.current_step,
                        revision,
                        now,
                        finished_at
                    ],
                )
                .map_err(CommandError::database)?;
            LessonProgressDto {
                revision_number: input.revision_number,
                current_step: input.current_step,
                furthest_step: input.current_step,
                checks: BTreeMap::new(),
                started_at: now,
                updated_at: now,
                finished_at,
                revision,
            }
        }
    };
    transaction.commit().map_err(CommandError::database)?;
    Ok(saved)
}

fn summary(connection: &Connection, lesson_id: &str) -> Result<LessonSummaryDto, CommandError> {
    let (subject_id, title, revision) = connection
        .query_row(
            "SELECT subject_id, title, revision FROM lessons WHERE id = ?1",
            [lesson_id],
            |row| Ok((row.get::<_, String>(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(lesson_missing)?;
    let latest: Option<(i64, String, String, i64)> = connection
        .query_row(
            "SELECT revision_number, origin, content_json, created_at FROM lesson_revisions
             WHERE lesson_id = ?1 ORDER BY revision_number DESC LIMIT 1",
            [lesson_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let written = match latest {
        None => None,
        Some((revision_number, origin, json, created_at)) => {
            let validated = ValidatedLesson::parse(&json)?;
            Some(WrittenLessonDto {
                revision_number,
                origin,
                step_count: validated.content.steps.len(),
                check_count: validated
                    .content
                    .steps
                    .iter()
                    .filter(|step| step.check.is_some())
                    .count(),
                estimated_minutes: validated.estimated_minutes(),
                sources: source_status(connection, lesson_id, revision_number)?,
                created_at,
            })
        }
    };
    Ok(LessonSummaryDto {
        id: lesson_id.to_string(),
        subject_id,
        title,
        revision,
        written,
        progress: progress(connection, lesson_id)?,
    })
}

fn revision_content(
    connection: &Connection,
    lesson_id: &str,
    revision_number: i64,
) -> Result<ValidatedLesson, CommandError> {
    let json: String = connection
        .query_row(
            "SELECT content_json FROM lesson_revisions
             WHERE lesson_id = ?1 AND revision_number = ?2",
            params![lesson_id, revision_number],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "lesson_revision_missing",
                "That version of the lesson is no longer in the library.",
            )
        })?;
    ValidatedLesson::parse(&json)
}

fn source_status(
    connection: &Connection,
    lesson_id: &str,
    revision_number: i64,
) -> Result<SourceStatus, CommandError> {
    let (changed, missing): (i64, i64) = connection
        .query_row(
            "SELECT
                 COALESCE(SUM(unit.id IS NOT NULL AND unit.content_hash <> used.unit_content_hash), 0),
                 COALESCE(SUM(unit.id IS NULL), 0)
             FROM lesson_revision_units used
             LEFT JOIN knowledge_units unit ON unit.id = used.unit_id
             WHERE used.lesson_id = ?1 AND used.revision_number = ?2",
            params![lesson_id, revision_number],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)?;
    Ok(if changed > 0 {
        SourceStatus::Changed
    } else if missing > 0 {
        SourceStatus::Unavailable
    } else {
        SourceStatus::Current
    })
}

fn progress(
    connection: &Connection,
    lesson_id: &str,
) -> Result<Option<LessonProgressDto>, CommandError> {
    connection
        .query_row(
            "SELECT revision_number, current_step, furthest_step, started_at, updated_at,
                    finished_at, revision, checks_json
             FROM lesson_progress WHERE lesson_id = ?1",
            [lesson_id],
            |row| {
                Ok((
                    LessonProgressDto {
                        revision_number: row.get(0)?,
                        current_step: row.get(1)?,
                        furthest_step: row.get(2)?,
                        checks: BTreeMap::new(),
                        started_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        finished_at: row.get(5)?,
                        revision: row.get(6)?,
                    },
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .map(|(progress, checks)| {
            Ok(LessonProgressDto {
                checks: checks::parse_answers(&checks)?,
                ..progress
            })
        })
        .transpose()
}

fn require_subject(connection: &Connection, subject_id: &str) -> Result<(), CommandError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM document_collections WHERE id = ?1)",
            [subject_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists {
        Ok(())
    } else {
        Err(CommandError::new(
            "subject_not_found",
            "That subject is no longer in the library.",
        ))
    }
}

fn require_lesson(connection: &Connection, lesson_id: &str) -> Result<(), CommandError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM lessons WHERE id = ?1)",
            [lesson_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists {
        Ok(())
    } else {
        Err(lesson_missing())
    }
}

fn lesson_missing() -> CommandError {
    CommandError::new(
        "lesson_not_found",
        "That lesson is no longer in the library.",
    )
}
