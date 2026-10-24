//! The figures in a learner's PDF documents (release schema 17).
//!
//! When a learner imports a PDF they choose whether to use its images. With
//! images on, oStudy searches the PDF for figures once its pages are read: the
//! pictures and drawings worth studying, each with its caption and the passages
//! elsewhere that explain it (see [`detect`]). Nothing here contacts a
//! provider. Crops are not stored: the Figures page draws each one from the PDF
//! as a page tile, as the annotator does when zoomed in.
//!
//! Searching needs PDFium's page objects, which only the desktop build has;
//! elsewhere a search records that it is unsupported and finds nothing.
//! Searching again replaces the figures but keeps a figure the learner hid
//! hidden while its box is unchanged.
//!
//! With a model that reads images, oStudy then reads each figure on its page
//! (see [`reading`]): a title, what it shows, how the text uses it and how
//! much it matters. Searching again keeps what was read of a figure found
//! again in the same place.

pub(crate) mod detect;
pub(crate) mod reading;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    ai::{
        document_intelligence::{self, DocumentTaskContext},
        document_provider::load_document_provider_capabilities,
        AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::{self, PdfRenderRuntime},
    task_cancellation::CancellationSignal,
    util::{unix_time_millis, validate_entity_id},
};

use detect::{DetectedFigure, PageFigures, Rect};

/// A box the learner hid stays hidden on a figure found again this close to it.
const SAME_FIGURE_OVERLAP: f64 = 0.8;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DocumentFiguresInput {
    pub document_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SetDocumentFiguresUseImagesInput {
    pub document_id: String,
    pub use_images: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SetDocumentFigureHiddenInput {
    pub document_id: String,
    pub figure_id: String,
    pub hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FigureBoxDto {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FigurePassageDto {
    pub page_number: i64,
    /// `mention` names the figure's label; `neighbour` is a page beside a
    /// figure that fills its own.
    pub kind: String,
    pub excerpt: Option<String>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentFigureDto {
    pub figure_id: String,
    pub page_number: i64,
    /// The page's size and the boxes are in PDF points from the top-left
    /// corner of the page as displayed.
    pub page_width: f64,
    pub page_height: f64,
    pub kind: String,
    pub bounds: FigureBoxDto,
    pub caption_bounds: Option<FigureBoxDto>,
    pub label: Option<String>,
    pub caption: Option<String>,
    pub hidden: bool,
    pub passages: Vec<FigurePassageDto>,
    pub reading: Option<FigureReadingDto>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FigureReadingDto {
    pub title: String,
    pub shows: String,
    pub use_in_text: Option<String>,
    pub concepts: Vec<String>,
    /// Pages whose text explains the figure, in page order.
    pub explained_on: Vec<i64>,
    /// 1 (incidental) to 5 (central to the document).
    pub importance: i64,
    pub worth_studying: bool,
    pub model: String,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentFiguresDto {
    pub document_id: String,
    /// The learner's choice; a document imported before the choice existed
    /// counts as using its images.
    pub use_images: bool,
    /// `none` before any search, `pending` while images are off or a search
    /// has not finished, then `found`, `unsupported` or `failed`.
    pub state: String,
    pub scanned_at: Option<i64>,
    /// `none` until figures are read, then `read`, `partial` (some pages
    /// could not be read), `unavailable` (the model cannot see images) or
    /// `failed`.
    pub reading: String,
    pub read_at: Option<i64>,
    pub figures: Vec<DocumentFigureDto>,
}

#[tauri::command]
pub(crate) fn get_document_figures(
    database: State<'_, Database>,
    input: DocumentFiguresInput,
) -> Result<DocumentFiguresDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    database.with_connection(|connection| {
        require_pdf(connection, &input.document_id)?;
        figures(connection, &input.document_id)
    })
}

#[tauri::command]
pub(crate) fn set_document_figures_use_images(
    database: State<'_, Database>,
    input: SetDocumentFiguresUseImagesInput,
) -> Result<DocumentFiguresDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| set_use_images(connection, input, now))
}

#[tauri::command]
pub(crate) fn set_document_figure_hidden(
    database: State<'_, Database>,
    input: SetDocumentFigureHiddenInput,
) -> Result<DocumentFiguresDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| set_hidden(connection, input, now))
}

/// Searches one document for figures now, turning its images on.
pub(crate) async fn find_document_figures(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PdfRenderRuntime>,
    input: DocumentFiguresInput,
) -> Result<DocumentFiguresDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    let now = unix_time_millis()?;
    let relative_path = database.with_connection(|connection| {
        let relative_path = require_pdf(connection, &input.document_id)?;
        record_choice(connection, &input.document_id, true, now)?;
        Ok(relative_path)
    })?;
    let source_path = paths.resolve_relative(&relative_path);
    search_and_store(
        &database,
        runtime.inner().clone(),
        &input.document_id,
        source_path,
    )
    .await;
    database.with_connection(|connection| figures(connection, &input.document_id))
}

/// After a PDF's pages are read on import or reprocessing: searches it for
/// figures when the learner chose to use its images. A failed search is
/// recorded and logged; it never fails the import.
pub(crate) async fn find_after_ingestion(
    database: &Database,
    runtime: PdfRenderRuntime,
    document_id: &str,
    source_path: PathBuf,
) {
    let wanted = database.with_connection(|connection| use_images(connection, document_id));
    match wanted {
        Ok(true) => search_and_store(database, runtime, document_id, source_path).await,
        Ok(false) => {}
        Err(error) => log_failure(document_id, &error.message),
    }
}

/// Reads a document's figures now with the learner's model.
pub(crate) async fn read_document_figures(
    database: &Database,
    paths: &AppPaths,
    render: &PdfRenderRuntime,
    runtime: &AiRuntimeState,
    input: DocumentFiguresInput,
) -> Result<DocumentFiguresDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    database.with_connection(|connection| require_pdf(connection, &input.document_id))?;
    let task = figure_task(database)?;
    reading::read_figures(reading::FigureReadingRun {
        database,
        paths,
        render,
        runtime,
        task: &task,
        document_id: &input.document_id,
        cancellation: &CancellationSignal::new(),
    })
    .await?;
    database.with_connection(|connection| figures(connection, &input.document_id))
}

