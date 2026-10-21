//! Reading a document's figures with the learner's model.
//!
//! Each request covers the figures on one page: the model sees the page whole,
//! a sharp crop of each figure and, for a figure that fills its page, the page
//! beside it, with the page's text and the passages elsewhere that name the
//! figures. Pages are read in parallel within the provider's bounds. A page
//! whose reply does not fit the contract is skipped; its figures keep their
//! captions and passages. Hidden figures are not read.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{
    ai::{
        concurrency::{document_request_parallelism, run_bounded_provider_queue, QueueControl},
        document_intelligence::{
            self, is_output_failure, DocumentTaskContext, FigurePage, FigureReading, FigureToRead,
            NearbyText, SourcePageImage,
        },
        AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::{self, PageTile, PdfRenderRuntime, MAX_TILED_PAGE_SIDE, MAX_TILE_SIDE},
    task_cancellation::CancellationSignal,
    util::unix_time_millis,
};

use super::{DocumentFigureDto, FigureBoxDto};

/// Figures read in one request: with the page and one neighbour, a request
/// stays within the gateway's six images.
const FIGURES_PER_REQUEST: usize = 4;
/// Crops are drawn at about this resolution, within one tile.
const CROP_DPI: f64 = 200.0;
/// Points of page kept around a crop so labels at its edge are not cut.
const CROP_MARGIN: f64 = 4.0;

/// Everything one reading pass needs.
pub(crate) struct FigureReadingRun<'a> {
    pub database: &'a Database,
    pub paths: &'a AppPaths,
    pub render: &'a PdfRenderRuntime,
    pub runtime: &'a AiRuntimeState,
    pub task: &'a DocumentTaskContext,
    pub document_id: &'a str,
    pub cancellation: &'a CancellationSignal,
}

/// One request: the figures on one page and the page shown beside them.
#[derive(Debug, PartialEq)]
pub(crate) struct ReadingWindow {
    pub page_number: i64,
    pub figures: Vec<usize>,
    pub neighbour: Option<i64>,
}

/// Reads the document's visible figures and stores what the model read. The
/// pass records `unavailable` when the model cannot see images, `failed` when
/// no page could be read, and `partial` when some could not.
pub(crate) async fn read_figures(run: FigureReadingRun<'_>) -> Result<(), CommandError> {
    let (document, figures) = run.database.with_connection(|connection| {
        let document = document_for_reading(connection, run.document_id)?;
        let figures = super::figures(connection, run.document_id)?
            .figures
            .into_iter()
            .filter(|figure| !figure.hidden)
            .collect::<Vec<_>>();
        Ok((document, figures))
    })?;
    if !run.task.include_page_images {
        return record_reading(run.database, run.document_id, "unavailable");
    }
    let windows = reading_windows(&figures);
    if windows.is_empty() {
        return record_reading(run.database, run.document_id, "read");
    }
    let source_path = run.paths.resolve_relative(&document.relative_path);
    let page_count = document.page_count.max(
        figures
            .iter()
            .map(|figure| figure.page_number)
            .max()
            .unwrap_or(1),
    );

    let parallelism = document_request_parallelism(&run.task.network_scope).clamp(1, windows.len());
    let mut readings = Vec::<FigureReading>::new();
    let mut failed_windows = 0_usize;
    let mut fatal: Option<CommandError> = None;
    let windows = &windows;
    let figures = &figures;
    let source_path = &source_path;
    let title = document.title.as_str();
    let run = &run;
    run_bounded_provider_queue(
        windows.len(),
        parallelism,
        |index| async move {
            let read = read_window(
                run,
                title,
                page_count,
                source_path,
                &windows[index],
                figures,
            )
            .await;
            (windows[index].page_number, read)
        },
        |(page_number, result)| {
            Ok::<_, CommandError>(match result {
                Ok(read) => {
                    readings.extend(read);
                    QueueControl::Continue
                }
                Err(error)
                    if is_output_failure(&error)
                        || error.code == "document_figure_render_failed" =>
                {
                    failed_windows += 1;
                    log_page_skipped(run.document_id, page_number, &error);
                    QueueControl::Continue
                }
                Err(error) => {
                    failed_windows += 1;
                    fatal.get_or_insert(error);
                    QueueControl::Stop
                }
            })
        },
    )
    .await?;

    let state = if failed_windows == 0 {
        "read"
    } else if readings.is_empty() {
        "failed"
    } else {
        "partial"
    };
    let now = unix_time_millis()?;
    run.database.with_connection(|connection| {
        store_readings(
            connection,
            run.document_id,
            &readings,
            &run.task.provider_kind,
            &run.task.model_id,
            state,
            now,
        )
    })?;
    if let Some(error) = fatal {
        log_failure(run.document_id, &error.message);
    }
    Ok(())
}

