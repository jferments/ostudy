//! Document ingestion adapters.
//!
//! The active adapter is PDF, but its output is an ostudy-owned
//! `IngestedDocument` with a format-neutral source descriptor. Future DOCX/PPTX
//! adapters can produce the same boundary without changing semantic planning,
//! provider execution, checkpointing, or canonical authoring.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
};

use rusqlite::{params, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::{
    database::Database,
    document_domain::DocumentSourceDescriptor,
    error::CommandError,
    paths::AppPaths,
    pdf_backend::{self, ExtractedDocument, PdfBackendError, PdfBackendErrorKind},
    pdf_render_backend::{
        self, PdfRenderError, PdfRenderErrorKind, PdfRenderRuntime, RenderProcessError,
    },
    util::unix_time_millis,
};

const MAX_PAGE_TEXT_CHARS: usize = 200_000;
const MAX_TOTAL_TEXT_BYTES: usize = 96 * 1024 * 1024;
const MAX_SEMANTIC_TEXT_BYTES: usize = 128 * 1024 * 1024;
const MAX_RENDER_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct IngestedPage {
    pub unit_number: i64,
    pub plain_text: String,
    pub text_sha256: String,
    pub render_relative_path: String,
    pub render_byte_size: i64,
    pub render_sha256: String,
}

#[derive(Debug)]
pub(crate) struct IngestedDocument {
    pub source: DocumentSourceDescriptor,
    pub pages: Vec<IngestedPage>,
    pub semantic: ExtractedDocument,
    pub extraction_backend: &'static str,
    pub render_backend: &'static str,
}

pub(crate) fn active_pdf_extraction_backend() -> &'static str {
    pdf_backend::active_backend_id()
}

pub(crate) fn active_pdf_render_backend() -> &'static str {
    pdf_render_backend::active_backend_id()
}

pub(crate) fn ingest_pdf<F>(
    paths: &AppPaths,
    render_runtime: &PdfRenderRuntime,
    document_id: &str,
    source_path: &Path,
    maximum_source_units: usize,
    mut report_progress: F,
) -> Result<IngestedDocument, CommandError>
where
    F: FnMut(usize, usize) -> Result<(), CommandError>,
{
    let semantic = pdf_backend::extract_document(source_path).map_err(map_pdf_backend_error)?;
    if semantic.page_count == 0 || semantic.page_count > maximum_source_units {
        return Err(CommandError::new(
            "unsupported_document_unit_count",
            format!(
                "This PDF must contain between 1 and {maximum_source_units} pages for automatic document processing."
            ),
        ));
    }
    validate_pdf_semantic_provenance(&semantic)?;

    let mut total_text_bytes = 0_usize;
    let normalized_text = semantic
        .pages
        .iter()
        .map(|page| {
            let text = normalize_page_text(&page.text);
            total_text_bytes = total_text_bytes.saturating_add(text.len());
            (page.page_number, text)
        })
        .collect::<Vec<_>>();
    if total_text_bytes > MAX_TOTAL_TEXT_BYTES {
        return Err(CommandError::new(
            "document_text_too_large",
            "The extracted document text exceeds ostudy's 96 MB processing limit.",
        ));
    }
    let semantic_text_bytes = semantic
        .chunks
        .iter()
        .map(|chunk| chunk.full_text.len())
        .fold(0_usize, usize::saturating_add);
    if semantic_text_bytes > MAX_SEMANTIC_TEXT_BYTES {
        return Err(CommandError::new(
            "document_semantic_content_too_large",
            "The structured document content exceeds ostudy's semantic-processing limit.",
        ));
    }

    report_progress(0, semantic.page_count)?;
    let page_directory = paths.document_pages_dir(document_id);
    fs::create_dir_all(&page_directory).map_err(CommandError::io)?;
    let mut rendered = Vec::<(usize, String, i64, String)>::new();
    let render_result = pdf_render_backend::render_pages(
        render_runtime,
        source_path,
        |page_number, png| -> Result<(), CommandError> {
            if png.is_empty() || png.len() > MAX_RENDER_BYTES {
                return Err(CommandError::new(
                    "document_render_too_large",
                    format!(
                        "The rendered image for source page {page_number} exceeds ostudy's image limit."
                    ),
                ));
            }
            let relative = AppPaths::document_page_relative_path(
                document_id,
                i64::try_from(page_number).unwrap_or(i64::MAX),
            );
            write_replacing_file(&paths.resolve_relative(&relative), &png)?;
            rendered.push((
                page_number,
                relative,
                i64::try_from(png.len()).unwrap_or(i64::MAX),
                sha256_hex(&png),
            ));
            report_progress(rendered.len(), semantic.page_count)?;
            Ok(())
        },
    );
    let rendered_count = match render_result {
        Ok(count) => count,
        Err(RenderProcessError::Visitor(error)) => return Err(error),
        Err(RenderProcessError::Backend(error)) => return Err(map_pdf_render_error(error)),
    };
    if rendered_count != semantic.page_count {
        return Err(CommandError::new(
            "document_backend_unit_count_mismatch",
            "The extraction and rendering adapters reported different source-unit counts.",
        ));
    }
    sync_directory(&page_directory)?;

    let mut pages = Vec::with_capacity(semantic.page_count);
    for page_number in 1..=semantic.page_count {
        let plain_text = normalized_text
            .iter()
            .find(|(number, _)| *number == page_number)
            .map(|(_, text)| text.clone())
            .unwrap_or_default();
        let (_, render_relative_path, render_byte_size, render_sha256) = rendered
            .iter()
            .find(|(number, _, _, _)| *number == page_number)
            .cloned()
            .ok_or_else(|| {
                CommandError::new(
                    "document_render_missing",
                    format!("The PDF renderer did not return source page {page_number}."),
                )
            })?;
        pages.push(IngestedPage {
            unit_number: i64::try_from(page_number).unwrap_or(i64::MAX),
            text_sha256: sha256_hex(plain_text.as_bytes()),
            plain_text,
            render_relative_path,
            render_byte_size,
            render_sha256,
        });
    }

    Ok(IngestedDocument {
        source: DocumentSourceDescriptor::pdf(semantic.page_count),
        pages,
        semantic,
        extraction_backend: active_pdf_extraction_backend(),
        render_backend: active_pdf_render_backend(),
    })
}