/// Figures are read by the model that writes the document's cards, without
/// reasoning, like the rest of the document's work.
fn figure_task(database: &Database) -> Result<DocumentTaskContext, CommandError> {
    let mut task = document_intelligence::load_document_task_context(database)?;
    let capabilities = load_document_provider_capabilities(
        database,
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    Ok(task)
}

/// What reading after an import needs.
pub(crate) struct ReadAfterSearch<'a> {
    pub database: &'a Database,
    pub paths: &'a AppPaths,
    pub render: &'a PdfRenderRuntime,
    pub runtime: &'a AiRuntimeState,
    pub task: &'a DocumentTaskContext,
    pub document_id: &'a str,
}

/// During generation: once the figure search an import started has finished,
/// reads the figures it found that are not read yet. A failure is logged and
/// never fails generation.
pub(crate) async fn read_after_search(
    input: ReadAfterSearch<'_>,
    search: Option<tauri::async_runtime::JoinHandle<()>>,
) {
    if let Some(search) = search {
        let _ = search.await;
    }
    let waiting = input
        .database
        .with_connection(|connection| reading::waiting_to_be_read(connection, input.document_id));
    let read = match waiting {
        Ok(true) => {
            reading::read_figures(reading::FigureReadingRun {
                database: input.database,
                paths: input.paths,
                render: input.render,
                runtime: input.runtime,
                task: input.task,
                document_id: input.document_id,
                cancellation: &crate::task_cancellation::current_signal()
                    .unwrap_or_else(CancellationSignal::new),
            })
            .await
        }
        Ok(false) => Ok(()),
        Err(error) => Err(error),
    };
    if let Err(error) = read {
        log_failure(input.document_id, &error.message);
    }
}