struct DocumentForReading {
    title: String,
    relative_path: String,
    page_count: i64,
}

fn document_for_reading(
    connection: &Connection,
    document_id: &str,
) -> Result<DocumentForReading, CommandError> {
    let relative_path = super::require_pdf(connection, document_id)?;
    let (title, page_count) = connection
        .query_row(
            "SELECT title, MAX(page_count, source_unit_count) FROM documents WHERE id = ?1",
            [document_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(CommandError::database)?;
    Ok(DocumentForReading {
        title,
        relative_path,
        page_count,
    })
}

/// The requests for a document's figures: each page's figures in order, four
/// at a time, with the first page named beside them as their neighbour.
pub(crate) fn reading_windows(figures: &[DocumentFigureDto]) -> Vec<ReadingWindow> {
    let mut windows = Vec::<ReadingWindow>::new();
    for (index, figure) in figures.iter().enumerate() {
        let open = windows.last_mut().filter(|window| {
            window.page_number == figure.page_number && window.figures.len() < FIGURES_PER_REQUEST
        });
        let neighbour = figure
            .passages
            .iter()
            .find(|passage| {
                passage.kind == "neighbour" && passage.page_number != figure.page_number
            })
            .map(|passage| passage.page_number);
        match open {
            Some(window) => {
                window.figures.push(index);
                window.neighbour = window.neighbour.or(neighbour);
            }
            None => windows.push(ReadingWindow {
                page_number: figure.page_number,
                figures: vec![index],
                neighbour,
            }),
        }
    }
    windows
}

async fn read_window(
    run: &FigureReadingRun<'_>,
    title: &str,
    page_count: i64,
    source_path: &Path,
    window: &ReadingWindow,
    figures: &[DocumentFigureDto],
) -> Result<Vec<FigureReading>, CommandError> {
    let on_page = window
        .figures
        .iter()
        .map(|index| &figures[*index])
        .collect::<Vec<_>>();
    let prepared = prepare_window(
        run.render.clone(),
        source_path.to_path_buf(),
        window.page_number,
        window.neighbour,
        on_page
            .iter()
            .map(|figure| {
                (
                    figure.figure_id.clone(),
                    figure.page_width,
                    figure.page_height,
                    figure.bounds,
                )
            })
            .collect(),
    )
    .await?;

    let to_read = on_page
        .iter()
        .map(|figure| FigureToRead {
            figure_id: &figure.figure_id,
            label: figure.label.as_deref(),
            caption: figure.caption.as_deref(),
            place: [
                figure.bounds.left / figure.page_width,
                figure.bounds.top / figure.page_height,
                figure.bounds.width / figure.page_width,
                figure.bounds.height / figure.page_height,
            ],
        })
        .collect::<Vec<_>>();
    let mut nearby = Vec::new();
    if let Some((page_number, text)) = &prepared.neighbour_text {
        nearby.push(NearbyText {
            page_number: *page_number,
            text,
        });
    }
    for figure in &on_page {
        for passage in &figure.passages {
            if let (Some(excerpt), true) = (
                passage.excerpt.as_deref(),
                passage.page_number != window.page_number,
            ) {
                nearby.push(NearbyText {
                    page_number: passage.page_number,
                    text: excerpt,
                });
            }
        }
    }
    document_intelligence::request_figure_readings(
        run.runtime,
        run.task,
        &FigurePage {
            document_title: title,
            page_count,
            page_number: window.page_number,
            page_text: &prepared.page_text,
            figures: &to_read,
            nearby: &nearby,
            images: &prepared.images,
        },
        run.cancellation,
    )
    .await
}

struct PreparedWindow {
    images: Vec<SourcePageImage>,
    page_text: String,
    neighbour_text: Option<(i64, String)>,
}

type CropRequest = (String, f64, f64, FigureBoxDto);

/// Draws the page, each figure's crop and the neighbour, and reads their text.
async fn prepare_window(
    render: PdfRenderRuntime,
    source_path: PathBuf,
    page_number: i64,
    neighbour: Option<i64>,
    crops: Vec<CropRequest>,
) -> Result<PreparedWindow, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let render_error = |error: pdf_render_backend::PdfRenderError| {
            CommandError::new(
                "document_figure_render_failed",
                format!("A figure's page could not be drawn for reading: {error}"),
            )
        };
        let page = usize::try_from(page_number).unwrap_or(1);
        let (page_width, page_height) = crops
            .first()
            .map(|(_, width, height, _)| (*width, *height))
            .unwrap_or((612.0, 792.0));
        let mut images = vec![SourcePageImage {
            page_number,
            bytes: pdf_render_backend::render_page_tile(
                &render,
                &source_path,
                page,
                whole_page_tile(page_width, page_height),
            )
            .map_err(render_error)?,
            crop_of: None,
        }];
        for (figure_id, width, height, bounds) in &crops {
            images.push(SourcePageImage {
                page_number,
                bytes: pdf_render_backend::render_page_tile(
                    &render,
                    &source_path,
                    page,
                    crop_tile(*width, *height, *bounds),
                )
                .map_err(render_error)?,
                crop_of: Some(figure_id.clone()),
            });
        }
        let mut wanted = vec![page];
        if let Some(neighbour) = neighbour.and_then(|page| usize::try_from(page).ok()) {
            images.push(SourcePageImage {
                page_number: neighbour as i64,
                bytes: pdf_render_backend::render_page_tile(
                    &render,
                    &source_path,
                    neighbour,
                    whole_page_tile(page_width, page_height),
                )
                .map_err(render_error)?,
                crop_of: None,
            });
            wanted.push(neighbour);
        }
        let texts = pdf_render_backend::extract_native_text_pages(&render, &source_path, &wanted)
            .map_err(render_error)?;
        let text_of = |wanted: usize| {
            texts
                .iter()
                .find(|(number, _)| *number == wanted)
                .map(|(_, text)| text.clone())
                .unwrap_or_default()
        };
        Ok(PreparedWindow {
            page_text: text_of(page),
            neighbour_text: neighbour.map(|neighbour| {
                (
                    neighbour,
                    text_of(usize::try_from(neighbour).unwrap_or_default()),
                )
            }),
            images,
        })
    })
    .await
    .map_err(|error| {
        CommandError::new(
            "document_figure_render_failed",
            format!("Drawing a figure's page stopped unexpectedly: {error}"),
        )
    })?
}