pub(crate) fn update_ingestion_progress(
    database: &Database,
    document_id: &str,
    completed_units: usize,
    total_units: usize,
) -> Result<(), CommandError> {
    let total = to_i64(total_units);
    let completed = to_i64(completed_units).min(total);
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE documents
                 SET page_count = ?1, extracted_page_count = ?1,
                     rendered_page_count = ?2, updated_at = ?3
                 WHERE id = ?4 AND lifecycle_state = 'processing'",
                params![total, completed, now, document_id],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_progress_failed",
                    format!("document={document_id} completed={completed} total={total}"),
                    error,
                )
            })?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_changed",
                "The document changed while ingestion progress was being recorded.",
            ));
        }
        Ok(())
    })
}

pub(crate) fn persist_ingested_document(
    database: &Database,
    document_id: &str,
    processed: &IngestedDocument,
) -> Result<(), CommandError> {
    processed.source.validate()?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_persist_begin_failed",
                    format!("document={document_id}"),
                    error,
                )
            })?;
        for table in [
            "document_pages",
            "document_semantic_elements",
            "document_semantic_chunks",
            "document_outlines",
            "document_outline_checkpoints",
            "document_generation_checkpoints",
        ] {
            transaction
                .execute(
                    &format!("DELETE FROM {table} WHERE document_id = ?1"),
                    [document_id],
                )
                .map_err(|error| {
                    CommandError::database_at(
                        "document_ingestion_clear_derivatives_failed",
                        format!("document={document_id} table={table}"),
                        error,
                    )
                })?;
        }

        for page in &processed.pages {
            transaction
                .execute(
                    "INSERT INTO document_pages (
                         document_id, page_number, plain_text, text_sha256,
                         render_relative_path, render_mime_type, render_byte_size,
                         render_sha256, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, 'image/png', ?6, ?7, ?8, ?8)",
                    params![
                        document_id,
                        page.unit_number,
                        page.plain_text,
                        page.text_sha256,
                        page.render_relative_path,
                        page.render_byte_size,
                        page.render_sha256,
                        now,
                    ],
                )
                .map_err(|error| {
                    CommandError::database_at(
                        "document_ingestion_page_insert_failed",
                        format!("document={document_id} unit={}", page.unit_number),
                        error,
                    )
                })?;
        }

        for element in &processed.semantic.elements {
            let text = normalize_semantic_text(&element.text, 250_000);
            transaction
                .execute(
                    "INSERT INTO document_semantic_elements (
                         document_id, element_index, page_number, element_type,
                         text, parent_heading, heading_path_json,
                         bbox_x, bbox_y, bbox_width, bbox_height, confidence,
                         font_name, font_size, is_bold, is_italic,
                         text_sha256, created_at
                     ) VALUES (
                         ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                         ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
                     )",
                    params![
                        document_id,
                        to_i64(element.element_index),
                        to_i64(element.page_number),
                        normalize_element_type(&element.element_type)?,
                        text,
                        element.parent_heading,
                        serde_json::to_string(&element.heading_path)
                            .map_err(CommandError::serialization)?,
                        element.bbox.x,
                        element.bbox.y,
                        element.bbox.width.max(0.0),
                        element.bbox.height.max(0.0),
                        element.confidence.clamp(0.0, 1.0),
                        element.font_name,
                        element.font_size,
                        i64::from(element.is_bold),
                        i64::from(element.is_italic),
                        sha256_hex(text.as_bytes()),
                        now,
                    ],
                )
                .map_err(|error| {
                    CommandError::database_at(
                        "document_ingestion_element_insert_failed",
                        format!("document={document_id} element={}", element.element_index),
                        error,
                    )
                })?;
        }

        for chunk in &processed.semantic.chunks {
            let text = normalize_semantic_text(&chunk.text, 1_000_000);
            let full_text = normalize_semantic_text(&chunk.full_text, 1_100_000);
            let unit_numbers = chunk
                .page_numbers
                .iter()
                .map(|unit| to_i64(*unit))
                .collect::<Vec<_>>();
            transaction
                .execute(
                    "INSERT INTO document_semantic_chunks (
                         document_id, chunk_index, text, full_text,
                         page_numbers_json, bounding_boxes_json, element_types_json,
                         heading_context, token_estimate, is_oversized,
                         text_sha256, created_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        document_id,
                        to_i64(chunk.chunk_index),
                        text,
                        full_text,
                        serde_json::to_string(&unit_numbers)
                            .map_err(CommandError::serialization)?,
                        serde_json::to_string(&chunk.bounding_boxes)
                            .map_err(CommandError::serialization)?,
                        serde_json::to_string(&chunk.element_types)
                            .map_err(CommandError::serialization)?,
                        chunk.heading_context,
                        to_i64(chunk.token_estimate),
                        i64::from(chunk.is_oversized),
                        sha256_hex(text.as_bytes()),
                        now,
                    ],
                )
                .map_err(|error| {
                    CommandError::database_at(
                        "document_ingestion_chunk_insert_failed",
                        format!("document={document_id} chunk={}", chunk.chunk_index),
                        error,
                    )
                })?;
        }

        let unit_count = to_i64(processed.source.unit_count);
        let changed = transaction
            .execute(
                "UPDATE documents
                 SET page_count = ?1, extracted_page_count = ?1, rendered_page_count = ?1,
                     source_format = ?2, source_unit_kind = ?3,
                     extraction_backend = ?4, render_backend = ?5,
                     semantic_element_count = ?6, semantic_chunk_count = ?7,
                     scale_mode = 'standard', estimated_source_tokens = 0,
                     outline_window_count = 0, outline_completed_window_count = 0,
                     generation_window_count = 0, generation_completed_window_count = 0,
                     outline_state = 'pending', safe_error_code = '', safe_error_message = '',
                     updated_at = ?8, revision = revision + 1
                 WHERE id = ?9 AND lifecycle_state = 'processing'",
                params![
                    unit_count,
                    processed.source.format.as_str(),
                    processed.source.unit_kind.as_str(),
                    processed.extraction_backend,
                    processed.render_backend,
                    to_i64(processed.semantic.elements.len()),
                    to_i64(processed.semantic.chunks.len()),
                    now,
                    document_id,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_finalize_failed",
                    format!("document={document_id}"),
                    error,
                )
            })?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_changed",
                "The document changed before extracted derivatives could be committed.",
            ));
        }
        transaction.commit().map_err(|error| {
            CommandError::database_at(
                "document_ingestion_commit_failed",
                format!("document={document_id}"),
                error,
            )
        })
    })
}

