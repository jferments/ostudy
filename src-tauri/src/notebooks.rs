//! Notebooks: a learner's own pages of paper, drawn and typed on with the
//! annotator's tools.
//!
//! A notebook (release schema 16) is filed in a subject like a deck or a
//! document, or in No subject. Its pages are ordered by `position` and each has
//! its own paper and size in points. Every mark on a page is one
//! `notebook_items` row, validated by the same rules as a document annotation
//! (`AnnotationShape`); a text mark may also be a link to a document page, a
//! study guide, a deck or another notebook page. Links are looked up when a
//! notebook opens and never followed by a write, so deleting their target
//! leaves the link in place, reported as unavailable.
//!
//! A page may be a board (release schema 26, `notebook_boards`): one large
//! sheet without paper edges, panned and zoomed around, for mind maps and
//! diagrams. It is stored as a page of `BOARD_SIZE` points a side, so its marks
//! follow the same rules as any other page's.
//!
//! Item writes carry the whole new state of each item, so a request repeated
//! after a lost response changes nothing further. Whole-library backups keep
//! notebooks; device sync packages leave them out.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    asset_lifecycle::{finalize_assets_for_owner, DraftFinalization},
    database::Database,
    document_annotations::{AnnotationShape, LinkTarget, TextNote},
    error::CommandError,
    paths::AppPaths,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_TITLE_CHARS: usize = 200;
const MAX_PAGES_PER_NOTEBOOK: i64 = 500;
const MAX_ITEMS_PER_NOTEBOOK: i64 = 50_000;
const MAX_CHANGES_PER_REQUEST: usize = 500;
/// Marks the list sends with each notebook so its first page can be drawn small.
const MAX_PREVIEW_ITEMS: i64 = 400;
/// Links a notebook's tile names, and how much of each label.
const MAX_LINK_LABELS: usize = 4;
const MAX_LINK_LABEL_CHARS: usize = 40;
/// A board's width and height in points: the largest page a notebook (or a
/// PDF) may have, 200 inches a side.
pub(crate) const BOARD_SIZE: f64 = 14_400.0;

/// The paper a page is drawn on.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Paper {
    Blank,
    Lined,
    Grid,
    Dots,
}

impl Paper {
    fn as_str(self) -> &'static str {
        match self {
            Self::Blank => "blank",
            Self::Lined => "lined",
            Self::Grid => "grid",
            Self::Dots => "dots",
        }
    }

    fn parse(value: &str) -> Result<Self, CommandError> {
        Ok(match value {
            "blank" => Self::Blank,
            "lined" => Self::Lined,
            "grid" => Self::Grid,
            "dots" => Self::Dots,
            _ => return Err(unreadable()),
        })
    }
}

/// Whether a page is a page of paper or a board.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PageLayout {
    #[default]
    Page,
    Board,
}

/// A new page's size: US Letter, A4, or a wide landscape page (11 by 17
/// inches) for mind maps and diagrams.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PageSize {
    Letter,
    A4,
    Wide,
}

