//! Ink on study materials: what a learner writes and draws on a lesson's
//! lecture slides and on a document study guide's PDF pages.
//!
//! Every mark is one `material_ink` row (release schema 21) on one page of one
//! material, validated by the same rules as a document annotation
//! (`AnnotationShape`). A lesson's pages are its slide keys and a guide's are
//! the keys its canvas gives each PDF page (`chapter:<id>:2`; marks from the
//! earlier single section pages keep a topic id). Both are laid out natively,
//! so marks are placed in the page's own points. These pages have no text layer, so
//! text highlights are refused. A guide's pages may hold links, as a
//! notebook's do, looked up when the guide opens; slides hold none.
//!
//! Writes carry the whole new state of each mark, so a request repeated after
//! a lost response changes nothing further. Whole-library backups keep the
//! marks; device sync leaves them on the device that made them.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    document_annotations::AnnotationShape,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

const MAX_MARKS_PER_MATERIAL: i64 = 20_000;
const MAX_CHANGES_PER_REQUEST: usize = 500;
const MAX_PAGE_KEY_CHARS: usize = 96;

/// The material a mark is drawn on.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum InkMaterial {
    /// A lesson's lecture slides, whichever revision is shown.
    #[serde(rename_all = "camelCase")]
    Lesson { lesson_id: String },
    /// A document's study guide, as its PDF pages.
    #[serde(rename_all = "camelCase")]
    StudyGuide { document_id: String },
}

impl InkMaterial {
    fn validate(&self) -> Result<(), CommandError> {
        match self {
            Self::Lesson { lesson_id } => validate_entity_id(lesson_id, "lesson"),
            Self::StudyGuide { document_id } => validate_entity_id(document_id, "document"),
        }
    }

    /// The owner column and id this material's marks are stored under.
    fn column(&self) -> (&'static str, &str) {
        match self {
            Self::Lesson { lesson_id } => ("lesson_id", lesson_id),
            Self::StudyGuide { document_id } => ("document_id", document_id),
        }
    }

