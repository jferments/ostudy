//! Document ingestion adapters.
//!
//! The active adapter is PDF, but its output is an ostudy-owned
//! `IngestedDocument` with a format-neutral source descriptor. Future DOCX/PPTX
//! adapters can produce the same boundary without changing semantic planning,
//! provider execution, checkpointing, or canonical authoring.

use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    sync::{
        mpsc::{self, TryRecvError},
        Arc, Mutex,
    },
};

use oxidize_pdf::{OcrError, OcrOptions, OcrProvider};
use rusqlite::{params, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::{
    database::Database,
    document_domain::DocumentSourceDescriptor,
    error::CommandError,
    paths::AppPaths,
    pdf_backend::{
        self, ExtractedDocument, PdfBackendError, PdfBackendErrorKind, RecoveredPageText,
    },
    pdf_ocr_backend::{self, TesseractCliOcrProvider},
    pdf_render_backend::{
        self, PdfRenderError, PdfRenderErrorKind, PdfRenderRuntime, RenderProcessError,
    },
    task_cancellation,
    util::unix_time_millis,
};

const MAX_PAGE_TEXT_CHARS: usize = 200_000;
const MAX_TOTAL_TEXT_BYTES: usize = 96 * 1024 * 1024;
const MAX_SEMANTIC_TEXT_BYTES: usize = 128 * 1024 * 1024;
const MAX_RENDER_BYTES: usize = 32 * 1024 * 1024;
const MAX_OCR_RENDER_BYTES: usize = 64 * 1024 * 1024;
const OCR_LANGUAGE: &str = "en";
const OCR_TIMEOUT_SECONDS: u32 = 120;
const MAX_OCR_WORKERS: usize = 8;
const OCR_QUEUE_DEPTH_PER_WORKER: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExtractionMode {
    Native,
    PageTextFallback,
    OcrFallback,
}

impl ExtractionMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::PageTextFallback => "page_text_fallback",
            Self::OcrFallback => "ocr_fallback",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IngestionProgress {
    Rendering {
        completed_units: usize,
        total_units: usize,
    },
    Ocr {
        completed_units: usize,
        total_units: usize,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct OcrIngestionSummary {
    pub page_count: usize,
    pub mean_confidence: Option<f64>,
    pub engine: String,
}

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
    pub extraction_mode: ExtractionMode,
    pub ocr: OcrIngestionSummary,
}

#[derive(Debug)]
struct OcrWorkItem {
    candidate: pdf_backend::OcrPageCandidate,
    png: Vec<u8>,
}

#[derive(Debug)]
enum OcrWorkResult {
    Recovered(RecoveredPageText),
    VisualOnly { page_number: usize },
    Failed(CommandError),
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
    F: FnMut(IngestionProgress) -> Result<(), CommandError>,
{
    ensure_document_not_canceled(document_id)?;
    let mut semantic = pdf_backend::extract_document(source_path).map_err(map_pdf_backend_error)?;
    ensure_document_not_canceled(document_id)?;
    if semantic.page_count == 0 || semantic.page_count > maximum_source_units {
        return Err(CommandError::new(
            "unsupported_document_unit_count",
            format!(
                "This PDF must contain between 1 and {maximum_source_units} pages for automatic document processing."
            ),
        ));
    }
    validate_pdf_semantic_provenance(&semantic)?;
    let recovery_plan = pdf_backend::plan_text_recovery(&semantic);
    for candidate in &recovery_plan.ocr_pages {
        crate::logging::info(
            "document_ocr_fallback_detected",
            format!(
                "document={document_id} page={} required={} reasons={}",
                candidate.page_number,
                candidate.required,
                candidate.reasons.join(",")
            ),
        );
    }
    for page in &mut semantic.pages {
        page.text = normalize_page_text(&page.text);
    }

    report_progress(IngestionProgress::Rendering {
        completed_units: 0,
        total_units: semantic.page_count,
    })?;
    let page_directory = paths.document_pages_dir(document_id);
    fs::create_dir_all(&page_directory).map_err(CommandError::io)?;
    let mut rendered = Vec::<(usize, String, i64, String)>::new();
    let render_result = pdf_render_backend::render_pages(
        render_runtime,
        source_path,
        |page_number, png| {
            ensure_document_not_canceled(document_id)?;
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
            report_progress(IngestionProgress::Rendering {
                completed_units: rendered.len(),
                total_units: semantic.page_count,
            })?;
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
    ensure_document_not_canceled(document_id)?;

    let recovered_pages = recover_ocr_pages(
        render_runtime,
        document_id,
        source_path,
        &recovery_plan.ocr_pages,
        &mut report_progress,
    )?;
    let ocr_page_count = recovered_pages.len();
    let ocr_mean_confidence = if recovered_pages.is_empty() {
        None
    } else {
        Some(
            recovered_pages
                .iter()
                .map(|page| page.confidence)
                .sum::<f64>()
                / recovered_pages.len() as f64,
        )
    };
    let semantic_recovery_pages = expand_semantic_recovery_pages(
        &semantic,
        &recovered_pages,
        &recovery_plan.native_semantic_fallback_pages,
    );
    pdf_backend::apply_text_recovery(&mut semantic, &recovered_pages, &semantic_recovery_pages);
    validate_pdf_semantic_provenance(&semantic)?;
    validate_recovered_text_quality(&semantic)?;
    ensure_document_not_canceled(document_id)?;

    let extraction_mode = if ocr_page_count > 0 {
        ExtractionMode::OcrFallback
    } else if semantic.semantic_analysis_degraded
        || !recovery_plan.native_semantic_fallback_pages.is_empty()
    {
        ExtractionMode::PageTextFallback
    } else {
        ExtractionMode::Native
    };
    if extraction_mode != ExtractionMode::Native {
        crate::logging::info(
            "document_extraction_degraded",
            format!(
                "document={document_id} mode={} ocr_pages={} semantic_native_fallback_pages={} recovery_context_pages={}",
                extraction_mode.as_str(),
                ocr_page_count,
                recovery_plan.native_semantic_fallback_pages.len(),
                semantic_recovery_pages.len(),
            ),
        );
    }

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
        extraction_mode,
        ocr: OcrIngestionSummary {
            page_count: ocr_page_count,
            mean_confidence: ocr_mean_confidence,
            engine: if ocr_page_count == 0 {
                String::new()
            } else {
                TesseractCliOcrProvider::new().engine_name().to_string()
            },
        },
    })
}

fn recover_ocr_pages<F>(
    render_runtime: &PdfRenderRuntime,
    document_id: &str,
    source_path: &Path,
    candidates: &[pdf_backend::OcrPageCandidate],
    report_progress: &mut F,
) -> Result<Vec<RecoveredPageText>, CommandError>
where
    F: FnMut(IngestionProgress) -> Result<(), CommandError>,
{
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    ensure_document_not_canceled(document_id)?;
    let availability_probe = TesseractCliOcrProvider::for_document(document_id);
    if let Err(error) = availability_probe.ensure_available() {
        if requires_ocr_provider(candidates) {
            return Err(map_ocr_error(None, error));
        }
        crate::logging::info(
            "document_optional_ocr_unavailable",
            format!(
                "document={document_id} optional_pages={} detail={}",
                candidates.len(),
                bounded_diagnostic(&error.to_string(), 320)
            ),
        );
        return Ok(Vec::new());
    }
    report_progress(IngestionProgress::Ocr {
        completed_units: 0,
        total_units: candidates.len(),
    })?;

    let worker_count = ocr_worker_count(candidates.len());
    let queue_capacity = worker_count
        .saturating_mul(OCR_QUEUE_DEPTH_PER_WORKER)
        .max(1);
    crate::logging::info(
        "document_ocr_parallel_started",
        format!(
            "document={document_id} pages={} workers={worker_count} queue_capacity={queue_capacity}",
            candidates.len()
        ),
    );

    let page_numbers = candidates
        .iter()
        .map(|candidate| candidate.page_number)
        .collect::<Vec<_>>();
    let (work_sender, work_receiver) = mpsc::sync_channel::<OcrWorkItem>(queue_capacity);
    let work_receiver = Arc::new(Mutex::new(work_receiver));
    let (result_sender, result_receiver) = mpsc::channel::<OcrWorkResult>();
    let mut recovered = Vec::new();
    let mut completed = 0_usize;

    std::thread::scope(|scope| -> Result<Vec<RecoveredPageText>, CommandError> {
        for _ in 0..worker_count {
            let work_receiver = Arc::clone(&work_receiver);
            let result_sender = result_sender.clone();
            scope.spawn(move || {
                let provider = TesseractCliOcrProvider::for_document(document_id);
                loop {
                    let item = {
                        let receiver = match work_receiver.lock() {
                            Ok(receiver) => receiver,
                            Err(_) => break,
                        };
                        match receiver.recv() {
                            Ok(item) => item,
                            Err(_) => break,
                        }
                    };
                    let outcome = process_ocr_work_item(&provider, document_id, item);
                    if result_sender.send(outcome).is_err() {
                        break;
                    }
                }
            });
        }
        drop(result_sender);

        let render_result = pdf_render_backend::render_ocr_pages(
            render_runtime,
            source_path,
            &page_numbers,
            |page_number, rendered| -> Result<(), CommandError> {
                ensure_document_not_canceled(document_id)?;
                drain_ocr_results(
                    &result_receiver,
                    &mut recovered,
                    &mut completed,
                    candidates.len(),
                    report_progress,
                )?;
                let candidate = candidates
                    .iter()
                    .find(|candidate| candidate.page_number == page_number)
                    .cloned()
                    .ok_or_else(|| {
                        CommandError::new(
                            "document_ocr_plan_changed",
                            "The PDF OCR recovery plan changed while the page was being processed.",
                        )
                    })?;
                let png = match rendered {
                    Ok(png) => png,
                    Err(error) if candidate.required => return Err(map_pdf_render_error(error)),
                    Err(error) => {
                        crate::logging::info(
                            "document_optional_ocr_render_failed",
                            format!(
                                "document={document_id} page={page_number} detail={}",
                                bounded_diagnostic(&error.to_string(), 320)
                            ),
                        );
                        completed = completed.saturating_add(1);
                        report_progress(IngestionProgress::Ocr {
                            completed_units: completed,
                            total_units: candidates.len(),
                        })?;
                        return Ok(());
                    }
                };
                if png.is_empty() || png.len() > MAX_OCR_RENDER_BYTES {
                    if candidate.required {
                        return Err(CommandError::new(
                            "document_ocr_render_too_large",
                            format!(
                                "The high-resolution OCR render for PDF page {page_number} exceeds ostudy's image safety limit."
                            ),
                        ));
                    }
                    crate::logging::info(
                        "document_optional_ocr_render_skipped",
                        format!(
                            "document={document_id} page={page_number} bytes={}",
                            png.len()
                        ),
                    );
                    completed = completed.saturating_add(1);
                    report_progress(IngestionProgress::Ocr {
                        completed_units: completed,
                        total_units: candidates.len(),
                    })?;
                    return Ok(());
                }
                work_sender
                    .send(OcrWorkItem { candidate, png })
                    .map_err(|_| {
                        CommandError::new(
                            "document_ocr_worker_stopped",
                            "The OCR worker pool stopped before all selected PDF pages were processed.",
                        )
                    })?;
                drain_ocr_results(
                    &result_receiver,
                    &mut recovered,
                    &mut completed,
                    candidates.len(),
                    report_progress,
                )?;
                Ok(())
            },
        );
        drop(work_sender);

        match render_result {
            Ok(count) if count == candidates.len() => {}
            Ok(_) => {
                return Err(CommandError::new(
                    "document_ocr_render_incomplete",
                    "The PDF renderer did not attempt every page selected for OCR recovery.",
                ));
            }
            Err(RenderProcessError::Visitor(error)) => return Err(error),
            Err(RenderProcessError::Backend(error)) => return Err(map_pdf_render_error(error)),
        }

        for result in result_receiver {
            apply_ocr_result(
                result,
                &mut recovered,
                &mut completed,
                candidates.len(),
                report_progress,
            )?;
        }
        if completed != candidates.len() {
            return Err(CommandError::new(
                "document_ocr_worker_incomplete",
                "The OCR worker pool stopped before every selected PDF page reached a terminal result.",
            ));
        }
        recovered.sort_by_key(|page| page.page_number);
        Ok(recovered)
    })
}

fn process_ocr_work_item(
    provider: &TesseractCliOcrProvider,
    document_id: &str,
    item: OcrWorkItem,
) -> OcrWorkResult {
    let page_number = item.candidate.page_number;
    let options = OcrOptions {
        language: OCR_LANGUAGE.to_string(),
        timeout_seconds: OCR_TIMEOUT_SECONDS,
        debug_output: false,
        ..Default::default()
    };
    let result = match provider.process_image(&item.png, &options) {
        Ok(result) => result,
        Err(error) if item.candidate.required => {
            return OcrWorkResult::Failed(map_ocr_error(Some(page_number), error));
        }
        Err(error) => {
            if pdf_ocr_backend::is_cancellation_error(&error) {
                return OcrWorkResult::Failed(map_ocr_error(Some(page_number), error));
            }
            crate::logging::info(
                "document_optional_ocr_page_failed",
                format!(
                    "document={document_id} page={page_number} detail={}",
                    bounded_diagnostic(&error.to_string(), 320)
                ),
            );
            return OcrWorkResult::VisualOnly { page_number };
        }
    };
    let text = normalize_page_text(&result.text);
    let quality = pdf_backend::assess_text_quality(&text);
    if item.candidate.required && (text.is_empty() || quality.suspicious) {
        crate::logging::info(
            "document_ocr_text_rejected",
            format!(
                "document={document_id} page={page_number} confidence_milli={} chars={} substantive={} reasons={}",
                confidence_milli(result.confidence),
                text.chars().count(),
                quality.substantive,
                quality.reasons.join(",")
            ),
        );
        return OcrWorkResult::Failed(CommandError::new(
            "document_ocr_text_unusable",
            format!(
                "The PDF text layer on page {page_number} is unreliable, and OCR could not recover trustworthy text from the PDFium-rendered page. The original PDF remains available for visual review."
            ),
        ));
    }
    if !text.is_empty() && !quality.suspicious && (item.candidate.required || quality.substantive) {
        crate::logging::info(
            "document_ocr_page_recovered",
            format!(
                "document={document_id} page={page_number} confidence_milli={} required={}",
                confidence_milli(result.confidence),
                item.candidate.required
            ),
        );
        return OcrWorkResult::Recovered(RecoveredPageText {
            page_number,
            text,
            confidence: result.confidence.clamp(0.0, 1.0),
        });
    }
    crate::logging::info(
        "document_ocr_sparse_page_kept_visual",
        format!("document={document_id} page={page_number}"),
    );
    OcrWorkResult::VisualOnly { page_number }
}

fn drain_ocr_results<F>(
    receiver: &mpsc::Receiver<OcrWorkResult>,
    recovered: &mut Vec<RecoveredPageText>,
    completed: &mut usize,
    total: usize,
    report_progress: &mut F,
) -> Result<(), CommandError>
where
    F: FnMut(IngestionProgress) -> Result<(), CommandError>,
{
    loop {
        match receiver.try_recv() {
            Ok(result) => apply_ocr_result(result, recovered, completed, total, report_progress)?,
            Err(TryRecvError::Empty) => return Ok(()),
            Err(TryRecvError::Disconnected) => return Ok(()),
        }
    }
}

fn apply_ocr_result<F>(
    result: OcrWorkResult,
    recovered: &mut Vec<RecoveredPageText>,
    completed: &mut usize,
    total: usize,
    report_progress: &mut F,
) -> Result<(), CommandError>
where
    F: FnMut(IngestionProgress) -> Result<(), CommandError>,
{
    match result {
        OcrWorkResult::Recovered(page) => recovered.push(page),
        OcrWorkResult::VisualOnly { page_number } => {
            crate::logging::info(
                "document_ocr_page_visual_only",
                format!("page={page_number}"),
            );
        }
        OcrWorkResult::Failed(error) => return Err(error),
    }
    *completed = completed.saturating_add(1);
    report_progress(IngestionProgress::Ocr {
        completed_units: *completed,
        total_units: total,
    })
}

fn ocr_worker_count(candidate_count: usize) -> usize {
    let available = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    bounded_ocr_worker_count(candidate_count, available)
}

fn bounded_ocr_worker_count(candidate_count: usize, available: usize) -> usize {
    available
        .max(1)
        .min(MAX_OCR_WORKERS)
        .min(candidate_count.max(1))
}

fn expand_semantic_recovery_pages(
    document: &ExtractedDocument,
    recovered_pages: &[RecoveredPageText],
    native_fallback_pages: &[usize],
) -> Vec<usize> {
    let mut pages = native_fallback_pages
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    pages.extend(recovered_pages.iter().map(|page| page.page_number));
    if pages.is_empty() {
        return Vec::new();
    }

    loop {
        let before = pages.len();
        for chunk in &document.chunks {
            if chunk.page_numbers.iter().any(|page| pages.contains(page)) {
                pages.extend(chunk.page_numbers.iter().copied());
            }
        }
        if pages.len() == before {
            break;
        }
    }
    pages.into_iter().collect()
}

fn requires_ocr_provider(candidates: &[pdf_backend::OcrPageCandidate]) -> bool {
    candidates.iter().any(|candidate| candidate.required)
}

fn ensure_document_not_canceled(document_id: &str) -> Result<(), CommandError> {
    if task_cancellation::is_document_task_canceled(document_id) {
        return Err(CommandError::new(
            "ai_request_canceled",
            "The document generation request was canceled.",
        ));
    }
    Ok(())
}

pub(crate) fn update_ingestion_progress(
    database: &Database,
    document_id: &str,
    progress: IngestionProgress,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = match progress {
            IngestionProgress::Rendering {
                completed_units,
                total_units,
            } => {
                let total = to_i64(total_units);
                let completed = to_i64(completed_units).min(total);
                connection
                    .execute(
                        "UPDATE documents
                         SET page_count = ?1, extracted_page_count = ?1,
                             rendered_page_count = ?2,
                             extraction_mode = 'native', ocr_page_count = 0,
                             ocr_completed_page_count = 0, ocr_mean_confidence = NULL,
                             ocr_engine = '', updated_at = ?3
                         WHERE id = ?4 AND lifecycle_state = 'processing'",
                        params![total, completed, now, document_id],
                    )
                    .map_err(|error| {
                        CommandError::database_at(
                            "document_ingestion_progress_failed",
                            format!(
                                "document={document_id} stage=render completed={completed} total={total}"
                            ),
                            error,
                        )
                    })?
            }
            IngestionProgress::Ocr {
                completed_units,
                total_units,
            } => {
                let total = to_i64(total_units);
                let completed = to_i64(completed_units).min(total);
                connection
                    .execute(
                        "UPDATE documents
                         SET extraction_mode = 'ocr_fallback', ocr_page_count = ?1,
                             ocr_completed_page_count = ?2, ocr_mean_confidence = NULL,
                             ocr_engine = 'tesseract-cli', updated_at = ?3
                         WHERE id = ?4 AND lifecycle_state = 'processing'",
                        params![total, completed, now, document_id],
                    )
                    .map_err(|error| {
                        CommandError::database_at(
                            "document_ocr_progress_failed",
                            format!(
                                "document={document_id} completed={completed} total={total}"
                            ),
                            error,
                        )
                    })?
            }
        };
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
        let ocr_page_count = to_i64(processed.ocr.page_count);
        let changed = transaction
            .execute(
                "UPDATE documents
                 SET page_count = ?1, extracted_page_count = ?1, rendered_page_count = ?1,
                     source_format = ?2, source_unit_kind = ?3,
                     extraction_backend = ?4, render_backend = ?5,
                     extraction_mode = ?6, ocr_page_count = ?7,
                     ocr_completed_page_count = ?7, ocr_mean_confidence = ?8,
                     ocr_engine = ?9,
                     semantic_element_count = ?10, semantic_chunk_count = ?11,
                     scale_mode = 'standard', estimated_source_tokens = 0,
                     outline_window_count = 0, outline_completed_window_count = 0,
                     generation_window_count = 0, generation_completed_window_count = 0,
                     outline_state = 'pending', safe_error_code = '', safe_error_message = '',
                     updated_at = ?12, revision = revision + 1
                 WHERE id = ?13 AND lifecycle_state = 'processing'",
                params![
                    unit_count,
                    processed.source.format.as_str(),
                    processed.source.unit_kind.as_str(),
                    processed.extraction_backend,
                    processed.render_backend,
                    processed.extraction_mode.as_str(),
                    ocr_page_count,
                    processed.ocr.mean_confidence,
                    processed.ocr.engine,
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

fn validate_recovered_text_quality(semantic: &ExtractedDocument) -> Result<(), CommandError> {
    for chunk in &semantic.chunks {
        if pdf_backend::assess_text_quality(&chunk.full_text).suspicious {
            return Err(CommandError::new(
                "document_text_layer_unreliable",
                format!(
                    "The PDF still contains an unreliable text mapping around source page {} after native/OCR recovery. ostudy stopped before sending corrupted text to the AI provider.",
                    chunk.page_numbers.first().copied().unwrap_or_default()
                ),
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
    }
}

fn map_ocr_error(page_number: Option<usize>, error: OcrError) -> CommandError {
    if pdf_ocr_backend::is_cancellation_error(&error) {
        return CommandError::new(
            "ai_request_canceled",
            "The document generation request was canceled.",
        );
    }
    let page = page_number
        .map(|page| format!(" page {page}"))
        .unwrap_or_default();
    match error {
        OcrError::ProviderNotAvailable(_) => CommandError::new(
            "document_ocr_unavailable",
            "The PDF text layer is unreliable and ostudy needs local OCR to recover it, but Tesseract OCR is not available. Install Tesseract with English trained data (or set OSTUDY_TESSERACT_PATH), then retry generation. The original PDF remains in Documents.",
        ),
        OcrError::LowConfidence(_) => CommandError::new(
            "document_ocr_low_confidence",
            format!(
                "OCR confidence was too low to trust the recovered text for PDF{page}. The original PDF remains available for visual review."
            ),
        ),
        other => CommandError::new(
            "document_ocr_failed",
            format!(
                "The PDF text layer is unreliable and OCR failed for PDF{page}: {other}. The original PDF remains in Documents."
            ),
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

fn bounded_diagnostic(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || *character == '\n')
        .take(maximum)
        .collect::<String>()
        .replace('\n', " ")
}

fn confidence_milli(value: f64) -> usize {
    if !value.is_finite() {
        return 0;
    }
    (value.clamp(0.0, 1.0) * 1_000.0).round() as usize
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

    #[test]
    fn extraction_modes_have_stable_database_values() {
        assert_eq!(ExtractionMode::Native.as_str(), "native");
        assert_eq!(
            ExtractionMode::PageTextFallback.as_str(),
            "page_text_fallback"
        );
        assert_eq!(ExtractionMode::OcrFallback.as_str(), "ocr_fallback");
    }

    #[test]
    fn ocr_worker_count_uses_multiple_cores_without_unbounded_fanout() {
        assert_eq!(bounded_ocr_worker_count(1, 32), 1);
        assert_eq!(bounded_ocr_worker_count(4, 32), 4);
        assert_eq!(bounded_ocr_worker_count(100, 32), MAX_OCR_WORKERS);
        assert_eq!(bounded_ocr_worker_count(100, 3), 3);
        assert_eq!(bounded_ocr_worker_count(100, 0), 1);
    }

    #[test]
    fn only_corrupt_text_candidates_make_the_ocr_runtime_mandatory() {
        let optional = pdf_backend::OcrPageCandidate {
            page_number: 1,
            required: false,
            reasons: vec!["sparse_or_missing_text"],
        };
        assert!(!requires_ocr_provider(std::slice::from_ref(&optional)));

        let required = pdf_backend::OcrPageCandidate {
            page_number: 2,
            required: true,
            reasons: vec!["private_use_unicode"],
        };
        assert!(requires_ocr_provider(&[optional, required]));
    }

    #[test]
    fn semantic_recovery_expands_across_chunks_that_cross_corrupt_page_boundaries() {
        let semantic = ExtractedDocument {
            page_count: 3,
            pages: vec![
                pdf_backend::ExtractedTextPage {
                    page_number: 1,
                    text: "recovered page one".to_string(),
                },
                pdf_backend::ExtractedTextPage {
                    page_number: 2,
                    text: "clean page two".to_string(),
                },
                pdf_backend::ExtractedTextPage {
                    page_number: 3,
                    text: "clean page three".to_string(),
                },
            ],
            elements: Vec::new(),
            chunks: vec![
                pdf_backend::ExtractedSemanticChunk {
                    chunk_index: 0,
                    text: "pages one and two".to_string(),
                    full_text: "pages one and two".to_string(),
                    page_numbers: vec![1, 2],
                    bounding_boxes: Vec::new(),
                    element_types: vec!["paragraph".to_string()],
                    heading_context: None,
                    token_estimate: 4,
                    is_oversized: false,
                },
                pdf_backend::ExtractedSemanticChunk {
                    chunk_index: 1,
                    text: "pages two and three".to_string(),
                    full_text: "pages two and three".to_string(),
                    page_numbers: vec![2, 3],
                    bounding_boxes: Vec::new(),
                    element_types: vec!["paragraph".to_string()],
                    heading_context: None,
                    token_estimate: 4,
                    is_oversized: false,
                },
            ],
            native_text_failed_pages: Vec::new(),
            semantic_analysis_degraded: false,
        };
        let recovered = [RecoveredPageText {
            page_number: 1,
            text: "recovered page one".to_string(),
            confidence: 0.9,
        }];
        assert_eq!(
            expand_semantic_recovery_pages(&semantic, &recovered, &[]),
            vec![1, 2, 3]
        );
    }
}
