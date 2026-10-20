//! Saving a PDF with its annotations drawn in.
//!
//! The learner's marks stay out of the stored document. Exporting writes a
//! new copy: the stored PDF's bytes followed by an update that draws each
//! annotated page's marks over it (see `pdf_export`). The copy is a prepared
//! export, which the learner then saves or shares like any other.

use std::fs;

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::{
    load_annotations,
    page_tiles::pdf_source_path,
    pdf_export::{annotated_pdf, ExportError, PageAnnotations},
    AnnotationShape,
};
use crate::{
    database::Database,
    error::CommandError,
    external_files::{PreparedExportDto, RegisterPreparedExport},
    paths::AppPaths,
    util::validate_entity_id,
};

const EXPORT_PURPOSE: &str = "annotated_pdf";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareAnnotatedPdfInput {
    pub document_id: String,
    pub operation_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotatedPdfSummary {
    annotation_count: i64,
    page_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedAnnotatedPdfDto {
    pub prepared_export: PreparedExportDto,
    /// Annotations drawn into the copy, and the pages they are on.
    pub annotation_count: i64,
    pub page_count: i64,
}

/// Prepares a copy of a PDF document with every annotation drawn into its
/// pages. Repeating an operation returns the copy it already prepared.
pub(crate) async fn prepare_annotated_pdf(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareAnnotatedPdfInput,
) -> Result<PreparedAnnotatedPdfDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    validate_entity_id(&input.operation_id, "annotated PDF export operation")?;

    if let Some((prepared_export, summary)) = database.with_connection(|connection| {
        crate::external_files::load_prepared_export_summary::<AnnotatedPdfSummary>(
            connection,
            paths.inner(),
            &input.operation_id,
            EXPORT_PURPOSE,
        )
    })? {
        return Ok(PreparedAnnotatedPdfDto {
            prepared_export,
            annotation_count: summary.annotation_count,
            page_count: summary.page_count,
        });
    }

    let (relative_source, title, annotations) = database.with_connection(|connection| {
        let title: String = connection
            .query_row(
                "SELECT title FROM documents WHERE id = ?1",
                [&input.document_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_not_found",
                    "That document is no longer in the library.",
                )
            })?;
        Ok((
            pdf_source_path(connection, &input.document_id)?,
            title,
            load_annotations(connection, &input.document_id)?,
        ))
    })?;
    if annotations.is_empty() {
        return Err(CommandError::new(
            "annotated_pdf_empty",
            "This document has no annotations to save yet.",
        ));
    }

    let source_path = paths.resolve_relative(&relative_source);
    let relative = AppPaths::prepared_export_relative_path(&input.operation_id, "pdf");
    let destination = paths.resolve_relative(&relative);
    let shapes: Vec<(u32, AnnotationShape)> = annotations
        .into_iter()
        .filter_map(|annotation| {
            u32::try_from(annotation.page_number)
                .ok()
                .map(|page| (page, annotation.shape))
        })
        .collect();
    let annotation_count = i64::try_from(shapes.len()).unwrap_or(i64::MAX);
    let page_count = tauri::async_runtime::spawn_blocking(move || {
        write_annotated_copy(&source_path, &destination, &shapes)
    })
    .await
    .map_err(|_| export_failed())??;

    let summary = AnnotatedPdfSummary {
        annotation_count,
        page_count,
    };
    let suggested_filename = format!(
        "{}-annotated.pdf",
        crate::document_study_guide_export::portable_filename_or(&title, "document")
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
    Ok(PreparedAnnotatedPdfDto {
        prepared_export,
        annotation_count,
        page_count,
    })
}

/// Writes the annotated copy and returns how many pages it drew on. `shapes`
/// come in page order, earliest first on each page.
fn write_annotated_copy(
    source: &std::path::Path,
    destination: &std::path::Path,
    shapes: &[(u32, AnnotationShape)],
) -> Result<i64, CommandError> {
    let bytes = fs::read(source).map_err(CommandError::io)?;
    let mut pages: Vec<PageAnnotations<'_>> = Vec::new();
    for (page_number, shape) in shapes {
        match pages.last_mut() {
            Some(page) if page.page_number == *page_number => page.shapes.push(shape),
            _ => pages.push(PageAnnotations {
                page_number: *page_number,
                shapes: vec![shape],
            }),
        }
    }
    let font = shapes
        .iter()
        .any(|(_, shape)| matches!(shape, AnnotationShape::Text(_)))
        .then(|| crate::document_study_guide_export::plain_sans_font().to_vec());
    let output = annotated_pdf(&bytes, &pages, font).map_err(|error| {
        crate::logging::warn_target(
            "documents.annotations",
            "annotated_pdf_failed",
            "A PDF could not be saved with its annotations.",
            serde_json::json!({ "error": error.to_string() }),
        );
        match error {
            ExportError::Encrypted => CommandError::new(
                "annotated_pdf_encrypted",
                "This PDF is password protected, so oStudy cannot save a copy with your annotations.",
            ),
            ExportError::Unreadable(_) => CommandError::new(
                "annotated_pdf_unreadable",
                "oStudy could not read this PDF's structure to save a copy with your annotations.",
            ),
            ExportError::PageMissing(_) | ExportError::Write(_) => export_failed(),
        }
    })?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    fs::write(destination, output).map_err(CommandError::io)?;
    Ok(i64::try_from(pages.len()).unwrap_or(i64::MAX))
}

fn export_failed() -> CommandError {
    CommandError::new(
        "annotated_pdf_failed",
        "oStudy could not save a copy of this PDF with your annotations.",
    )
}
