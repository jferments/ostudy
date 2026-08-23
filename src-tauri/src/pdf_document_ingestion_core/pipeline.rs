use std::path::Path;

use oxidize_pdf::OcrProvider;

use crate::{
    document_domain::DocumentSourceDescriptor,
    error::CommandError,
    paths::AppPaths,
    pdf_backend,
    pdf_ocr_backend::TesseractCliOcrProvider,
    pdf_render_backend::PdfRenderRuntime,
};

use super::{
    active_pdf_extraction_backend, active_pdf_render_backend,
    contracts::{
        ExtractionMode, IngestedDocument, IngestedPage, IngestionProgress,
        OcrIngestionSummary,
    },
    errors::map_pdf_backend_error,
    files::sha256_hex,
    ocr::recover_ocr_pages,
    recovery::{expand_semantic_recovery_pages, rescue_alternate_native_text},
    rendering::render_standard_pages,
    validation::{
        ensure_document_not_canceled, normalize_page_text, validate_pdf_semantic_provenance,
        validate_recovered_text_quality, validate_text_limits,
    },
};

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
    let recovery_plan = rescue_alternate_native_text(
        render_runtime,
        document_id,
        source_path,
        &mut semantic,
        recovery_plan,
    );

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

    let rendered = render_standard_pages(
        paths,
        render_runtime,
        document_id,
        source_path,
        semantic.page_count,
        &mut report_progress,
    )?;
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
    validate_text_limits(&semantic)?;
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

    let normalized_text = semantic
        .pages
        .iter()
        .map(|page| (page.page_number, normalize_page_text(&page.text)))
        .collect::<Vec<_>>();
    let mut pages = Vec::with_capacity(semantic.page_count);
    for page_number in 1..=semantic.page_count {
        let plain_text = normalized_text
            .iter()
            .find(|(number, _)| *number == page_number)
            .map(|(_, text)| text.clone())
            .unwrap_or_default();
        let rendered_page = rendered
            .iter()
            .find(|page| page.page_number == page_number)
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
            render_relative_path: rendered_page.relative_path.clone(),
            render_byte_size: rendered_page.byte_size,
            render_sha256: rendered_page.sha256.clone(),
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