/// The whole page as one tile, its longest side one tile long.
pub(crate) fn whole_page_tile(page_width: f64, page_height: f64) -> PageTile {
    let scale = f64::from(MAX_TILE_SIDE) / page_width.max(page_height).max(1.0);
    let width = ((page_width * scale).round() as u32).clamp(1, MAX_TILE_SIDE);
    let height = ((page_height * scale).round() as u32).clamp(1, MAX_TILE_SIDE);
    PageTile {
        page_width: width,
        page_height: height,
        left: 0,
        top: 0,
        width,
        height,
    }
}

/// A figure and a small margin at about 200 dpi, shrunk to fit one tile.
pub(crate) fn crop_tile(page_width: f64, page_height: f64, bounds: FigureBoxDto) -> PageTile {
    let left = (bounds.left - CROP_MARGIN).max(0.0);
    let top = (bounds.top - CROP_MARGIN).max(0.0);
    let right = (bounds.left + bounds.width + CROP_MARGIN).min(page_width);
    let bottom = (bounds.top + bounds.height + CROP_MARGIN).min(page_height);
    let longest = (right - left).max(bottom - top).max(1.0);
    let scale = (CROP_DPI / 72.0)
        .min(f64::from(MAX_TILE_SIDE) / longest)
        .min(f64::from(MAX_TILED_PAGE_SIDE) / page_width.max(page_height).max(1.0));
    let page_pixels_wide = ((page_width * scale).floor() as u32).clamp(1, MAX_TILED_PAGE_SIDE);
    let page_pixels_high = ((page_height * scale).floor() as u32).clamp(1, MAX_TILED_PAGE_SIDE);
    let tile_left = ((left * scale).floor() as u32).min(page_pixels_wide - 1);
    let tile_top = ((top * scale).floor() as u32).min(page_pixels_high - 1);
    let tile_width = ((right * scale).ceil() as u32)
        .min(page_pixels_wide)
        .saturating_sub(tile_left)
        .clamp(1, MAX_TILE_SIDE);
    let tile_height = ((bottom * scale).ceil() as u32)
        .min(page_pixels_high)
        .saturating_sub(tile_top)
        .clamp(1, MAX_TILE_SIDE);
    PageTile {
        page_width: page_pixels_wide,
        page_height: page_pixels_high,
        left: tile_left,
        top: tile_top,
        width: tile_width.min(page_pixels_wide - tile_left),
        height: tile_height.min(page_pixels_high - tile_top),
    }
}