fn validate_pdf_semantic_provenance(semantic: &ExtractedDocument) -> Result<(), CommandError> {
    for page in &semantic.pages {
        if page.page_number == 0 || page.page_number > semantic.page_count {
            return Err(CommandError::new(
                "document_extraction_provenance_invalid",
                "The PDF extraction adapter returned a text page outside the document's page range.",
            ));
        }
    }
    for element in &semantic.elements {
        if element.page_number == 0 || element.page_number > semantic.page_count {
            return Err(CommandError::new(
                "document_extraction_provenance_invalid",
                "The PDF extraction adapter returned a semantic element outside the document's page range.",
            ));
        }
    }
    for chunk in &semantic.chunks {
        if chunk.page_numbers.is_empty()
            || chunk
                .page_numbers
                .iter()
                .any(|unit| *unit == 0 || *unit > semantic.page_count)
        {
            return Err(CommandError::new(
                "document_extraction_provenance_invalid",
                "The PDF extraction adapter returned a semantic chunk with invalid page provenance.",
            ));
        }
    }
    Ok(())
}

fn map_pdf_backend_error(error: PdfBackendError) -> CommandError {
    let backend = active_pdf_extraction_backend();
    let page = error.page_number.unwrap_or_default();
    match error.kind {
        PdfBackendErrorKind::Open | PdfBackendErrorKind::PageCount => CommandError::new(
            "pdf_open_failed",
            format!("The PDF could not be opened by the extraction adapter ({backend}): {error}"),
        ),
        PdfBackendErrorKind::ExtractText => CommandError::new(
            "pdf_text_extraction_failed",
            format!("Text could not be extracted from PDF page {page} by {backend}: {error}"),
        ),
        PdfBackendErrorKind::SemanticAnalysis => CommandError::new(
            "pdf_semantic_analysis_failed",
            format!("The PDF structure could not be analyzed by {backend}: {error}"),
        ),
    }
}