    fn require(&self, connection: &Connection) -> Result<(), CommandError> {
        let (sql, id, missing) = match self {
            Self::Lesson { lesson_id } => (
                "SELECT 1 FROM lessons WHERE id = ?1",
                lesson_id,
                "That lesson is no longer in the library.",
            ),
            Self::StudyGuide { document_id } => (
                "SELECT 1 FROM documents WHERE id = ?1",
                document_id,
                "That document is no longer in the library.",
            ),
        };
        let found = connection
            .query_row(sql, [id], |_| Ok(()))
            .optional()
            .map_err(CommandError::database)?;
        found.ok_or_else(|| CommandError::new("material_ink_material_missing", missing))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InkMarkDto {
    pub id: String,
    /// The slide key or section topic id the mark is drawn on.
    pub page_key: String,
    pub shape: AnnotationShape,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialInkDto {
    /// Earlier marks first.
    pub marks: Vec<InkMarkDto>,
    /// Links whose document, page, guide, deck or notebook page is gone.
    pub unavailable_link_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedInkChangesDto {
    pub marks: Vec<InkMarkDto>,
    pub deleted_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetMaterialInkInput {
    pub material: InkMaterial,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyMaterialInkChangesInput {
    pub material: InkMaterial,
    pub changes: Vec<InkChange>,
}

/// The new state of one mark. A put creates it or replaces all of it, and may
/// move it to another page of the same material; deleting one that is
/// already gone succeeds.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum InkChange {
    #[serde(rename_all = "camelCase")]
    Put {
        id: String,
        page_key: String,
        shape: AnnotationShape,
    },
    Delete {
        id: String,
    },
}

impl InkChange {
    fn id(&self) -> &str {
        match self {
            Self::Put { id, .. } | Self::Delete { id } => id,
        }
    }
}

#[tauri::command]
pub fn get_material_ink(
    database: State<'_, Database>,
    input: GetMaterialInkInput,
) -> Result<MaterialInkDto, CommandError> {
    database.with_connection(|connection| read(connection, &input.material))
}

#[tauri::command]
pub fn apply_material_ink_changes(
    database: State<'_, Database>,
    input: ApplyMaterialInkChangesInput,
) -> Result<AppliedInkChangesDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| apply_changes(connection, input, now))
}

pub(crate) fn read(
    connection: &Connection,
    material: &InkMaterial,
) -> Result<MaterialInkDto, CommandError> {
    material.validate()?;
    material.require(connection)?;
    let (column, owner) = material.column();
    let mut statement = connection
        .prepare(&format!(
            "SELECT id, page_key, kind, payload_json, revision, created_at, updated_at
             FROM material_ink WHERE {column} = ?1 ORDER BY created_at, id"
        ))
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([owner], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut marks = Vec::with_capacity(rows.len());
    for (id, page_key, kind, payload, revision, created_at, updated_at) in rows {
        // Every write is validated, so a row that no longer parses was changed
        // outside oStudy. Skipping it keeps the rest of the marks usable.
        match AnnotationShape::from_stored(&kind, &payload) {
            Ok(shape) => marks.push(InkMarkDto {
                id,
                page_key,
                shape,
                revision,
                created_at,
                updated_at,
            }),
            Err(error) => crate::logging::warn_target(
                "material_ink",
                "material_ink_unreadable",
                "A stored slide or guide page mark could not be read and was skipped.",
                serde_json::json!({ "markId": id, "error": error.to_string() }),
            ),
        }
    }
    let unavailable_link_ids = crate::notebooks::unavailable_links(
        connection,
        marks.iter().map(|mark| (mark.id.as_str(), &mark.shape)),
    )?;
    Ok(MaterialInkDto {
        marks,
        unavailable_link_ids,
    })
}

pub(crate) fn apply_changes(
    connection: &mut Connection,
    input: ApplyMaterialInkChangesInput,
    now: i64,
) -> Result<AppliedInkChangesDto, CommandError> {
    input.material.validate()?;
    if input.changes.is_empty() || input.changes.len() > MAX_CHANGES_PER_REQUEST {
        return Err(invalid(format!(
            "Save between 1 and {MAX_CHANGES_PER_REQUEST} changes at a time."
        )));
    }
    let mut ids = BTreeSet::new();
    for change in &input.changes {
        validate_mark_id(change.id())?;
        if !ids.insert(change.id()) {
            return Err(invalid("A request changes the same mark twice."));
        }
        if let InkChange::Put {
            page_key, shape, ..
        } = change
        {
            validate_page_key(page_key)?;
            shape.validate()?;
            if matches!(shape, AnnotationShape::Highlight(stroke) if stroke.rects.is_some()) {
                return Err(invalid("A designed page has no text to highlight."));
            }
            if shape.is_link() && matches!(input.material, InkMaterial::Lesson { .. }) {
                return Err(invalid("Links go in notebooks or guides, not on slides."));
            }
            if shape.is_topic() {
                return Err(invalid("Mind maps go in notebooks."));
            }
        }
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    input.material.require(&transaction)?;
    let (column, owner) = input.material.column();
    // New marks draw on top of the material's existing ones, in request order,
    // even when they share a request or the clock has gone backwards.
    let latest_created_at: Option<i64> = transaction
        .query_row(
            &format!("SELECT MAX(created_at) FROM material_ink WHERE {column} = ?1"),
            [owner],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let mut next_created_at = latest_created_at.map_or(now, |latest| now.max(latest + 1));
    let mut marks = Vec::new();
    let mut deleted_ids = Vec::new();
    for change in input.changes {
        match change {
            InkChange::Put {
                id,
                page_key,
                shape,
            } => marks.push(put_mark(
                &transaction,
                &input.material,
                id,
                page_key,
                shape,
                now,
                &mut next_created_at,
            )?),
            InkChange::Delete { id } => {
                delete_mark(&transaction, &input.material, &id)?;
                deleted_ids.push(id);
            }
        }
    }
    let count: i64 = transaction
        .query_row(
            &format!("SELECT COUNT(*) FROM material_ink WHERE {column} = ?1"),
            [owner],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if count > MAX_MARKS_PER_MATERIAL {
        return Err(CommandError::new(
            "material_ink_limit",
            format!("Slides or a guide can hold up to {MAX_MARKS_PER_MATERIAL} marks. Erase some first."),
        ));
    }
    transaction.commit().map_err(CommandError::database)?;
    Ok(AppliedInkChangesDto { marks, deleted_ids })
}

/// The material a stored mark belongs to, if the mark exists.
fn owner_of(connection: &Connection, id: &str) -> Result<Option<InkMaterial>, CommandError> {
    connection
        .query_row(
            "SELECT lesson_id, document_id FROM material_ink WHERE id = ?1",
            [id],
            |row| {
                // The table holds exactly one of the two.
                Ok(match row.get::<_, Option<String>>(0)? {
                    Some(lesson_id) => InkMaterial::Lesson { lesson_id },
                    None => InkMaterial::StudyGuide {
                        document_id: row.get(1)?,
                    },
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
}

/// Create the mark, replace it when its content differs, or leave an
/// identical one (and its revision) as it is.
fn put_mark(
    connection: &Connection,
    material: &InkMaterial,
    id: String,
    page_key: String,
    shape: AnnotationShape,
    now: i64,
    next_created_at: &mut i64,
) -> Result<InkMarkDto, CommandError> {
    let kind = shape.kind();
    let payload = shape.payload_json()?;
    if let Some(owner) = owner_of(connection, &id)? {
        if &owner != material {
            return Err(other_material());
        }
        let (stored_page, stored_kind, stored_payload, revision, created_at, updated_at) =
            connection
                .query_row(
                    "SELECT page_key, kind, payload_json, revision, created_at, updated_at
                 FROM material_ink WHERE id = ?1",
                    [&id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, i64>(5)?,
                        ))
                    },
                )
                .map_err(CommandError::database)?;
        if stored_page == page_key && stored_kind == kind && stored_payload == payload {
            return Ok(InkMarkDto {
                id,
                page_key,
                shape,
                revision,
                created_at,
                updated_at,
            });
        }
        let updated_at = now.max(created_at);
        connection
            .execute(
                "UPDATE material_ink
                 SET page_key = ?2, kind = ?3, payload_json = ?4, revision = revision + 1, updated_at = ?5
                 WHERE id = ?1",
                params![id, page_key, kind, payload, updated_at],
            )
            .map_err(CommandError::database)?;
        return Ok(InkMarkDto {
            id,
            page_key,
            shape,
            revision: revision + 1,
            created_at,
            updated_at,
        });
    }
    let created_at = *next_created_at;
    *next_created_at += 1;
    let (lesson_id, document_id) = match material {
        InkMaterial::Lesson { lesson_id } => (Some(lesson_id.as_str()), None),
        InkMaterial::StudyGuide { document_id } => (None, Some(document_id.as_str())),
    };
    connection
        .execute(
            "INSERT INTO material_ink(
                 id, lesson_id, document_id, page_key, kind, payload_json, revision, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?7)",
            params![id, lesson_id, document_id, page_key, kind, payload, created_at],
        )
        .map_err(CommandError::database)?;
    Ok(InkMarkDto {
        id,
        page_key,
        shape,
        revision: 1,
        created_at,
        updated_at: created_at,
    })
}

fn delete_mark(
    connection: &Connection,
    material: &InkMaterial,
    id: &str,
) -> Result<(), CommandError> {
    match owner_of(connection, id)? {
        None => Ok(()),
        Some(owner) if &owner != material => Err(other_material()),
        Some(_) => connection
            .execute("DELETE FROM material_ink WHERE id = ?1", [id])
            .map(|_| ())
            .map_err(CommandError::database),
    }
}

fn validate_mark_id(id: &str) -> Result<(), CommandError> {
    if id.len() == 32
        && id
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_identifier",
            "The mark identifier is invalid.",
        ))
    }
}

/// Slide keys (`title`, `step:<key>`, `sources`) and topic ids are short
/// ASCII names.
fn validate_page_key(key: &str) -> Result<(), CommandError> {
    if !key.is_empty()
        && key.len() <= MAX_PAGE_KEY_CHARS
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':' | b'.'))
    {
        Ok(())
    } else {
        Err(invalid(
            "That page is not part of these slides or this guide.",
        ))
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new("material_ink_invalid", message)
}

fn other_material() -> CommandError {
    CommandError::new(
        "material_ink_conflict",
        "That mark belongs to other slides or another guide.",
    )
}

#[cfg(test)]
#[path = "material_ink/tests.rs"]
mod tests;
