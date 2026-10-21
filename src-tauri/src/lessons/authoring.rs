//! Writing lessons and their revisions.
//!
//! A course written with AI is saved whole by `create_generated_course`, and
//! a lesson written again from its sources is its next revision. The
//! hand-written writers serve the tests and the fixture lesson until lessons
//! can be edited in the app.

use std::collections::{BTreeSet, HashMap};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    content::{canonical_json, LessonContent, ValidatedLesson},
    require_lesson, require_subject, summary, LessonSummaryDto,
};
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

/// The part of a subject a lesson written with AI teaches, from its
/// provenance: the document and the top-level topic of its study plan. Topic
/// identities outlive plan revisions.
pub(crate) fn taught_part(provenance: &Value) -> Option<(String, String)> {
    Some((
        provenance["documentId"].as_str()?.to_string(),
        provenance["topicIds"][0].as_str()?.to_string(),
    ))
}

/// Saves a course written again, whole. Each lesson becomes the next
/// revision of the lesson that teaches the same part, or a new lesson. The
/// course is ordered as written, and lessons whose part is no longer in the
/// study plan, and hand-written ones, keep their order after it.
pub(crate) fn rewrite_generated_course(
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
    let existing = course_lessons(&transaction, subject_id)?;
    let mut by_part = HashMap::new();
    for (id, part) in &existing {
        if let Some(part) = part {
            by_part.entry(part.clone()).or_insert_with(|| id.clone());
        }
    }
    let mut written = Vec::new();
    for (lesson, provenance) in &validated {
        let id = match taught_part(provenance).and_then(|part| by_part.remove(&part)) {
            Some(id) => {
                write_revision(
                    &transaction,
                    &id,
                    lesson,
                    Origin::Generated(provenance),
                    now,
                )?;
                transaction
                    .execute(
                        "UPDATE lessons SET title = ?2, revision = revision + 1,
                             updated_at = MAX(created_at, ?3)
                         WHERE id = ?1",
                        params![id, lesson.content.title, now],
                    )
                    .map_err(CommandError::database)?;
                id
            }
            None => insert_lesson(
                &transaction,
                subject_id,
                lesson,
                Origin::Generated(provenance),
                now,
            )?,
        };
        written.push(id);
    }
    let rest = existing
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| !written.contains(id))
        .collect::<Vec<_>>();
    for (position, id) in written.iter().chain(rest.iter()).enumerate() {
        let order = i64::try_from(position + 1)
            .ok()
            .and_then(|position| position.checked_mul(ORDER_STEP))
            .ok_or_else(|| {
                CommandError::new(
                    "library_order_too_large",
                    "The library contains too many ordered items.",
                )
            })?;
        transaction
            .execute(
                "UPDATE lessons SET sort_order = ?2 WHERE id = ?1",
                params![id, order],
            )
            .map_err(CommandError::database)?;
    }
    transaction.commit().map_err(CommandError::database)?;
    Ok(written)
}

/// A lesson id with the document and topic its newest revision teaches.
type CourseLesson = (String, Option<(String, String)>);

/// The subject's lessons in course order, each with the part its newest
/// revision teaches; none for a hand-written one.
fn course_lessons(
    connection: &Connection,
    subject_id: &str,
) -> Result<Vec<CourseLesson>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT lesson.id,
                    (SELECT revision.provenance_json FROM lesson_revisions revision
                     WHERE revision.lesson_id = lesson.id
                     ORDER BY revision.revision_number DESC LIMIT 1)
             FROM lessons lesson WHERE lesson.subject_id = ?1
             ORDER BY lesson.sort_order, lesson.id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([subject_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows
        .into_iter()
        .map(|(id, provenance)| {
            let part = provenance
                .and_then(|json| serde_json::from_str::<Value>(&json).ok())
                .and_then(|provenance| taught_part(&provenance));
            (id, part)
        })
        .collect())
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
    add_revision(connection, lesson_id, content, Origin::Authored, now)
}

/// Adds a lesson written again from its sources as its next revision. The
/// lesson keeps its place in the course; progress made in an earlier
/// revision stays with that revision.
pub(crate) fn add_generated_revision(
    connection: &mut Connection,
    lesson_id: &str,
    content: LessonContent,
    provenance: &Value,
    now: i64,
) -> Result<LessonSummaryDto, CommandError> {
    add_revision(
        connection,
        lesson_id,
        content,
        Origin::Generated(provenance),
        now,
    )
}

fn add_revision(
    connection: &mut Connection,
    lesson_id: &str,
    content: LessonContent,
    origin: Origin<'_>,
    now: i64,
) -> Result<LessonSummaryDto, CommandError> {
    let validated = ValidatedLesson::validate(content)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    require_lesson(&transaction, lesson_id)?;
    if write_revision(&transaction, lesson_id, &validated, origin, now)? {
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
