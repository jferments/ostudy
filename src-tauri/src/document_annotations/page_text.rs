//! A PDF page's own text, positioned, so highlights can follow it.
//!
//! The highlighter snaps to whole characters in reading order when a drag
//! starts on text. Positions come from the stored PDF when asked for and are
//! not kept: the PDF stays the one source of its text.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::{self, PdfRenderRuntime},
    util::validate_entity_id,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetDocumentPageTextInput {
    pub document_id: String,
    pub page_number: i64,
}

/// A page's characters in reading order. `characters` holds one Unicode
/// character per entry of `boxes`, and `boxes` holds `x, y, width, height`
/// for each in points from the page's top-left corner as displayed, in a
/// page `width` by `height` points; an all-zero box marks a space or line
/// break the PDF only implies.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPageTextDto {
    pub page_number: i64,
    pub width: f64,
    pub height: f64,
    pub characters: String,
    pub boxes: Vec<f64>,
}

pub(crate) async fn get_document_page_text(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PdfRenderRuntime>,
    input: GetDocumentPageTextInput,
) -> Result<DocumentPageTextDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    let page_number = usize::try_from(input.page_number)
        .ok()
        .filter(|page| *page >= 1)
        .ok_or_else(super::page_missing)?;
    let relative_path = database.with_connection(|connection| {
        super::page_tiles::pdf_source_path(connection, &input.document_id)
    })?;
    let source_path = paths.resolve_relative(&relative_path);
    let runtime = runtime.inner().clone();
    let page = tauri::async_runtime::spawn_blocking(move || {
        pdf_render_backend::page_characters(&runtime, &source_path, page_number)
    })
    .await
    .map_err(|_| text_unavailable())?
    .map_err(|error| {
        crate::logging::warn_target(
            "documents.annotations",
            "document_page_text_unavailable",
            "A page's text positions could not be read; highlights stay free strokes there.",
            serde_json::json!({ "pageNumber": page_number, "error": error.to_string() }),
        );
        text_unavailable()
    })?;
    Ok(to_dto(input.page_number, page))
}

pub(crate) fn to_dto(
    page_number: i64,
    page: pdf_render_backend::PageCharacters,
) -> DocumentPageTextDto {
    let mut characters = String::with_capacity(page.characters.len());
    let mut boxes = Vec::with_capacity(page.characters.len() * 4);
    for character in page.characters {
        characters.push(character.text);
        let [x, y, width, height] = character.bounds.unwrap_or([0.0; 4]);
        boxes.extend([round(x), round(y), round(width), round(height)]);
    }
    DocumentPageTextDto {
        page_number,
        width: page.width,
        height: page.height,
        characters,
        boxes,
    }
}

/// Hundredths of a point are finer than any screen shows and keep replies small.
fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn text_unavailable() -> CommandError {
    CommandError::new(
        "document_page_text_unavailable",
        "This page's text could not be read.",
    )
}