/// Records the learner's choice for a document they are importing.
pub(crate) fn record_import_choice(
    connection: &Connection,
    document_id: &str,
    use_images: bool,
    now: i64,
) -> Result<(), CommandError> {
    record_choice(connection, document_id, use_images, now)
}

async fn search_and_store(
    database: &Database,
    runtime: PdfRenderRuntime,
    document_id: &str,
    source_path: PathBuf,
) {
    let found = if !crate::pdf_render_backend::PAGE_LAYOUTS_SUPPORTED {
        Err(SearchFailure::Unsupported)
    } else {
        tauri::async_runtime::spawn_blocking(move || search(&runtime, &source_path))
            .await
            .unwrap_or_else(|error| Err(SearchFailure::Failed(error.to_string())))
    };
    let stored = database.with_connection(|connection| {
        let now = unix_time_millis()?;
        match &found {
            Ok(figures) => replace_figures(connection, document_id, figures, now),
            Err(SearchFailure::Unsupported) => {
                record_state(connection, document_id, "unsupported", now)
            }
            Err(SearchFailure::Failed(_)) => record_state(connection, document_id, "failed", now),
        }
    });
    if let Err(SearchFailure::Failed(detail)) = &found {
        log_failure(document_id, detail);
    }
    if let Err(error) = stored {
        log_failure(document_id, &error.message);
    }
}

enum SearchFailure {
    Unsupported,
    Failed(String),
}

fn search(
    runtime: &PdfRenderRuntime,
    source_path: &Path,
) -> Result<Vec<DetectedFigure>, SearchFailure> {
    let mut pages = Vec::new();
    pdf_render_backend::page_layouts(runtime, source_path, |layout| {
        pages.push(PageFigures::from_layout(&layout));
        Ok(())
    })
    .map_err(|error| SearchFailure::Failed(error.to_string()))?;
    Ok(detect::detect(pages))
}

fn log_failure(document_id: &str, detail: &str) {
    crate::logging::warn_target(
        "documents.figures",
        "document_figure_search_failed",
        "Figures could not be found in a document; it is still used as text.",
        serde_json::json!({
            "document": document_id,
            "detail": detail.chars().take(300).collect::<String>(),
        }),
    );
}

/// The stored PDF behind a document; other formats have no figures here yet.
fn require_pdf(connection: &Connection, document_id: &str) -> Result<String, CommandError> {
    crate::document_annotations::pdf_source_path(connection, document_id).map_err(|error| {
        if error.code == "document_not_found" {
            error
        } else {
            CommandError::new(
                "document_figures_unsupported",
                "Figures can be found in PDF documents only.",
            )
        }
    })
}

