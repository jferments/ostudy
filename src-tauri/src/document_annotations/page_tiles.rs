//! Sharp page tiles for a zoomed-in annotator.
//!
//! The page renders made on import are 120 dpi, which is soft past 125% or on
//! a high-density screen. When the learner zooms in, the annotator asks for
//! the visible parts of a page drawn again from the stored PDF at the size it
//! now shows, a tile at a time, and lays them over the page render. Tiles are
//! not stored: they are a view of the PDF, never a second copy of it.

use rusqlite::OptionalExtension;
use serde::Deserialize;
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::{self, PageTile, PdfRenderRuntime},
    util::validate_entity_id,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderDocumentPageTileInput {
    pub document_id: String,
    pub page_number: i64,
    /// The whole page's size at this zoom, in pixels.
    pub page_width: u32,
    pub page_height: u32,
    /// The tile, in pixels from the page's top-left corner at that size.
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

/// One tile of one PDF page as a PNG.
pub(crate) async fn render_document_page_tile(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PdfRenderRuntime>,
    input: RenderDocumentPageTileInput,
) -> Result<tauri::ipc::Response, CommandError> {
    let (page_number, tile) = validated_request(&input)?;
    let relative_path =
        database.with_connection(|connection| pdf_source_path(connection, &input.document_id))?;
    let source_path = paths.resolve_relative(&relative_path);
    let runtime = runtime.inner().clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        pdf_render_backend::render_page_tile(&runtime, &source_path, page_number, tile)
    })
    .await
    .map_err(|_| tile_failed())?
    .map_err(|error| {
        crate::logging::warn_target(
            "documents.annotations",
            "document_page_tile_failed",
            "A sharp page tile could not be drawn; the page render stays in place.",
            serde_json::json!({ "pageNumber": page_number, "error": error.to_string() }),
        );
        tile_failed()
    })?;
    Ok(tauri::ipc::Response::new(bytes))
}

pub(crate) fn validated_request(
    input: &RenderDocumentPageTileInput,
) -> Result<(usize, PageTile), CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    let page_number = usize::try_from(input.page_number)
        .ok()
        .filter(|page| *page >= 1)
        .ok_or_else(super::page_missing)?;
    let tile = PageTile {
        page_width: input.page_width,
        page_height: input.page_height,
        left: input.left,
        top: input.top,
        width: input.width,
        height: input.height,
    };
    if !tile.is_valid() {
        return Err(super::invalid(
            "That part of the page is outside the page or larger than oStudy draws at once.",
        ));
    }
    Ok((page_number, tile))
}

/// The stored PDF behind a document; other formats have no pages to draw.
pub(crate) fn pdf_source_path(
    connection: &rusqlite::Connection,
    document_id: &str,
) -> Result<String, CommandError> {
    let row: Option<(String, String)> = connection
        .query_row(
            "SELECT source_relative_path, source_format FROM documents WHERE id = ?1",
            [document_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    match row {
        Some((relative_path, format)) if format == "pdf" => Ok(relative_path),
        Some(_) => Err(super::invalid("Only PDF pages can be drawn sharper.")),
        None => Err(CommandError::new(
            "document_not_found",
            "That document is no longer in the library.",
        )),
    }
}

fn tile_failed() -> CommandError {
    CommandError::new(
        "document_page_tile_failed",
        "This part of the page could not be drawn sharper.",
    )
}
