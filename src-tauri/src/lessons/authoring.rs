//! Writing lessons and their revisions.
//!
//! A course written with AI is saved whole by `create_generated_course`. The
//! hand-written writers serve the tests and the fixture lesson until lessons
//! can be edited in the app.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    content::{canonical_json, LessonContent, ValidatedLesson},
    require_subject,
};
#[cfg(test)]
use super::{require_lesson, summary, LessonSummaryDto};
use crate::{error::CommandError, util::generate_id};

/// A course holds at most this many lessons.
pub(crate) const MAX_LESSONS_PER_SUBJECT: i64 = 200;
const ORDER_STEP: i64 = 1_000;

/// How a revision came to be: written by hand, or by AI with a record of
/// the request that wrote it.
pub(crate) enum Origin<'a> {
    #[cfg(test)]
    Authored,
    Generated(&'a Value),
}

/// Adds a lesson at the end of the subject's course with `content` as its
/// first revision.
#[cfg(test)]
pub(crate) fn create_authored_lesson(
    connection: &mut Connection,
    subject_id: &str,
    content: LessonContent,
    now: i64,
) -> Result<LessonSummaryDto, CommandError> {
    let validated = ValidatedLesson::validate(content)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    require_subject(&transaction, subject_id)?;
    let id = insert_lesson(&transaction, subject_id, &validated, Origin::Authored, now)?;
    let lesson = summary(&transaction, &id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(lesson)
}

/// Saves a whole course written with AI, in order, or nothing. A subject
/// that already has lessons is left as it is, so writing a course twice
/// cannot add a second copy.
pub(crate) fn create_generated_course(
    connection: &mut Connection,
    subject_id: &str,
    lessons: &[(LessonContent, Value)],
    now: i64,
) -> Result<Vec<String>, CommandError> {
    let validated = lessons
        .iter()
        .map(|(content, provenance)| {
            ValidatedLesson::validate(content.clone()).map(|lesson| (lesson, provenance))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    require_subject(&transaction, subject_id)?;
    ensure_course_empty(&transaction, subject_id)?;
    let ids = validated
        .iter()
        .map(|(lesson, provenance)| {
            insert_lesson(
                &transaction,
                subject_id,
                lesson,
                Origin::Generated(provenance),
                now,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(ids)
}

/// Refuses when the subject's course already has lessons.
pub(crate) fn ensure_course_empty(
    connection: &Connection,
    subject_id: &str,
) -> Result<(), CommandError> {
    let existing: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM lessons WHERE subject_id = ?1",
            [subject_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if existing > 0 {
        return Err(CommandError::new(
            "lesson_course_exists",
            "This subject already has lessons.",
        ));
    }
    Ok(())
}

fn insert_lesson(
    connection: &Connection,
    subject_id: &str,
    validated: &ValidatedLesson,
    origin: Origin<'_>,
    now: i64,
) -> Result<String, CommandError> {
    let (count, last_order): (i64, Option<i64>) = connection
        .query_row(
            "SELECT COUNT(*), MAX(sort_order) FROM lessons WHERE subject_id = ?1",
            [subject_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)?;
    if count >= MAX_LESSONS_PER_SUBJECT {
        return Err(CommandError::new(
            "lesson_limit",
            format!("A subject can hold up to {MAX_LESSONS_PER_SUBJECT} lessons."),
        ));
    }
    let id = generate_id(connection)?;
    let sort_order = last_order.map_or(Some(ORDER_STEP), |last| last.checked_add(ORDER_STEP));
    connection
        .execute(
            "INSERT INTO lessons(id, subject_id, sort_order, title, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
            params![
                id,
                subject_id,
                sort_order.ok_or_else(|| CommandError::new(
                    "library_order_too_large",
                    "The library contains too many ordered items.",
                ))?,
                validated.content.title,
                now
            ],
        )
        .map_err(CommandError::database)?;
    write_revision(connection, &id, validated, origin, now)?;
    Ok(id)
}

/// Adds `content` as the lesson's next revision. The same content as the
/// latest revision adds nothing, so a repeated request changes nothing.
#[cfg(test)]
pub(crate) fn add_authored_revision(
    connection: &mut Connection,
    lesson_id: &str,
    content: LessonContent,
    now: i64,
) -> Result<LessonSummaryDto, CommandError> {
    let validated = ValidatedLesson::validate(content)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    require_lesson(&transaction, lesson_id)?;
    if write_revision(&transaction, lesson_id, &validated, Origin::Authored, now)? {
        let created_at: i64 = transaction
            .query_row(
                "SELECT created_at FROM lessons WHERE id = ?1",
                [lesson_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE lessons SET title = ?2, revision = revision + 1, updated_at = ?3
                 WHERE id = ?1",
                params![lesson_id, validated.content.title, now.max(created_at)],
            )
            .map_err(CommandError::database)?;
    }
    let lesson = summary(&transaction, lesson_id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(lesson)
}

/// Writes the revision and the units it explains, stamped with their content
/// now. Returns false when the latest revision already holds this content.
fn write_revision(
    connection: &Connection,
    lesson_id: &str,
    validated: &ValidatedLesson,
    origin: Origin<'_>,
    now: i64,
) -> Result<bool, CommandError> {
    let (origin, provenance) = match origin {
        #[cfg(test)]
        Origin::Authored => ("authored", None),
        Origin::Generated(provenance) => (
            "generated",
            Some(serde_json::to_string(provenance).map_err(CommandError::serialization)?),
        ),
    };
    let json = canonical_json(&validated.content)?;
    let content_hash = format!("{:x}", Sha256::digest(json.as_bytes()));
    let latest: Option<(i64, String, String)> = connection
        .query_row(
            "SELECT revision_number, origin, content_hash FROM lesson_revisions
             WHERE lesson_id = ?1 ORDER BY revision_number DESC LIMIT 1",
            [lesson_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    if latest.as_ref().is_some_and(|(_, latest_origin, hash)| {
        origin == "authored" && latest_origin == "authored" && *hash == content_hash
    }) {
        return Ok(false);
    }
    let revision_number = latest.map_or(1, |(number, _, _)| number + 1);
    connection
        .execute(
            "INSERT INTO lesson_revisions(
                 lesson_id, revision_number, origin, content_json, content_hash,
                 provenance_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                lesson_id,
                revision_number,
                origin,
                json,
                content_hash,
                provenance,
                now
            ],
        )
        .map_err(CommandError::database)?;
    // Every knowledge unit the lesson explains, once each.
    let unit_ids = validated
        .content
        .steps
        .iter()
        .flat_map(|step| step.knowledge_unit_ids.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    for unit_id in unit_ids {
        let unit_hash: String = connection
            .query_row(
                "SELECT content_hash FROM knowledge_units WHERE id = ?1",
                [unit_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "lesson_unit_missing",
                    "A lesson can only explain knowledge that is in the library.",
                )
            })?;
        connection
            .execute(
                "INSERT INTO lesson_revision_units(
                     lesson_id, revision_number, unit_id, unit_content_hash
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![lesson_id, revision_number, unit_id, unit_hash],
            )
            .map_err(CommandError::database)?;
    }
    Ok(true)
}