fn use_images(connection: &Connection, document_id: &str) -> Result<bool, CommandError> {
    let stored: Option<i64> = connection
        .query_row(
            "SELECT use_images FROM document_figure_scans WHERE document_id = ?1",
            [document_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    Ok(stored.is_none_or(|value| value == 1))
}

fn record_choice(
    connection: &Connection,
    document_id: &str,
    use_images: bool,
    now: i64,
) -> Result<(), CommandError> {
    connection
        .execute(
            "INSERT INTO document_figure_scans(document_id, use_images, state, updated_at)
             VALUES (?1, ?2, 'pending', ?3)
             ON CONFLICT(document_id) DO UPDATE SET
                 use_images = excluded.use_images,
                 updated_at = excluded.updated_at",
            params![document_id, i64::from(use_images), now.max(0)],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn record_state(
    connection: &Connection,
    document_id: &str,
    state: &str,
    now: i64,
) -> Result<(), CommandError> {
    connection
        .execute(
            "INSERT INTO document_figure_scans(
                 document_id, use_images, state, detector, scanned_at, updated_at
             ) VALUES (?1, 1, ?2, ?3, ?4, ?4)
             ON CONFLICT(document_id) DO UPDATE SET
                 state = excluded.state,
                 detector = excluded.detector,
                 scanned_at = excluded.scanned_at,
                 updated_at = excluded.updated_at",
            params![document_id, state, detect::DETECTOR, now.max(0)],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn set_use_images(
    connection: &mut Connection,
    input: SetDocumentFiguresUseImagesInput,
    now: i64,
) -> Result<DocumentFiguresDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    require_pdf(&transaction, &input.document_id)?;
    record_choice(&transaction, &input.document_id, input.use_images, now)?;
    if !input.use_images {
        // Turning images off forgets what was found, hidden figures included.
        transaction
            .execute(
                "DELETE FROM document_figures WHERE document_id = ?1",
                [&input.document_id],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_figure_scans
                 SET state = 'pending', detector = NULL, scanned_at = NULL,
                     reading = NULL, read_at = NULL
                 WHERE document_id = ?1",
                [&input.document_id],
            )
            .map_err(CommandError::database)?;
    }
    let saved = figures(&transaction, &input.document_id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(saved)
}

pub(crate) fn set_hidden(
    connection: &mut Connection,
    input: SetDocumentFigureHiddenInput,
    now: i64,
) -> Result<DocumentFiguresDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    validate_figure_id(&input.figure_id)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let changed = transaction
        .execute(
            "UPDATE document_figures
             SET hidden = ?3, updated_at = MAX(created_at, ?4)
             WHERE document_id = ?1 AND figure_id = ?2",
            params![
                input.document_id,
                input.figure_id,
                i64::from(input.hidden),
                now.max(0)
            ],
        )
        .map_err(CommandError::database)?;
    if changed == 0 {
        return Err(CommandError::new(
            "document_figure_not_found",
            "That figure is no longer in this document. Open its Figures page again.",
        ));
    }
    let saved = figures(&transaction, &input.document_id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(saved)
}

fn validate_figure_id(figure_id: &str) -> Result<(), CommandError> {
    let valid = (1..=32).contains(&figure_id.len())
        && figure_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(CommandError::new(
            "document_figure_invalid",
            "That figure id is not valid.",
        ))
    }
}

/// Replaces a document's figures with a search's, in one transaction.
pub(crate) fn replace_figures(
    connection: &mut Connection,
    document_id: &str,
    found: &[DetectedFigure],
    now: i64,
) -> Result<(), CommandError> {
    let now = now.max(0);
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let hidden_before = hidden_boxes(&transaction, document_id)?;
    let read_before = read_boxes(&transaction, document_id)?;
    transaction
        .execute(
            "DELETE FROM document_figures WHERE document_id = ?1",
            [document_id],
        )
        .map_err(CommandError::database)?;
    for (ordinal, figure) in found.iter().enumerate() {
        let hidden = hidden_before.iter().any(|(page, bounds)| {
            *page == figure.page_number as i64
                && bounds.overlap(&figure.bounds) >= SAME_FIGURE_OVERLAP
        });
        let caption_bounds = figure.caption.as_ref().map(|caption| caption.bounds);
        transaction
            .execute(
                "INSERT INTO document_figures(
                     document_id, figure_id, ordinal, page_number, page_width, page_height,
                     kind, box_left, box_top, box_width, box_height,
                     caption_left, caption_top, caption_width, caption_height,
                     label, caption, hidden, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                           ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?19)",
                params![
                    document_id,
                    figure.figure_id,
                    ordinal as i64,
                    figure.page_number as i64,
                    figure.page_width,
                    figure.page_height,
                    figure.kind.as_str(),
                    figure.bounds.left,
                    figure.bounds.top,
                    figure.bounds.width,
                    figure.bounds.height,
                    caption_bounds.map(|bounds| bounds.left),
                    caption_bounds.map(|bounds| bounds.top),
                    caption_bounds.map(|bounds| bounds.width),
                    caption_bounds.map(|bounds| bounds.height),
                    figure
                        .caption
                        .as_ref()
                        .map(|caption| caption.label.as_str()),
                    figure
                        .caption
                        .as_ref()
                        .map(|caption| caption.text.as_str())
                        .filter(|text| !text.is_empty()),
                    i64::from(hidden),
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        for (passage_ordinal, passage) in figure.passages.iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO document_figure_passages(
                         document_id, figure_id, ordinal, page_number, kind, excerpt
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        document_id,
                        figure.figure_id,
                        passage_ordinal as i64,
                        passage.page_number as i64,
                        passage.kind.as_str(),
                        passage.excerpt.as_deref().filter(|text| !text.is_empty()),
                    ],
                )
                .map_err(CommandError::database)?;
        }
    }
    // What was read of a figure found again in the same place stays with it.
    let mut unread = 0_usize;
    for figure in found {
        let earlier = read_before.iter().find(|(page, bounds, _)| {
            *page == figure.page_number as i64
                && bounds.overlap(&figure.bounds) >= SAME_FIGURE_OVERLAP
        });
        match earlier {
            Some((_, _, reading)) => {
                insert_reading(&transaction, document_id, &figure.figure_id, reading)?
            }
            None => unread += 1,
        }
    }
    record_state(&transaction, document_id, "found", now)?;
    if unread > 0 {
        transaction
            .execute(
                "UPDATE document_figure_scans SET reading = NULL, read_at = NULL
                 WHERE document_id = ?1",
                [document_id],
            )
            .map_err(CommandError::database)?;
    }
    transaction.commit().map_err(CommandError::database)?;
    Ok(())
}

/// A stored reading as its columns hold it, to carry over to a figure.
struct StoredReading {
    columns: (
        String,
        String,
        Option<String>,
        String,
        String,
        i64,
        i64,
        String,
        String,
        i64,
    ),
}

fn read_boxes(
    connection: &Connection,
    document_id: &str,
) -> Result<Vec<(i64, Rect, StoredReading)>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT figure.page_number, figure.box_left, figure.box_top, figure.box_width,
                    figure.box_height, reading.title, reading.shows, reading.use_in_text,
                    reading.concepts_json, reading.explained_json, reading.importance,
                    reading.worth_studying, reading.provider, reading.model, reading.read_at
             FROM document_figure_readings AS reading
             JOIN document_figures AS figure
               ON figure.document_id = reading.document_id
              AND figure.figure_id = reading.figure_id
             WHERE reading.document_id = ?1",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([document_id], |row| {
            Ok((
                row.get(0)?,
                Rect::from_bounds([row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?]),
                StoredReading {
                    columns: (
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                        row.get(12)?,
                        row.get(13)?,
                        row.get(14)?,
                    ),
                },
            ))
        })
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn insert_reading(
    connection: &Connection,
    document_id: &str,
    figure_id: &str,
    reading: &StoredReading,
) -> Result<(), CommandError> {
    let (title, shows, use_in_text, concepts, explained, importance, worth, provider, model, at) =
        &reading.columns;
    connection
        .execute(
            "INSERT INTO document_figure_readings(
                 document_id, figure_id, title, shows, use_in_text, concepts_json,
                 explained_json, importance, worth_studying, provider, model, read_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                document_id,
                figure_id,
                title,
                shows,
                use_in_text,
                concepts,
                explained,
                importance,
                worth,
                provider,
                model,
                at
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn hidden_boxes(
    connection: &Connection,
    document_id: &str,
) -> Result<Vec<(i64, Rect)>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT page_number, box_left, box_top, box_width, box_height
             FROM document_figures WHERE document_id = ?1 AND hidden = 1",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([document_id], |row| {
            Ok((
                row.get(0)?,
                Rect::from_bounds([row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?]),
            ))
        })
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

pub(crate) fn figures(
    connection: &Connection,
    document_id: &str,
) -> Result<DocumentFiguresDto, CommandError> {
    type Scan = (i64, String, Option<i64>, Option<String>, Option<i64>);
    let scan: Option<Scan> = connection
        .query_row(
            "SELECT use_images, state, scanned_at, reading, read_at
             FROM document_figure_scans WHERE document_id = ?1",
            [document_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let (use_images, state, scanned_at, reading, read_at) = match scan {
        Some((use_images, state, scanned_at, reading, read_at)) => {
            (use_images == 1, state, scanned_at, reading, read_at)
        }
        None => (true, "none".to_string(), None, None, None),
    };
    let mut readings = figure_readings(connection, document_id)?;

    let mut passages_statement = connection
        .prepare(
            "SELECT figure_id, page_number, kind, excerpt FROM document_figure_passages
             WHERE document_id = ?1 ORDER BY figure_id, ordinal",
        )
        .map_err(CommandError::database)?;
    let mut passages = std::collections::HashMap::<String, Vec<FigurePassageDto>>::new();
    let rows = passages_statement
        .query_map([document_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                FigurePassageDto {
                    page_number: row.get(1)?,
                    kind: row.get(2)?,
                    excerpt: row.get(3)?,
                },
            ))
        })
        .map_err(CommandError::database)?;
    for row in rows {
        let (figure_id, passage) = row.map_err(CommandError::database)?;
        passages.entry(figure_id).or_default().push(passage);
    }

    let mut statement = connection
        .prepare(
            "SELECT figure_id, page_number, page_width, page_height, kind,
                    box_left, box_top, box_width, box_height,
                    caption_left, caption_top, caption_width, caption_height,
                    label, caption, hidden
             FROM document_figures WHERE document_id = ?1 ORDER BY ordinal",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([document_id], |row| {
            let caption_bounds = match (row.get(9)?, row.get(10)?, row.get(11)?, row.get(12)?) {
                (Some(left), Some(top), Some(width), Some(height)) => Some(FigureBoxDto {
                    left,
                    top,
                    width,
                    height,
                }),
                _ => None,
            };
            Ok(DocumentFigureDto {
                figure_id: row.get(0)?,
                page_number: row.get(1)?,
                page_width: row.get(2)?,
                page_height: row.get(3)?,
                kind: row.get(4)?,
                bounds: FigureBoxDto {
                    left: row.get(5)?,
                    top: row.get(6)?,
                    width: row.get(7)?,
                    height: row.get(8)?,
                },
                caption_bounds,
                label: row.get(13)?,
                caption: row.get(14)?,
                hidden: row.get::<_, i64>(15)? == 1,
                passages: Vec::new(),
                reading: None,
            })
        })
        .map_err(CommandError::database)?;
    let mut figures = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    for figure in &mut figures {
        figure.passages = passages.remove(&figure.figure_id).unwrap_or_default();
        figure.reading = readings.remove(&figure.figure_id);
    }
    Ok(DocumentFiguresDto {
        document_id: document_id.to_string(),
        use_images,
        state,
        scanned_at,
        reading: reading.unwrap_or_else(|| "none".to_string()),
        read_at,
        figures,
    })
}

fn figure_readings(
    connection: &Connection,
    document_id: &str,
) -> Result<std::collections::HashMap<String, FigureReadingDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT figure_id, title, shows, use_in_text, concepts_json, explained_json,
                    importance, worth_studying, model
             FROM document_figure_readings WHERE document_id = ?1",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([document_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
            ))
        })
        .map_err(CommandError::database)?;
    let mut readings = std::collections::HashMap::new();
    for row in rows {
        let (figure_id, title, shows, use_in_text, concepts, explained, importance, worth, model) =
            row.map_err(CommandError::database)?;
        readings.insert(
            figure_id,
            FigureReadingDto {
                title,
                shows,
                use_in_text,
                concepts: serde_json::from_str(&concepts).unwrap_or_default(),
                explained_on: serde_json::from_str(&explained).unwrap_or_default(),
                importance,
                worth_studying: worth == 1,
                model,
            },
        );
    }
    Ok(readings)
}