impl PageSize {
    fn as_str(self) -> &'static str {
        match self {
            Self::Letter => "letter",
            Self::A4 => "a4",
            Self::Wide => "wide",
        }
    }

    fn parse(value: &str) -> Result<Self, CommandError> {
        Ok(match value {
            "letter" => Self::Letter,
            "a4" => Self::A4,
            "wide" => Self::Wide,
            _ => return Err(unreadable()),
        })
    }

    /// Width and height in points.
    fn points(self) -> (f64, f64) {
        match self {
            Self::Letter => (612.0, 792.0),
            Self::A4 => (595.28, 841.89),
            Self::Wide => (1224.0, 792.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookDto {
    pub id: String,
    pub subject_id: Option<String>,
    pub title: String,
    pub paper: Paper,
    pub page_size: PageSize,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookPageDto {
    pub id: String,
    pub position: i64,
    pub paper: Paper,
    /// The page size in points; a board's is `BOARD_SIZE` both ways.
    pub width: f64,
    pub height: f64,
    pub layout: PageLayout,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookItemDto {
    pub id: String,
    pub page_id: String,
    pub shape: AnnotationShape,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// One notebook in the list, with its first page's marks for a small preview.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookSummaryDto {
    pub notebook: NotebookDto,
    pub page_count: i64,
    pub link_count: i64,
    /// The first few links' labels, in page order, each once.
    pub link_labels: Vec<String>,
    /// Its Notes folder; None is the top of Notes.
    pub folder_id: Option<String>,
    /// When it was pinned; None when it is not.
    pub pinned_at: Option<i64>,
    pub first_page: Option<NotebookPageDto>,
    /// The first page's earliest marks, in drawing order.
    pub preview: Vec<NotebookItemDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookContentsDto {
    pub notebook: NotebookDto,
    /// In page order.
    pub pages: Vec<NotebookPageDto>,
    /// In drawing order on each page: earlier marks first.
    pub items: Vec<NotebookItemDto>,
    /// Links whose document, page, guide, deck or notebook page is gone.
    pub unavailable_link_ids: Vec<String>,
    /// The titles of the documents the notebook's clips came from, by
    /// document; a document that is gone is left out.
    pub clip_documents: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedNotebookItemChangesDto {
    /// The stored state of every item the request put.
    pub items: Vec<NotebookItemDto>,
    pub deleted_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateNotebookInput {
    pub title: String,
    pub subject_id: Option<String>,
    pub paper: Paper,
    pub page_size: PageSize,
    /// The first page is a board; later pages default to `page_size`.
    #[serde(default)]
    pub layout: PageLayout,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotebookInput {
    pub notebook_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameNotebookInput {
    pub notebook_id: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveNotebookToSubjectInput {
    pub notebook_id: String,
    pub subject_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddNotebookPageInput {
    pub notebook_id: String,
    /// Where the page goes, from 1; past the end adds it last.
    pub position: i64,
    /// Defaults to the notebook's paper and page size.
    pub paper: Option<Paper>,
    pub page_size: Option<PageSize>,
    /// A board ignores `page_size`.
    #[serde(default)]
    pub layout: PageLayout,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotebookPageInput {
    pub notebook_id: String,
    pub page_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveNotebookPageInput {
    pub notebook_id: String,
    pub page_id: String,
    /// The page's new position, from 1.
    pub position: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyNotebookItemChangesInput {
    pub notebook_id: String,
    pub changes: Vec<NotebookItemChange>,
}

/// The new state of one mark. A put creates it or replaces all of it, and may
/// move it to another page of the same notebook; deleting one that is already
/// gone succeeds.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum NotebookItemChange {
    /// A new picture names the image draft its asset was imported into, until
    /// a save has kept the picture; later puts of it leave the draft out.
    #[serde(rename_all = "camelCase")]
    Put {
        id: String,
        page_id: String,
        shape: Box<AnnotationShape>,
        #[serde(default)]
        asset_draft_id: Option<String>,
    },
    Delete {
        id: String,
    },
}

impl NotebookItemChange {
    fn id(&self) -> &str {
        match self {
            Self::Put { id, .. } | Self::Delete { id } => id,
        }
    }
}

#[tauri::command]
pub fn list_notebooks(
    database: State<'_, Database>,
) -> Result<Vec<NotebookSummaryDto>, CommandError> {
    database.with_connection(|connection| list(connection))
}

#[tauri::command]
pub fn create_notebook(
    database: State<'_, Database>,
    input: CreateNotebookInput,
) -> Result<NotebookContentsDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| create(connection, input, now))
}

#[tauri::command]
pub fn get_notebook(
    database: State<'_, Database>,
    input: NotebookInput,
) -> Result<NotebookContentsDto, CommandError> {
    validate_entity_id(&input.notebook_id, "notebook")?;
    database.with_connection(|connection| contents(connection, &input.notebook_id))
}

#[tauri::command]
pub fn rename_notebook(
    database: State<'_, Database>,
    input: RenameNotebookInput,
) -> Result<NotebookDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| rename(connection, input, now))
}

#[tauri::command]
pub fn move_notebook_to_subject(
    database: State<'_, Database>,
    input: MoveNotebookToSubjectInput,
) -> Result<NotebookDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| move_to_subject(connection, input, now))
}

#[tauri::command]
pub fn delete_notebook(
    database: State<'_, Database>,
    input: NotebookInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.notebook_id, "notebook")?;
    database.with_connection(|connection| delete(connection, &input.notebook_id))
}

#[tauri::command]
pub fn add_notebook_page(
    database: State<'_, Database>,
    input: AddNotebookPageInput,
) -> Result<NotebookContentsDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| add_page(connection, input, now))
}

#[tauri::command]
pub fn delete_notebook_page(
    database: State<'_, Database>,
    input: NotebookPageInput,
) -> Result<NotebookContentsDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| delete_page(connection, input, now))
}

#[tauri::command]
pub fn move_notebook_page(
    database: State<'_, Database>,
    input: MoveNotebookPageInput,
) -> Result<NotebookContentsDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| move_page(connection, input, now))
}

#[tauri::command]
pub fn apply_notebook_item_changes(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ApplyNotebookItemChangesInput,
) -> Result<AppliedNotebookItemChangesDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| apply_changes(connection, paths.inner(), input, now))
}

