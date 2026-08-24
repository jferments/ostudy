use std::{
    path::Path,
    sync::{
        mpsc::{self, TryRecvError},
        Arc, Mutex,
    },
};

use oxidize_pdf::{OcrOptions, OcrProvider};

use crate::{
    error::CommandError,
    pdf_backend::{self, OcrPageCandidate, RecoveredPageText},
    pdf_ocr_backend::{self, TesseractCliOcrProvider},
    pdf_render_backend::{self, PdfRenderRuntime, RenderProcessError},
};

use super::{
    contracts::IngestionProgress,
    errors::{map_ocr_error, map_pdf_render_error},
    validation::{
        bounded_diagnostic, confidence_milli, ensure_document_not_canceled, normalize_page_text,
    },
};

const MAX_OCR_RENDER_BYTES: usize = 64 * 1024 * 1024;
const OCR_LANGUAGE: &str = "en";
const OCR_TIMEOUT_SECONDS: u32 = 120;
const OCR_WORKER_MEMORY_BYTES: u64 = 384 * 1024 * 1024;
const OCR_QUEUE_ITEM_MEMORY_BYTES: u64 = MAX_OCR_RENDER_BYTES as u64;
const OCR_QUEUE_DEPTH_PER_WORKER: usize = 2;

#[derive(Debug)]
struct OcrWorkItem {
    candidate: OcrPageCandidate,
    png: Vec<u8>,
}

#[derive(Debug)]
enum OcrWorkResult {
    Recovered(RecoveredPageText),
    VisualOnly { page_number: usize },
    Failed(CommandError),
}

pub(crate) fn recover_ocr_pages<F>(
    render_runtime: &PdfRenderRuntime,
    document_id: &str,
    source_path: &Path,
    candidates: &[OcrPageCandidate],
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

    let resource_plan = crate::system_resources::plan_parallel_workers(
        candidates.len(),
        OCR_WORKER_MEMORY_BYTES,
        "OSTUDY_OCR_WORKERS",
    );
    let worker_count = resource_plan.worker_count;
    let queue_capacity = crate::system_resources::queue_capacity(
        resource_plan,
        candidates.len(),
        OCR_QUEUE_ITEM_MEMORY_BYTES,
        OCR_QUEUE_DEPTH_PER_WORKER,
    );
    crate::logging::info(
        "document_ocr_parallel_started",
        format!(
            "document={document_id} pages={} logical_cpus={} available_memory_mb={} working_budget_mb={} per_worker_mb={} workers={worker_count} queue_capacity={queue_capacity}",
            candidates.len(),
            resource_plan.logical_cpus,
            resource_plan.available_memory_bytes / (1024 * 1024),
            resource_plan.working_memory_budget_bytes / (1024 * 1024),
            resource_plan.per_worker_memory_bytes / (1024 * 1024),
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
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Ok(()),
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

fn requires_ocr_provider(candidates: &[OcrPageCandidate]) -> bool {
    candidates.iter().any(|candidate| candidate.required)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_estimates_and_queue_items_remain_bounded() {
        assert_eq!(OCR_WORKER_MEMORY_BYTES, 384 * 1024 * 1024);
        assert_eq!(OCR_QUEUE_ITEM_MEMORY_BYTES, 64 * 1024 * 1024);
    }

    #[test]
    fn only_corrupt_text_candidates_require_ocr_runtime() {
        let optional = OcrPageCandidate {
            page_number: 1,
            required: false,
            reasons: vec!["sparse_or_missing_text"],
        };
        assert!(!requires_ocr_provider(std::slice::from_ref(&optional)));
        let required = OcrPageCandidate {
            page_number: 2,
            required: true,
            reasons: vec!["private_use_unicode"],
        };
        assert!(requires_ocr_provider(&[optional, required]));
    }
}