/// Stores a pass's readings over the earlier ones, for figures still there.
pub(crate) fn store_readings(
    connection: &mut Connection,
    document_id: &str,
    readings: &[FigureReading],
    provider: &str,
    model: &str,
    state: &str,
    now: i64,
) -> Result<(), CommandError> {
    let now = now.max(0);
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    for reading in readings {
        let concepts =
            serde_json::to_string(&reading.concepts).map_err(CommandError::serialization)?;
        let explained =
            serde_json::to_string(&reading.explained_on).map_err(CommandError::serialization)?;
        transaction
            .execute(
                "INSERT INTO document_figure_readings(
                     document_id, figure_id, title, shows, use_in_text, concepts_json,
                     explained_json, importance, worth_studying, provider, model, read_at
                 )
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12
                 WHERE EXISTS (
                     SELECT 1 FROM document_figures WHERE document_id = ?1 AND figure_id = ?2
                 )
                 ON CONFLICT(document_id, figure_id) DO UPDATE SET
                     title = excluded.title,
                     shows = excluded.shows,
                     use_in_text = excluded.use_in_text,
                     concepts_json = excluded.concepts_json,
                     explained_json = excluded.explained_json,
                     importance = excluded.importance,
                     worth_studying = excluded.worth_studying,
                     provider = excluded.provider,
                     model = excluded.model,
                     read_at = excluded.read_at",
                params![
                    document_id,
                    reading.figure_id,
                    reading.title,
                    reading.shows,
                    reading.use_in_text,
                    concepts,
                    explained,
                    reading.importance,
                    i64::from(reading.worth_studying),
                    provider,
                    model,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
    }
    record_reading_state(&transaction, document_id, state, now)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(())
}

fn record_reading(database: &Database, document_id: &str, state: &str) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| record_reading_state(connection, document_id, state, now))
}

pub(crate) fn record_reading_state(
    connection: &Connection,
    document_id: &str,
    state: &str,
    now: i64,
) -> Result<(), CommandError> {
    connection
        .execute(
            "UPDATE document_figure_scans
             SET reading = ?2, read_at = ?3, updated_at = MAX(updated_at, ?3)
             WHERE document_id = ?1",
            params![document_id, state, now.max(0)],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

/// Whether a document's figures were found and are waiting to be read.
pub(crate) fn waiting_to_be_read(
    connection: &Connection,
    document_id: &str,
) -> Result<bool, CommandError> {
    let waiting: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM document_figure_scans
             WHERE document_id = ?1 AND use_images = 1 AND state = 'found' AND reading IS NULL",
            [document_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    Ok(waiting.is_some())
}

fn log_page_skipped(document_id: &str, page_number: i64, error: &CommandError) {
    crate::logging::warn_target(
        "documents.figures",
        "document_figure_page_skipped",
        "The figures on one page could not be read; they keep their captions.",
        serde_json::json!({
            "document": document_id,
            "page": page_number,
            "code": error.code,
            "detail": error.message.chars().take(300).collect::<String>(),
        }),
    );
}

fn log_failure(document_id: &str, detail: &str) {
    crate::logging::warn_target(
        "documents.figures",
        "document_figure_reading_failed",
        "The figures of a document could not all be read; they keep their captions.",
        serde_json::json!({
            "document": document_id,
            "detail": detail.chars().take(300).collect::<String>(),
        }),
    );
}