pub(crate) fn list(connection: &Connection) -> Result<Vec<NotebookSummaryDto>, CommandError> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {NOTEBOOK_COLUMNS},
                    (SELECT COUNT(*) FROM notebook_pages p WHERE p.notebook_id = n.id),
                    (SELECT COUNT(*) FROM notebook_items i
                     JOIN notebook_pages p ON p.id = i.page_id
                     WHERE p.notebook_id = n.id AND i.kind = 'text'
                       AND json_type(i.payload_json, '$.link') IS NOT NULL)
             FROM notebooks n
             ORDER BY n.updated_at DESC, n.id"
        ))
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                notebook_row(row),
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut summaries = Vec::with_capacity(rows.len());
    for (notebook, page_count, link_count) in rows {
        let notebook = notebook?;
        let first_page = pages(connection, &notebook.id)?.into_iter().next();
        let preview = match &first_page {
            Some(page) => page_items(connection, &page.id, MAX_PREVIEW_ITEMS)?,
            None => Vec::new(),
        };
        let link_labels = if link_count > 0 {
            link_labels(connection, &notebook.id)?
        } else {
            Vec::new()
        };
        let shelf = folders::shelf(connection, &notebook.id)?;
        summaries.push(NotebookSummaryDto {
            notebook,
            page_count,
            link_count,
            link_labels,
            folder_id: shelf.folder_id,
            pinned_at: shelf.pinned_at,
            first_page,
            preview,
        });
    }
    Ok(summaries)
}

