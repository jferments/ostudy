//! Clips: a region of a PDF page drawn as a picture for a notebook.
//!
//! The annotator's Clip tool names a rectangle on a page in PDF points. The
//! region is drawn again from the stored PDF, sharper than the page render,
//! and staged as a PNG in the notebook's image draft; the notebook then places
//! it like any other picture, naming the page it came from. That page lists
//! the notebooks its clips are in.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    asset_lifecycle::stage_generated_png,
    assets::AssetDto,
    database::Database,
    document_annotations::{pdf_source_path, AnnotationShape},
    error::CommandError,
    notebooks::NotebookItemDto,
    paths::AppPaths,
    pdf_render_backend::{self, PageTile, PdfRenderRuntime, MAX_TILED_PAGE_SIDE, MAX_TILE_SIDE},
    util::validate_entity_id,
};

/// Clips draw at up to three pixels per point (216 dpi), and never wider or
/// taller than one page tile.
const MAX_PIXELS_PER_POINT: f64 = 3.0;
/// A clip smaller than this, in points, is taken to be a slip of the pointer.
const MIN_CLIP_SIDE: f64 = 4.0;
/// Page sizes the annotator lays pages out at, in points.
const MIN_PAGE_SIDE: f64 = 36.0;
const MAX_PAGE_SIDE: f64 = 14_400.0;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StageDocumentClipInput {
    /// The notebook's image draft the clip is staged in.
    pub draft_id: String,
    pub document_id: String,
    pub page_number: i64,
    /// The page's size in points, as the annotator lays it out.
    pub page_width: f64,
    pub page_height: f64,
    /// The region, in points from the page's top-left corner.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A clip staged for a notebook, and its size in points on its page.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedDocumentClipDto {
    pub asset: AssetDto,
    pub width: f64,
    pub height: f64,
}

/// A notebook holding a clip of one page of a document.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentClipDto {
    pub page_number: i64,
    pub notebook_id: String,
    pub notebook_title: String,
    /// The notebook page the clip is on.
    pub notebook_page_id: String,
}

pub(crate) async fn stage_document_clip(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PdfRenderRuntime>,
    input: StageDocumentClipInput,
) -> Result<StagedDocumentClipDto, CommandError> {
    validate_entity_id(&input.draft_id, "asset draft")?;
    validate_entity_id(&input.document_id, "document")?;
    let (page_number, tile) = clip_tile(&input)?;
    let relative_path =
        database.with_connection(|connection| pdf_source_path(connection, &input.document_id))?;
    let source_path = paths.resolve_relative(&relative_path);
    let runtime = runtime.inner().clone();
    let png = tauri::async_runtime::spawn_blocking(move || {
        pdf_render_backend::render_page_tile(&runtime, &source_path, page_number, tile)
    })
    .await
    .map_err(|_| clip_failed())?
    .map_err(|error| {
        crate::logging::warn_target(
            "notebooks.clips",
            "document_clip_failed",
            "A clip of a document page could not be drawn.",
            serde_json::json!({ "pageNumber": page_number, "error": error.to_string() }),
        );
        clip_failed()
    })?;
    let name = format!("Page {} clip.png", input.page_number);
    let asset = stage_generated_png(&database, &paths, &input.draft_id, &png, &name)?;
    Ok(StagedDocumentClipDto {
        asset,
        width: input.width,
        height: input.height,
    })
}