fn map_pdf_render_error(error: PdfRenderError) -> CommandError {
    let backend = active_pdf_render_backend();
    let page = error.page_number.unwrap_or_default();
    match error.kind {
        PdfRenderErrorKind::Bind => CommandError::new(
            "pdf_renderer_unavailable",
            format!("The PDF page renderer ({backend}) could not be loaded: {error}"),
        ),
        PdfRenderErrorKind::Open => CommandError::new(
            "pdf_render_open_failed",
            format!("The PDF could not be opened by the rendering adapter ({backend}): {error}"),
        ),
        PdfRenderErrorKind::RenderPage | PdfRenderErrorKind::EncodePage => CommandError::new(
            "pdf_render_failed",
            format!("PDF page {page} could not be rendered by {backend}: {error}"),
        ),
    }
}

fn normalize_element_type(value: &str) -> Result<&'static str, CommandError> {
    match value.to_ascii_lowercase().replace('-', "_").as_str() {
        "title" => Ok("title"),
        "paragraph" => Ok("paragraph"),
        "table" => Ok("table"),
        "header" => Ok("header"),
        "footer" => Ok("footer"),
        "listitem" | "list_item" => Ok("list_item"),
        "image" => Ok("image"),
        "codeblock" | "code_block" => Ok("code_block"),
        "keyvalue" | "key_value" => Ok("key_value"),
        _ => Err(CommandError::new(
            "document_semantic_element_unsupported",
            "The extraction adapter returned an unsupported semantic element type.",
        )),
    }
}

fn normalize_page_text(value: &str) -> String {
    normalize_semantic_text(
        &value.replace("\r\n", "\n").replace('\r', "\n"),
        MAX_PAGE_TEXT_CHARS,
    )
}

fn normalize_semantic_text(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_replacing_file(destination: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let parent = destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_document_path",
            "The rendered-source path is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    let temporary = destination.with_extension(format!("png.{}.tmp", std::process::id()));
    let _ = fs::remove_file(&temporary);
    write_new_file(&temporary, bytes)?;
    if destination.exists() {
        fs::remove_file(destination).map_err(CommandError::io)?;
    }
    fs::rename(&temporary, destination).map_err(CommandError::io)?;
    sync_file(destination)?;
    Ok(())
}

fn write_new_file(destination: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(CommandError::io)?;
    file.write_all(bytes).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)
}

fn sync_file(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)
}

fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_page_text_without_control_characters() {
        assert_eq!(
            normalize_page_text(" line\r\none\0two\rthree "),
            "line\nonetwo\nthree"
        );
    }
}