pub(crate) fn create(
    connection: &mut Connection,
    input: CreateNotebookInput,
    now: i64,
) -> Result<NotebookContentsDto, CommandError> {
    let title = clean_title(&input.title)?;
    if let Some(subject_id) = input.subject_id.as_deref() {
        validate_entity_id(subject_id, "subject")?;
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    if let Some(subject_id) = input.subject_id.as_deref() {
        require_subject(&transaction, subject_id)?;
    }
    let id = generate_id(&transaction)?;
    transaction
        .execute(
            "INSERT INTO notebooks(id, subject_id, title, paper, page_size, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
            params![
                id,
                input.subject_id,
                title,
                input.paper.as_str(),
                input.page_size.as_str(),
                now
            ],
        )
        .map_err(CommandError::database)?;
    insert_page(
        &transaction,
        &id,
        1,
        input.paper,
        input.page_size,
        input.layout,
        now,
    )?;
    let contents = contents(&transaction, &id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(contents)
}

pub(crate) fn contents(
    connection: &Connection,
    notebook_id: &str,
) -> Result<NotebookContentsDto, CommandError> {
    let notebook = require_notebook(connection, notebook_id)?;
    let pages = pages(connection, notebook_id)?;
    let mut items = Vec::new();
    for page in &pages {
        items.extend(page_items(connection, &page.id, i64::MAX)?);
    }
    let unavailable_link_ids = unavailable_links(
        connection,
        items.iter().map(|item| (item.id.as_str(), &item.shape)),
    )?;
    let clip_documents = clips::clip_document_titles(connection, &items)?;
    Ok(NotebookContentsDto {
        notebook,
        pages,
        items,
        unavailable_link_ids,
        clip_documents,
    })
}

fn rename(
    connection: &mut Connection,
    input: RenameNotebookInput,
    now: i64,
) -> Result<NotebookDto, CommandError> {
    validate_entity_id(&input.notebook_id, "notebook")?;
    let title = clean_title(&input.title)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let notebook = require_notebook(&transaction, &input.notebook_id)?;
    if notebook.title != title {
        transaction
            .execute(
                "UPDATE notebooks SET title = ?2, revision = revision + 1, updated_at = ?3
                 WHERE id = ?1",
                params![input.notebook_id, title, now.max(notebook.created_at)],
            )
            .map_err(CommandError::database)?;
    }
    let notebook = require_notebook(&transaction, &input.notebook_id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(notebook)
}

fn move_to_subject(
    connection: &mut Connection,
    input: MoveNotebookToSubjectInput,
    now: i64,
) -> Result<NotebookDto, CommandError> {
    validate_entity_id(&input.notebook_id, "notebook")?;
    if let Some(subject_id) = input.subject_id.as_deref() {
        validate_entity_id(subject_id, "subject")?;
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let notebook = require_notebook(&transaction, &input.notebook_id)?;
    if let Some(subject_id) = input.subject_id.as_deref() {
        require_subject(&transaction, subject_id)?;
    }
    if notebook.subject_id != input.subject_id {
        transaction
            .execute(
                "UPDATE notebooks SET subject_id = ?2, revision = revision + 1, updated_at = ?3
                 WHERE id = ?1",
                params![
                    input.notebook_id,
                    input.subject_id,
                    now.max(notebook.created_at)
                ],
            )
            .map_err(CommandError::database)?;
    }
    let notebook = require_notebook(&transaction, &input.notebook_id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(notebook)
}

/// Deleting a notebook that is already gone succeeds, so a repeated request is harmless.
fn delete(connection: &Connection, notebook_id: &str) -> Result<(), CommandError> {
    connection
        .execute("DELETE FROM notebooks WHERE id = ?1", [notebook_id])
        .map(|_| ())
        .map_err(CommandError::database)
}

fn add_page(
    connection: &mut Connection,
    input: AddNotebookPageInput,
    now: i64,
) -> Result<NotebookContentsDto, CommandError> {
    validate_entity_id(&input.notebook_id, "notebook")?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let notebook = require_notebook(&transaction, &input.notebook_id)?;
    let count = page_count(&transaction, &input.notebook_id)?;
    if count >= MAX_PAGES_PER_NOTEBOOK {
        return Err(CommandError::new(
            "notebook_page_limit",
            format!("A notebook can have up to {MAX_PAGES_PER_NOTEBOOK} pages."),
        ));
    }
    let position = input.position.clamp(1, count + 1);
    shift_positions(&transaction, &input.notebook_id, position, count, 1)?;
    insert_page(
        &transaction,
        &input.notebook_id,
        position,
        input.paper.unwrap_or(notebook.paper),
        input.page_size.unwrap_or(notebook.page_size),
        input.layout,
        now,
    )?;
    touch(&transaction, &notebook, now)?;
    let contents = contents(&transaction, &input.notebook_id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(contents)
}

fn delete_page(
    connection: &mut Connection,
    input: NotebookPageInput,
    now: i64,
) -> Result<NotebookContentsDto, CommandError> {
    validate_entity_id(&input.notebook_id, "notebook")?;
    validate_entity_id(&input.page_id, "page")?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let notebook = require_notebook(&transaction, &input.notebook_id)?;
    // A page that is already gone is not an error: the request was repeated.
    if let Some(position) = page_position(&transaction, &input.notebook_id, &input.page_id)? {
        let count = page_count(&transaction, &input.notebook_id)?;
        if count <= 1 {
            return Err(CommandError::new(
                "notebook_last_page",
                "A notebook keeps at least one page. Delete the notebook instead.",
            ));
        }
        transaction
            .execute("DELETE FROM notebook_pages WHERE id = ?1", [&input.page_id])
            .map_err(CommandError::database)?;
        shift_positions(&transaction, &input.notebook_id, position + 1, count, -1)?;
        touch(&transaction, &notebook, now)?;
    }
    let contents = contents(&transaction, &input.notebook_id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(contents)
}

fn move_page(
    connection: &mut Connection,
    input: MoveNotebookPageInput,
    now: i64,
) -> Result<NotebookContentsDto, CommandError> {
    validate_entity_id(&input.notebook_id, "notebook")?;
    validate_entity_id(&input.page_id, "page")?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let notebook = require_notebook(&transaction, &input.notebook_id)?;
    let from = page_position(&transaction, &input.notebook_id, &input.page_id)?
        .ok_or_else(page_missing)?;
    let count = page_count(&transaction, &input.notebook_id)?;
    let to = input.position.clamp(1, count);
    if from != to {
        // Positions are unique, so the page steps aside while the others move.
        transaction
            .execute(
                "UPDATE notebook_pages SET position = ?2 WHERE id = ?1",
                params![input.page_id, count + 1],
            )
            .map_err(CommandError::database)?;
        if to < from {
            shift_positions(&transaction, &input.notebook_id, to, from - 1, 1)?;
        } else {
            shift_positions(&transaction, &input.notebook_id, from + 1, to, -1)?;
        }
        transaction
            .execute(
                "UPDATE notebook_pages SET position = ?2 WHERE id = ?1",
                params![input.page_id, to],
            )
            .map_err(CommandError::database)?;
        touch(&transaction, &notebook, now)?;
    }
    let contents = contents(&transaction, &input.notebook_id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(contents)
}

pub(crate) fn apply_changes(
    connection: &mut Connection,
    paths: &AppPaths,
    input: ApplyNotebookItemChangesInput,
    now: i64,
) -> Result<AppliedNotebookItemChangesDto, CommandError> {
    validate_entity_id(&input.notebook_id, "notebook")?;
    if input.changes.is_empty() || input.changes.len() > MAX_CHANGES_PER_REQUEST {
        return Err(invalid(format!(
            "Save between 1 and {MAX_CHANGES_PER_REQUEST} changes at a time."
        )));
    }
    let mut ids = BTreeSet::new();
    for change in &input.changes {
        validate_item_id(change.id())?;
        if !ids.insert(change.id()) {
            return Err(invalid("A request changes the same mark twice."));
        }
        if let NotebookItemChange::Put {
            page_id,
            shape,
            asset_draft_id,
            ..
        } = change
        {
            validate_entity_id(page_id, "page")?;
            shape.validate()?;
            if asset_draft_id.is_some() && shape.image_asset().is_none() {
                return Err(invalid("Only a picture names an image draft."));
            }
            if matches!(&**shape, AnnotationShape::Highlight(stroke) if stroke.rects.is_some()) {
                return Err(invalid("A notebook page has no text to highlight."));
            }
        }
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let notebook = require_notebook(&transaction, &input.notebook_id)?;
    let pages = pages(&transaction, &input.notebook_id)?
        .into_iter()
        .map(|page| page.id)
        .collect::<BTreeSet<_>>();
    // New marks draw on top of the notebook's existing ones, in request order,
    // even when they share a request or the clock has gone backwards.
    let latest_created_at: Option<i64> = transaction
        .query_row(
            "SELECT MAX(i.created_at) FROM notebook_items i
             JOIN notebook_pages p ON p.id = i.page_id
             WHERE p.notebook_id = ?1",
            [&input.notebook_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let mut next_created_at = latest_created_at.map_or(now, |latest| now.max(latest + 1));
    let mut items = Vec::new();
    let mut deleted_ids = Vec::new();
    let mut finalizations = Vec::new();
    for change in input.changes {
        match change {
            NotebookItemChange::Put {
                id,
                page_id,
                shape,
                asset_draft_id,
            } => {
                if !pages.contains(&page_id) {
                    return Err(page_missing());
                }
                if let Some(asset_id) = shape.image_asset() {
                    finalizations.push(keep_image(
                        &transaction,
                        paths,
                        &input.notebook_id,
                        asset_id,
                        asset_draft_id.as_deref(),
                    )?);
                }
                let image_asset = shape.image_asset().map(str::to_string);
                let item = put_item(
                    &transaction,
                    &input.notebook_id,
                    id,
                    page_id,
                    *shape,
                    now,
                    &mut next_created_at,
                )?;
                record_image(&transaction, &item.id, image_asset.as_deref())?;
                items.push(item);
            }
            NotebookItemChange::Delete { id } => {
                delete_item(&transaction, &input.notebook_id, &id)?;
                deleted_ids.push(id);
            }
        }
    }
    let count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM notebook_items i
             JOIN notebook_pages p ON p.id = i.page_id
             WHERE p.notebook_id = ?1",
            [&input.notebook_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if count > MAX_ITEMS_PER_NOTEBOOK {
        return Err(CommandError::new(
            "notebook_item_limit",
            format!(
                "A notebook can hold up to {MAX_ITEMS_PER_NOTEBOOK} marks. Erase some or start a new notebook."
            ),
        ));
    }
    touch(&transaction, &notebook, now)?;
    transaction.commit().map_err(CommandError::database)?;
    for finalization in finalizations {
        finalization.cleanup();
    }
    Ok(AppliedNotebookItemChangesDto { items, deleted_ids })
}

/// Makes a picture's asset durable for the notebook: a newly imported one is
/// promoted from its draft; one a save already kept needs nothing more. The
/// editor opens one "import" draft per picture it adds (the drafts ledger
/// names only card, template and import drafts), so committing it never
/// retires another picture still waiting to be saved.
fn keep_image(
    transaction: &rusqlite::Transaction<'_>,
    paths: &AppPaths,
    notebook_id: &str,
    asset_id: &str,
    draft_id: Option<&str>,
) -> Result<DraftFinalization, CommandError> {
    let state: Option<String> = transaction
        .query_row(
            "SELECT lifecycle_state FROM assets WHERE id = ?1",
            [asset_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let draft_id = if state.as_deref() == Some("staged") {
        Some(draft_id.ok_or_else(|| {
            CommandError::new(
                "asset_draft_mismatch",
                "A new picture names the image draft it was added to.",
            )
        })?)
    } else {
        None
    };
    finalize_assets_for_owner(
        transaction,
        paths,
        &[asset_id.to_string()],
        draft_id,
        Some(notebook_id),
    )
}

/// Records which asset a picture uses, so the asset store keeps it while the
/// picture is on a page; a mark that is not a picture uses none.
fn record_image(
    connection: &Connection,
    item_id: &str,
    asset_id: Option<&str>,
) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM notebook_item_assets WHERE item_id = ?1",
            [item_id],
        )
        .map_err(CommandError::database)?;
    if let Some(asset_id) = asset_id {
        connection
            .execute(
                "INSERT INTO notebook_item_assets(item_id, asset_id) VALUES (?1, ?2)",
                params![item_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

/// Create the mark, replace it when its content differs, or leave an
/// identical one (and its revision) as it is.
fn put_item(
    connection: &Connection,
    notebook_id: &str,
    id: String,
    page_id: String,
    shape: AnnotationShape,
    now: i64,
    next_created_at: &mut i64,
) -> Result<NotebookItemDto, CommandError> {
    let kind = shape.kind();
    let payload = shape.payload_json()?;
    let existing = connection
        .query_row(
            "SELECT p.notebook_id, i.page_id, i.kind, i.payload_json, i.revision, i.created_at, i.updated_at
             FROM notebook_items i JOIN notebook_pages p ON p.id = i.page_id
             WHERE i.id = ?1",
            [&id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((owner, stored_page, stored_kind, stored_payload, revision, created_at, updated_at)) =
        existing
    else {
        let created_at = *next_created_at;
        *next_created_at += 1;
        connection
            .execute(
                "INSERT INTO notebook_items(id, page_id, kind, payload_json, revision, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
                params![id, page_id, kind, payload, created_at],
            )
            .map_err(CommandError::database)?;
        return Ok(NotebookItemDto {
            id,
            page_id,
            shape,
            revision: 1,
            created_at,
            updated_at: created_at,
        });
    };
    if owner != notebook_id {
        return Err(other_notebook());
    }
    if stored_page == page_id && stored_kind == kind && stored_payload == payload {
        return Ok(NotebookItemDto {
            id,
            page_id,
            shape,
            revision,
            created_at,
            updated_at,
        });
    }
    let updated_at = now.max(created_at);
    connection
        .execute(
            "UPDATE notebook_items
             SET page_id = ?2, kind = ?3, payload_json = ?4, revision = revision + 1, updated_at = ?5
             WHERE id = ?1",
            params![id, page_id, kind, payload, updated_at],
        )
        .map_err(CommandError::database)?;
    Ok(NotebookItemDto {
        id,
        page_id,
        shape,
        revision: revision + 1,
        created_at,
        updated_at,
    })
}

fn delete_item(connection: &Connection, notebook_id: &str, id: &str) -> Result<(), CommandError> {
    let owner: Option<String> = connection
        .query_row(
            "SELECT p.notebook_id FROM notebook_items i JOIN notebook_pages p ON p.id = i.page_id
             WHERE i.id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    match owner {
        None => Ok(()),
        Some(owner) if owner != notebook_id => Err(other_notebook()),
        Some(_) => connection
            .execute("DELETE FROM notebook_items WHERE id = ?1", [id])
            .map(|_| ())
            .map_err(CommandError::database),
    }
}

/// The labels of a notebook's first links, in page order, each once.
fn link_labels(connection: &Connection, notebook_id: &str) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT json_extract(i.payload_json, '$.text') FROM notebook_items i
             JOIN notebook_pages p ON p.id = i.page_id
             WHERE p.notebook_id = ?1 AND i.kind = 'text'
               AND json_type(i.payload_json, '$.link') IS NOT NULL
             ORDER BY p.position, i.created_at, i.id
             LIMIT 40",
        )
        .map_err(CommandError::database)?;
    let texts = statement
        .query_map([notebook_id], |row| row.get::<_, Option<String>>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut labels: Vec<String> = Vec::new();
    for text in texts.into_iter().flatten() {
        let label = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let label: String = label.chars().take(MAX_LINK_LABEL_CHARS).collect();
        if !label.is_empty() && !labels.contains(&label) {
            labels.push(label);
        }
        if labels.len() == MAX_LINK_LABELS {
            break;
        }
    }
    Ok(labels)
}

/// The links among `items` whose target is gone: a deleted document, a page
/// past its end, a document without a study guide, a deleted deck, or a
/// notebook page that was removed. Guide pages hold links too.
pub(crate) fn unavailable_links<'a>(
    connection: &Connection,
    items: impl IntoIterator<Item = (&'a str, &'a AnnotationShape)>,
) -> Result<Vec<String>, CommandError> {
    let mut page_counts: HashMap<String, Option<i64>> = HashMap::new();
    let mut unavailable = Vec::new();
    for (id, shape) in items {
        let AnnotationShape::Text(TextNote {
            link: Some(link), ..
        }) = shape
        else {
            continue;
        };
        let available = match &link.target {
            LinkTarget::DocumentPage {
                document_id,
                page_number,
            } => {
                let pages = match page_counts.get(document_id) {
                    Some(pages) => *pages,
                    None => {
                        let pages = document_page_count(connection, document_id)?;
                        page_counts.insert(document_id.clone(), pages);
                        pages
                    }
                };
                pages.is_some_and(|pages| *page_number <= pages)
            }
            LinkTarget::StudyGuide { document_id } => exists(
                connection,
                "SELECT EXISTS(SELECT 1 FROM document_study_guides
                               WHERE document_id = ?1 AND state = 'active')",
                document_id,
            )?,
            LinkTarget::Deck { deck_id } => exists(
                connection,
                "SELECT EXISTS(SELECT 1 FROM decks WHERE id = ?1 AND deleted_at IS NULL)",
                deck_id,
            )?,
            LinkTarget::NotebookPage {
                notebook_id,
                page_id,
            } => connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM notebook_pages WHERE id = ?1 AND notebook_id = ?2)",
                    params![page_id, notebook_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?,
        };
        if !available {
            unavailable.push(id.to_string());
        }
    }
    Ok(unavailable)
}

/// None when the document is gone. A document still being imported counts
/// no pages yet and is taken to have any page.
fn document_page_count(
    connection: &Connection,
    document_id: &str,
) -> Result<Option<i64>, CommandError> {
    connection
        .query_row(
            "SELECT source_unit_count FROM documents WHERE id = ?1",
            [document_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|pages| pages.map(|pages| if pages > 0 { pages } else { i64::MAX }))
        .map_err(CommandError::database)
}

fn exists(connection: &Connection, sql: &str, id: &str) -> Result<bool, CommandError> {
    connection
        .query_row(sql, [id], |row| row.get(0))
        .map_err(CommandError::database)
}

const NOTEBOOK_COLUMNS: &str =
    "n.id, n.subject_id, n.title, n.paper, n.page_size, n.revision, n.created_at, n.updated_at";

fn notebook_row(row: &rusqlite::Row<'_>) -> Result<NotebookDto, CommandError> {
    let read = || -> rusqlite::Result<_> {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, i64>(7)?,
        ))
    };
    let (id, subject_id, title, paper, page_size, revision, created_at, updated_at) =
        read().map_err(CommandError::database)?;
    Ok(NotebookDto {
        id,
        subject_id,
        title,
        paper: Paper::parse(&paper)?,
        page_size: PageSize::parse(&page_size)?,
        revision,
        created_at,
        updated_at,
    })
}

fn require_notebook(
    connection: &Connection,
    notebook_id: &str,
) -> Result<NotebookDto, CommandError> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {NOTEBOOK_COLUMNS} FROM notebooks n WHERE n.id = ?1"
        ))
        .map_err(CommandError::database)?;
    let mut rows = statement
        .query([notebook_id])
        .map_err(CommandError::database)?;
    match rows.next().map_err(CommandError::database)? {
        Some(row) => notebook_row(row),
        None => Err(CommandError::new(
            "notebook_not_found",
            "That notebook is no longer in the library.",
        )),
    }
}

fn require_subject(connection: &Connection, subject_id: &str) -> Result<(), CommandError> {
    if exists(
        connection,
        "SELECT EXISTS(SELECT 1 FROM document_collections WHERE id = ?1)",
        subject_id,
    )? {
        Ok(())
    } else {
        Err(CommandError::new(
            "subject_not_found",
            "That subject is no longer in the library.",
        ))
    }
}

fn pages(connection: &Connection, notebook_id: &str) -> Result<Vec<NotebookPageDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT p.id, p.position, p.paper, p.width, p.height, b.page_id IS NOT NULL
             FROM notebook_pages p LEFT JOIN notebook_boards b ON b.page_id = p.id
             WHERE p.notebook_id = ?1 ORDER BY p.position",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([notebook_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, bool>(5)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    rows.into_iter()
        .map(|(id, position, paper, width, height, board)| {
            Ok(NotebookPageDto {
                id,
                position,
                paper: Paper::parse(&paper)?,
                width,
                height,
                layout: if board {
                    PageLayout::Board
                } else {
                    PageLayout::Page
                },
            })
        })
        .collect()
}

fn page_items(
    connection: &Connection,
    page_id: &str,
    limit: i64,
) -> Result<Vec<NotebookItemDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, kind, payload_json, revision, created_at, updated_at FROM notebook_items
             WHERE page_id = ?1 ORDER BY created_at, id LIMIT ?2",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![page_id, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut items = Vec::with_capacity(rows.len());
    for (id, kind, payload, revision, created_at, updated_at) in rows {
        // Every write is validated, so a row that no longer parses was changed
        // outside oStudy. Skipping it keeps the rest of the notebook usable.
        match AnnotationShape::from_stored(&kind, &payload) {
            Ok(shape) => items.push(NotebookItemDto {
                id,
                page_id: page_id.to_string(),
                shape,
                revision,
                created_at,
                updated_at,
            }),
            Err(error) => crate::logging::warn_target(
                "notebooks",
                "notebook_item_unreadable",
                "A stored notebook mark could not be read and was skipped.",
                serde_json::json!({ "itemId": id, "error": error.to_string() }),
            ),
        }
    }
    Ok(items)
}

fn insert_page(
    connection: &Connection,
    notebook_id: &str,
    position: i64,
    paper: Paper,
    size: PageSize,
    layout: PageLayout,
    now: i64,
) -> Result<(), CommandError> {
    let id = generate_id(connection)?;
    let (width, height) = match layout {
        PageLayout::Page => size.points(),
        PageLayout::Board => (BOARD_SIZE, BOARD_SIZE),
    };
    // Ruled lines belong to writing down a page; a board spreads every way.
    let paper = match (layout, paper) {
        (PageLayout::Board, Paper::Lined) => Paper::Dots,
        (_, paper) => paper,
    };
    connection
        .execute(
            "INSERT INTO notebook_pages(id, notebook_id, position, paper, width, height, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, notebook_id, position, paper.as_str(), width, height, now],
        )
        .map_err(CommandError::database)?;
    if layout == PageLayout::Board {
        connection
            .execute(
                "INSERT INTO notebook_boards(page_id, created_at) VALUES (?1, ?2)",
                params![id, now],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn page_count(connection: &Connection, notebook_id: &str) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM notebook_pages WHERE notebook_id = ?1",
            [notebook_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

fn page_position(
    connection: &Connection,
    notebook_id: &str,
    page_id: &str,
) -> Result<Option<i64>, CommandError> {
    connection
        .query_row(
            "SELECT position FROM notebook_pages WHERE id = ?1 AND notebook_id = ?2",
            params![page_id, notebook_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)
}

/// Moves the pages at positions `from..=to` by `by`. Positions are unique, so
/// the pages are first parked past every real position, then set in place.
fn shift_positions(
    connection: &Connection,
    notebook_id: &str,
    from: i64,
    to: i64,
    by: i64,
) -> Result<(), CommandError> {
    if from > to {
        return Ok(());
    }
    let park = MAX_PAGES_PER_NOTEBOOK * 4;
    connection
        .execute(
            "UPDATE notebook_pages SET position = position + ?4
             WHERE notebook_id = ?1 AND position BETWEEN ?2 AND ?3",
            params![notebook_id, from, to, park],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "UPDATE notebook_pages SET position = position - ?4 + ?5
             WHERE notebook_id = ?1 AND position BETWEEN ?2 + ?4 AND ?3 + ?4",
            params![notebook_id, from, to, park, by],
        )
        .map(|_| ())
        .map_err(CommandError::database)
}

/// A change to a notebook's pages or marks moves it to the top of the list.
fn touch(connection: &Connection, notebook: &NotebookDto, now: i64) -> Result<(), CommandError> {
    connection
        .execute(
            "UPDATE notebooks SET updated_at = ?2 WHERE id = ?1",
            params![notebook.id, now.max(notebook.updated_at)],
        )
        .map(|_| ())
        .map_err(CommandError::database)
}

fn clean_title(title: &str) -> Result<String, CommandError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(invalid("A notebook needs a name."));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(invalid(format!(
            "A notebook's name can be up to {MAX_TITLE_CHARS} characters."
        )));
    }
    if title.chars().any(char::is_control) {
        return Err(invalid(
            "A notebook's name cannot contain control characters.",
        ));
    }
    Ok(title.to_string())
}

fn validate_item_id(id: &str) -> Result<(), CommandError> {
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

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new("notebook_invalid", message)
}

fn unreadable() -> CommandError {
    CommandError::new(
        "notebook_unreadable",
        "A stored notebook could not be read.",
    )
}

fn page_missing() -> CommandError {
    CommandError::new(
        "notebook_page_missing",
        "That page is not part of this notebook.",
    )
}

fn other_notebook() -> CommandError {
    CommandError::new(
        "notebook_item_conflict",
        "That mark belongs to another notebook.",
    )
}

#[path = "notebooks/save_pdf.rs"]
mod save_pdf;
pub(crate) use save_pdf::{prepare_notebook_pdf, PrepareNotebookPdfInput, PreparedNotebookPdfDto};
#[path = "notebooks/clips.rs"]
pub mod clips;
#[path = "notebooks/folders.rs"]
pub mod folders;
pub(crate) use clips::{stage_document_clip, StageDocumentClipInput, StagedDocumentClipDto};

#[cfg(test)]
#[path = "notebooks/tests.rs"]
mod tests;