/// The page tile that draws the region: the whole page at the clip's scale,
/// and the region within it, both in pixels.
pub(crate) fn clip_tile(input: &StageDocumentClipInput) -> Result<(usize, PageTile), CommandError> {
    let page_number = usize::try_from(input.page_number)
        .ok()
        .filter(|page| *page >= 1)
        .ok_or_else(|| invalid("A clip names the page it is taken from."))?;
    let page_side = MIN_PAGE_SIDE..=MAX_PAGE_SIDE;
    if !page_side.contains(&input.page_width) || !page_side.contains(&input.page_height) {
        return Err(invalid(
            "That page size is not one oStudy lays pages out at.",
        ));
    }
    let finite = [input.x, input.y, input.width, input.height]
        .iter()
        .all(|value| value.is_finite());
    if !finite || input.width < MIN_CLIP_SIDE || input.height < MIN_CLIP_SIDE {
        return Err(invalid("Drag across the part of the page to clip."));
    }
    let slack = 0.5;
    if input.x < -slack
        || input.y < -slack
        || input.x + input.width > input.page_width + slack
        || input.y + input.height > input.page_height + slack
    {
        return Err(invalid("A clip lies within its page."));
    }
    let scale = (f64::from(MAX_TILE_SIDE) / input.width.max(input.height))
        .min(f64::from(MAX_TILED_PAGE_SIDE) / input.page_width.max(input.page_height))
        .min(MAX_PIXELS_PER_POINT);
    let page_width = (input.page_width * scale).round().max(1.0) as u32;
    let page_height = (input.page_height * scale).round().max(1.0) as u32;
    let left = (input.x.max(0.0) * scale).floor() as u32;
    let top = (input.y.max(0.0) * scale).floor() as u32;
    let width = ((input.width * scale).round() as u32)
        .clamp(1, MAX_TILE_SIDE)
        .min(page_width.saturating_sub(left).max(1));
    let height = ((input.height * scale).round() as u32)
        .clamp(1, MAX_TILE_SIDE)
        .min(page_height.saturating_sub(top).max(1));
    let tile = PageTile {
        page_width,
        page_height,
        left: left.min(page_width - 1),
        top: top.min(page_height - 1),
        width,
        height,
    };
    if !tile.is_valid() {
        return Err(invalid("A clip lies within its page."));
    }
    Ok((page_number, tile))
}

#[tauri::command]
pub fn list_document_clips(
    database: State<'_, Database>,
    document_id: String,
) -> Result<Vec<DocumentClipDto>, CommandError> {
    validate_entity_id(&document_id, "document")?;
    database.with_connection(|connection| clips_for_document(connection, &document_id))
}

/// Each page of the document with a clip in a notebook, once per notebook
/// page, in page order and then by notebook title.
pub(crate) fn clips_for_document(
    connection: &Connection,
    document_id: &str,
) -> Result<Vec<DocumentClipDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT json_extract(i.payload_json, '$.source.pageNumber') AS page_number,
                    n.id, n.title, p.id, p.position
             FROM notebook_items i
             JOIN notebook_pages p ON p.id = i.page_id
             JOIN notebooks n ON n.id = p.notebook_id
             WHERE i.kind = 'image'
               AND json_extract(i.payload_json, '$.source.documentId') = ?1
             ORDER BY page_number, n.title COLLATE NOCASE, n.id, p.position",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![document_id], |row| {
            Ok(DocumentClipDto {
                page_number: row.get(0)?,
                notebook_id: row.get(1)?,
                notebook_title: row.get(2)?,
                notebook_page_id: row.get(3)?,
            })
        })
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

/// The titles of the documents that clips among `items` came from; a
/// document that is gone is left out.
pub(crate) fn clip_document_titles(
    connection: &Connection,
    items: &[NotebookItemDto],
) -> Result<BTreeMap<String, String>, CommandError> {
    let mut titles = BTreeMap::new();
    let mut looked_up = std::collections::BTreeSet::new();
    for item in items {
        let AnnotationShape::Image(image) = &item.shape else {
            continue;
        };
        let Some(source) = &image.source else {
            continue;
        };
        if !looked_up.insert(source.document_id.clone()) {
            continue;
        }
        let title: Option<String> = connection
            .query_row(
                "SELECT title FROM documents WHERE id = ?1",
                [&source.document_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        if let Some(title) = title {
            titles.insert(source.document_id.clone(), title);
        }
    }
    Ok(titles)
}

fn invalid(message: &str) -> CommandError {
    CommandError::new("invalid_document_clip", message)
}

fn clip_failed() -> CommandError {
    CommandError::new(
        "document_clip_failed",
        "This part of the page could not be clipped. Try again.",
    )
}
