//! Saving a notebook as an ordinary PDF.
//!
//! The notebook's pages are written out with their paper, marks and
//! pictures (see `document_annotations::notebook_pdf`); links to pages of
//! the same notebook become PDF links. The file is a prepared export, which
//! the learner then saves or shares like any other.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::{contents, NotebookContentsDto, Paper};
use crate::{
    database::Database,
    document_annotations::{notebook_pdf, AnnotationShape, LinkTarget, NotebookPdfPage, Ruling},
    error::CommandError,
    external_files::{PreparedExportDto, RegisterPreparedExport},
    paths::AppPaths,
    util::validate_entity_id,
};

const EXPORT_PURPOSE: &str = "notebook_pdf";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareNotebookPdfInput {
    pub notebook_id: String,
    pub operation_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct NotebookPdfSummary {
    page_count: i64,
    /// Pictures whose image could not be read, and so were left out.
    missing_picture_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedNotebookPdfDto {
    pub prepared_export: PreparedExportDto,
    pub page_count: i64,
    pub missing_picture_count: i64,
}

/// Prepares a PDF of a notebook. Repeating an operation returns the file it
/// already prepared.
pub(crate) async fn prepare_notebook_pdf(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareNotebookPdfInput,
) -> Result<PreparedNotebookPdfDto, CommandError> {
    validate_entity_id(&input.notebook_id, "notebook")?;
    validate_entity_id(&input.operation_id, "notebook PDF export operation")?;

    if let Some((prepared_export, summary)) = database.with_connection(|connection| {
        crate::external_files::load_prepared_export_summary::<NotebookPdfSummary>(
            connection,
            paths.inner(),
            &input.operation_id,
            EXPORT_PURPOSE,
        )
    })? {
        return Ok(PreparedNotebookPdfDto {
            prepared_export,
            page_count: summary.page_count,
            missing_picture_count: summary.missing_picture_count,
        });
    }

    let (notebook, files) = database.with_connection(|connection| {
        let notebook = contents(connection, &input.notebook_id)?;
        let files = picture_files(connection, &notebook)?;
        Ok((notebook, files))
    })?;
    let files: Vec<(String, std::path::PathBuf)> = files
        .into_iter()
        .map(|(asset_id, relative)| (asset_id, paths.resolve_relative(&relative)))
        .collect();
    let relative = AppPaths::prepared_export_relative_path(&input.operation_id, "pdf");
    let destination = paths.resolve_relative(&relative);
    let title = notebook.notebook.title.clone();
    let (page_count, missing_picture_count) = tauri::async_runtime::spawn_blocking(move || {
        write_notebook(&notebook, &files, &destination)
    })
    .await
    .map_err(|_| export_failed())??;

    let summary = NotebookPdfSummary {
        page_count,
        missing_picture_count,
    };
    let suggested_filename = format!(
        "{}.pdf",
        crate::document_study_guide_export::portable_filename_or(&title, "notes")
    );
    let prepared_export = database.with_connection(|connection| {
        crate::external_files::register_prepared_export(
            connection,
            paths.inner(),
            RegisterPreparedExport {
                id: &input.operation_id,
                purpose: EXPORT_PURPOSE,
                suggested_filename: &suggested_filename,
                media_type: "application/pdf",
                relative_path: &relative,
                summary: &summary,
            },
        )
    })?;
    Ok(PreparedNotebookPdfDto {
        prepared_export,
        page_count,
        missing_picture_count,
    })
}

/// Where each picture's image is kept, by asset id; images that are gone are
/// left out.
pub(super) fn picture_files(
    connection: &Connection,
    notebook: &NotebookContentsDto,
) -> Result<Vec<(String, String)>, CommandError> {
    let mut files: Vec<(String, String)> = Vec::new();
    for item in &notebook.items {
        let AnnotationShape::Image(picture) = &item.shape else {
            continue;
        };
        if files
            .iter()
            .any(|(asset_id, _)| asset_id == &picture.asset_id)
        {
            continue;
        }
        let relative: Option<String> = connection
            .query_row(
                "SELECT relative_path FROM assets
                 WHERE id = ?1 AND lifecycle_state = 'ready' AND deleted_at IS NULL",
                [&picture.asset_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        if let Some(relative) = relative {
            files.push((picture.asset_id.clone(), relative));
        }
    }
    Ok(files)
}

/// Writes the notebook's PDF and returns its page count and how many
/// pictures were left out.
pub(crate) fn write_notebook(
    notebook: &NotebookContentsDto,
    files: &[(String, std::path::PathBuf)],
    destination: &std::path::Path,
) -> Result<(i64, i64), CommandError> {
    let mut pictures = HashMap::new();
    for (asset_id, path) in files {
        if let Some(image) = decode_picture(path) {
            pictures.insert(asset_id.clone(), image);
        }
    }
    let index_of: HashMap<&str, usize> = notebook
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| (page.id.as_str(), index))
        .collect();
    let mut missing = 0_i64;
    let pages: Vec<NotebookPdfPage<'_>> = notebook
        .pages
        .iter()
        .map(|page| {
            let shapes: Vec<&AnnotationShape> = notebook
                .items
                .iter()
                .filter(|item| item.page_id == page.id)
                .map(|item| &item.shape)
                .collect();
            let links = shapes
                .iter()
                .enumerate()
                .filter_map(|(index, shape)| match shape {
                    AnnotationShape::Text(note) => match note.link.as_ref().map(|link| &link.target) {
                        Some(LinkTarget::NotebookPage {
                            notebook_id,
                            page_id,
                        }) if *notebook_id == notebook.notebook.id => {
                            index_of.get(page_id.as_str()).map(|target| (index, *target))
                        }
                        _ => None,
                    },
                    _ => None,
                })
                .collect();
            missing += shapes
                .iter()
                .filter(|shape| {
                    matches!(shape, AnnotationShape::Image(picture) if !pictures.contains_key(&picture.asset_id))
                })
                .count() as i64;
            NotebookPdfPage {
                width: page.width,
                height: page.height,
                ruling: ruling(page.paper),
                shapes,
                links,
            }
        })
        .collect();
    let font = notebook
        .items
        .iter()
        .any(|item| matches!(item.shape, AnnotationShape::Text(_)))
        .then(|| crate::document_study_guide_export::plain_sans_font().to_vec());
    let output =
        notebook_pdf(&pages, &pictures, font, &notebook.notebook.title).map_err(|error| {
            crate::logging::warn_target(
                "notebooks.pdf",
                "notebook_pdf_failed",
                "A notebook could not be saved as a PDF.",
                serde_json::json!({ "error": error.to_string() }),
            );
            export_failed()
        })?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    std::fs::write(destination, output).map_err(CommandError::io)?;
    Ok((i64::try_from(pages.len()).unwrap_or(i64::MAX), missing))
}

/// A picture's image for the PDF: a JPEG as it is, anything else decoded,
/// keeping its transparency. None when it cannot be read.
fn decode_picture(path: &std::path::Path) -> Option<oxidize_pdf::graphics::Image> {
    let bytes = std::fs::read(path).ok()?;
    if image::guess_format(&bytes).ok() == Some(image::ImageFormat::Jpeg) {
        if let Ok(image) = oxidize_pdf::graphics::Image::from_jpeg_data(bytes.clone()) {
            return Some(image);
        }
    }
    let decoded = image::load_from_memory(&bytes).ok()?.into_rgba8();
    let (width, height) = decoded.dimensions();
    oxidize_pdf::graphics::Image::from_rgba_data(decoded.into_raw(), width, height).ok()
}

fn ruling(paper: Paper) -> Ruling {
    match paper {
        Paper::Blank => Ruling::Blank,
        Paper::Lined => Ruling::Lined,
        Paper::Grid => Ruling::Grid,
        Paper::Dots => Ruling::Dots,
    }
}

fn export_failed() -> CommandError {
    CommandError::new(
        "notebook_pdf_failed",
        "oStudy could not save this notebook as a PDF.",
    )
}
